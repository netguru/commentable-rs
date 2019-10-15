use std::fmt;
use std::collections::HashMap;

use chrono::{DateTime, Utc};
use crypto::digest::Digest;
use crypto::sha3::Sha3;
use maplit::hashmap;
use rusoto_dynamodb::{
  DynamoDb,
  DynamoDbClient,
  GetItemInput,
  QueryInput,
  PutItemInput,
  UpdateItemInput,
  DeleteItemInput,
  BatchWriteItemInput,
  WriteRequest,
  DeleteRequest,
  AttributeValue
};
use serde::Serialize;

pub type CommentableId = String;
pub type PrimaryKey = String;
pub type SortKey = String;

pub static COMMENTABLE_RS_TABLE_NAME: &str = "CommentableRsTable";
pub static REPLIES_INDEX_NAME: &str = "replies-index";
pub static REACTIONS_INDEX_NAME: &str = "reactions-index";

#[derive(Debug)]
pub enum DbError {
  Error(String),
  RecordInvalid(String),
}

impl fmt::Display for DbError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "\"{}\"", match self {
      DbError::Error(msg) => format!("DbError::Error -> {}", msg),
      DbError::RecordInvalid(msg) => format!("DbError::RecordInvalid -> {}", msg),
    })
  }
}


pub type DynamoDbAttributes = HashMap<String, AttributeValue>;

// A helper trait that allows for easier access to AttributeValue contents with validations
pub trait DynamoDbRecord {
  fn string(&mut self, field_name: &str) -> Result<String, DbError>;
  fn timestamp(&mut self, field_name: &str) -> Result<DateTime<Utc>, DbError>;
  fn optional_string(&mut self, field_name: &str) -> Option<String>;
}

impl DynamoDbRecord for DynamoDbAttributes {
  fn string(&mut self, field_name: &str) -> Result<String, DbError> {
    self.remove(field_name)
        .and_then(|value| value.s)
        .ok_or(DbError::RecordInvalid(format!("Missing field '{}'.", field_name)))
  }

  fn timestamp(&mut self, field_name: &str) -> Result<DateTime<Utc>, DbError> {
    self.remove(field_name) // -> Option<AttributeValue>
      .and_then(|value| value.s) // -> Option<String>
      .ok_or(DbError::RecordInvalid(format!("Missing field '{}'.", field_name))) // -> Result<String, DbError>
      .and_then(|string|
        DateTime::parse_from_rfc3339(&string).map_err(|_|
          DbError::Error(format!("Error parsing timestamps in field '{}'", field_name))
        )
      ) // -> Result<DateTime<FixedOffset>, DbError>
      .and_then(|datetime| Ok(datetime.with_timezone(&Utc))) // -> Result<DateTime<Utc>, DbError>
  }

  fn optional_string(&mut self, field_name: &str) -> Option<String> {
    self.remove(field_name)
        .and_then(|value| value.s)
  }
}

// This struct allows us to easily create DynamoDbAttributes
// by implementing Into<IntoDynamoDbAttributes> for custom structs.
pub struct IntoDynamoDbAttributes {
  pub attributes: HashMap<String, IntoAttributeValue>
}

// This trait implementation allows us to create DynamoDbAttributes
// from any HashMap<String, IntoAttributeValue>
impl From<IntoDynamoDbAttributes> for DynamoDbAttributes {
  fn from(mut attributes: IntoDynamoDbAttributes) -> Self {
    attributes.attributes.drain().map(|(key, value)| {
      (key, value.into())
    }).collect()
  }
}

// A wrapper struct that allows for implementation of Into<AttributeValue>
// for arbitraty types like Strings, u32s etc
pub struct IntoAttributeValue {
  attribute_value: AttributeValue,
}

// Main trait for handling DynamoDB records
pub trait DynamoDbModel where Self: Sized + Serialize {
  // #new is used internally to create structs from DynamoDB records
  fn new(attributes: DynamoDbAttributes) -> Result<Self, DbError>;

  fn find(db: &DynamoDbClient, key: PrimaryKey, id: SortKey) -> Result<Option<Self>, DbError> {
    db.get_item(GetItemInput {
      key: hashmap!{
        String::from("primary_key") => attribute_value(key),
        String::from("id") => attribute_value(id),
      },
      table_name: COMMENTABLE_RS_TABLE_NAME.to_string(),
      ..Default::default()
    }).sync()
      .map_err(|err| DbError::Error(err.to_string()))
      .and_then(|output| {
        Ok(output.item.and_then(|attributes| {
          // The unwrapping below should be safe, as we're restoring the struct from an existing record
          Some(Self::new(attributes).unwrap())
        }))
      })
  }

  fn query(db: &DynamoDbClient, query_input: QueryInput) -> Result<Vec<DynamoDbAttributes>, DbError> {
    let mut results: Vec<DynamoDbAttributes> = vec![];
    let mut last_evaluated_key = None;

    'pagination: loop {
      db.query(QueryInput {
        exclusive_start_key: last_evaluated_key.clone(),
        ..query_input.clone()
      }).sync()
        .map_err(|err| DbError::Error(err.to_string()))
        .and_then(|query_output| {
          results.append(query_output.items.unwrap_or(vec![]).as_mut());
          last_evaluated_key = query_output.last_evaluated_key;
          Ok(())
        })?;

      if last_evaluated_key == None {
        break 'pagination;
      }
    }

