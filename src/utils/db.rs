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

// A helper trait that allows for easier access into AttributeValue
// contents and handles necessary validations
pub trait DbValidate {
  fn string(&mut self, name: &str) -> Result<String, DbError>;
  fn timestamp(&mut self, name: &str) -> Result<DateTime<Utc>, DbError>;
}

impl DbValidate for DynamoDbAttributes {
  fn string(&mut self, name: &str) -> Result<String, DbError> {
    self.remove(name)
        .and_then(|value| value.s)
        .ok_or(DbError::RecordInvalid(format!("Missing field '{}'.", name)))
  }

  fn timestamp(&mut self, name: &str) -> Result<DateTime<Utc>, DbError> {
    self.remove(name) // -> Option<AttributeValue>
      .and_then(|value| value.s) // -> Option<String>
      .ok_or(DbError::RecordInvalid(format!("Missing field '{}'.", name))) // -> Result<String, DbError>
      .and_then(|string|
        DateTime::parse_from_rfc3339(&string).map_err(|_|
          DbError::Error(format!("Error parsing timestamps in field '{}'", name))
        ) // -> Result<DateTime<FixedOffset>, DbError>
      ) // -> Result<DateTime<FixedOffset>, ParseError>
      .and_then(|datetime| Ok(datetime.with_timezone(&Utc))) // -> Result<DateTime<Utc>, DbError>
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
pub trait DynamoDbRecord where Self: Sized + Serialize {
  fn table_name() -> String;
  // #new is used internally to create structs from DynamoDB records
  fn new(attributes: DynamoDbAttributes) -> Result<Self, DbError>;

  fn find<T: Into<IntoAttributeValue>>(db: &DynamoDbClient, id: T) -> Result<Option<Self>, DbError> {
    db.get_item(GetItemInput {
      key: hashmap!{ String::from("id") => id.into().into() },
      table_name: Self::table_name(),
      ..Default::default()
    }).sync()
      .map_err(|err| DbError::Error(err.to_string()))
      .and_then(|output| {
        Ok(output.item.and_then(|record| {
          // The unwrapping below is safe, as we're creating a struct from an existing record
          Some(Self::new(record).unwrap())
        }))
      })
  }

  fn create(db: &DynamoDbClient, attributes: IntoDynamoDbAttributes) -> Result<Self, DbError> {
    let attributes: DynamoDbAttributes = attributes.into();
    db.put_item(PutItemInput {
      item: attributes.clone(),
      table_name: Self::table_name(),
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
