use std::collections::HashMap;

use chrono::{DateTime, Utc};
use crypto::digest::Digest;
use crypto::sha3::Sha3;
use maplit::hashmap;
use rusoto_dynamodb::{
  DynamoDb,
  DynamoDbClient,
  GetItemInput,
  PutItemInput,
  AttributeValue
};
use serde::Serialize;

pub static COMMENTABLE_RS_TABLE_NAME: &str = "CommentableRsTable";

#[derive(Debug)]
pub enum DbError {
  Error(String),
  RecordInvalid(String),
}

impl ToString for DbError {
  fn to_string(&self) -> String {
    match self {
      DbError::Error(msg) => format!("DbError::Error -> {}", msg),
      DbError::RecordInvalid(msg) => format!("DbError::RecordInvalid -> {}", msg),
    }
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

  fn find<T: Into<IntoAttributeValue>>(db: &DynamoDbClient, key: T, id: T) -> Result<Option<Self>, DbError> {
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
        Ok(output.item.and_then(|record| {
          // The unwrapping below is safe (is it?), as we're restoring a struct from an existing record
          Some(Self::new(record).unwrap())
        }))
      })
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

  fn json(&self) -> String {
    serde_json::to_string(&self).unwrap()
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