    return Ok(results);
  }

  fn create(db: &DynamoDbClient, attributes: IntoDynamoDbAttributes) -> Result<Self, DbError> {
    let attributes: DynamoDbAttributes = attributes.into();
    db.put_item(PutItemInput {
      item: attributes.clone(),
      table_name: COMMENTABLE_RS_TABLE_NAME.to_string(),
      ..Default::default()
    }).sync()
      .map_err(|err| DbError::Error(err.to_string()))
      .and_then(|_| Self::new(attributes))
  }

  fn update(db: &DynamoDbClient, key: PrimaryKey, id: SortKey, expression: String, values: HashMap<String, AttributeValue>) -> Result<Self, DbError> {
    db.update_item(UpdateItemInput {
      table_name: COMMENTABLE_RS_TABLE_NAME.to_string(),
      key: hashmap!{
        String::from("primary_key") => attribute_value(key),
        String::from("id") => attribute_value(id),
      },
      update_expression: Some(expression),
      expression_attribute_values: Some(values),
      return_values: Some(String::from("ALL_NEW")),
      ..Default::default()
    }).sync()
      .map_err(|err| DbError::Error(err.to_string()))
      .and_then(|output| Ok(
        // The unwrapping below should be safe, as we're restoring the struct from an existing record
        Self::new(output.attributes.unwrap()).unwrap()
      ))
  }

  fn delete(db: &DynamoDbClient, key: PrimaryKey, id: SortKey) -> Result<(), DbError> {
    db.delete_item(DeleteItemInput {
      key: hashmap!{
        String::from("primary_key") => attribute_value(key),
        String::from("id") => attribute_value(id),
      },
      table_name: COMMENTABLE_RS_TABLE_NAME.to_string(),
      ..Default::default()
    }).sync()
      .map_err(|err| DbError::Error(err.to_string()))?;

    Ok(())
  }

  fn batch_delete(db: &DynamoDbClient, mut keys: Vec<(PrimaryKey, SortKey)>) -> Result<(), DbError> {
    let mut request_items: HashMap<String, Vec<WriteRequest>> = hashmap!{
      String::from(COMMENTABLE_RS_TABLE_NAME) =>
        keys
          .drain(..)
          .map(|(primary_key, sort_key)| WriteRequest {
            delete_request: Some(DeleteRequest {
              key: hashmap!{
                String::from("primary_key") => attribute_value(primary_key),
                String::from("id") => attribute_value(sort_key),
              }
            }),
            ..Default::default()
          }).collect(),
    };
    // Each request can delete max 25 items, we add 2 to account for any unexpected DB or Network errors
    let max_iterations = request_items.get(COMMENTABLE_RS_TABLE_NAME).unwrap().len() as f32 / 25.0 + 2.0;
    let mut current_iteration = 0.0;

    'pagination: loop {
      current_iteration += 1.0;

      db.batch_write_item(BatchWriteItemInput {
        request_items: request_items.clone(),
        ..Default::default()
      }).sync()
        .map_err(|err| DbError::Error(err.to_string()))
        .and_then(|output| {
          request_items = output.unprocessed_items.unwrap();
          Ok(())
        })?;

      if request_items.is_empty() || current_iteration > max_iterations {
        break 'pagination;
      }
    }

    Ok(())
  }

  fn json(&self) -> String {
    serde_json::to_string(&self).unwrap()
  }
}

// Trait for models that can implement #list (Comment & Reaction)
pub trait DynamoDbListableModel where Self: DynamoDbModel {
  fn id_prefix() -> String;

  fn list(db: &DynamoDbClient, commentable_id: CommentableId) -> Result<Vec<Self>, DbError> {
    Self::query(&db, QueryInput {
      table_name: COMMENTABLE_RS_TABLE_NAME.to_string(),
      key_condition_expression: String::from("primary_key = :v1 and begins_with(id, :v2)").into(),
      expression_attribute_values: hashmap!{
        String::from(":v1") => attribute_value(commentable_id),
        String::from(":v2") => attribute_value(Self::id_prefix()),
      }.into(),
      ..Default::default()
    })?
       .drain(..)
       .map(|attributes| Self::new(attributes))
       .collect::<Result<Vec<Self>, DbError>>()
  }
}

impl From<String> for IntoAttributeValue {
  fn from(string: String) -> Self {
    let attribute_value = AttributeValue {
      s: Some(string),
      ..Default::default()
    };
    IntoAttributeValue { attribute_value }
  }
}

impl From<bool> for IntoAttributeValue {
  fn from(value: bool) -> Self {
    let attribute_value = AttributeValue {
      bool: Some(value),
      ..Default::default()
    };
    IntoAttributeValue { attribute_value }
  }
}

impl From<IntoAttributeValue> for AttributeValue {
  fn from(wrapper: IntoAttributeValue) -> Self {
    wrapper.attribute_value
  }
}

pub fn hash(text: &str) -> String {
  let mut hasher = Sha3::sha3_256();
  hasher.input_str(text);
  hasher.result_str()
}

pub fn attribute_value<T: Into<IntoAttributeValue>>(value: T) -> AttributeValue {
  value // = Into<IntoAttributeValue>
    .into() // -> IntoAttributeValue
    .into() // -> AttributeValue
}
