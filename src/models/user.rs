use std::fmt;
use std::collections::HashSet;

use chrono::{DateTime, Utc};
use maplit::hashmap;
use rusoto_dynamodb::{
  DynamoDb,
  DynamoDbClient,
  BatchGetItemInput,
  KeysAndAttributes,
};
use serde::Serialize;

use crate::utils::db::{
  attribute_value,
  DynamoDbModel,
  DynamoDbAttributes,
  DynamoDbRecord,
  DbError,
  COMMENTABLE_RS_TABLE_NAME,
};

pub static TOKEN_DELIMITER: &str = "-=#=-";
pub type UserId = String;

#[derive(Serialize, Debug)]
pub struct User {
  pub primary_key: UserId,
  pub id: UserId,
  pub email: String,
  pub name: String,
  pub picture_url: String,
  pub auth_token: String,
  pub created_at: DateTime<Utc>,
}

impl DynamoDbModel for User {
  fn new(mut attributes: DynamoDbAttributes) -> Result<User, DbError> {
    Ok(User {
      primary_key: attributes.string("primary_key")?,
      id: attributes.string("id")?,
      email: attributes.string("email")?,
      name: attributes.string("name")?,
      auth_token: attributes.string("auth_token")?,
      picture_url: attributes.string("picture_url")?,
      created_at: attributes.timestamp("created_at")?
    })
  }
}

impl User {
  pub fn batch_get(db: &DynamoDbClient, mut ids: HashSet<&UserId>) -> Result<Vec<Self>, DbError> {
    let mut users: Vec<Self> = vec![];
    /* 100 is the maximum amount of records allowed
       per single BatchGetItem operation in DynamoDB */
    for slice in ids.drain().collect::<Vec<_>>().chunks(100) {
      users.append(
        &mut db.batch_get_item(BatchGetItemInput {
          request_items: hashmap! {
            String::from(COMMENTABLE_RS_TABLE_NAME) => KeysAndAttributes {
              keys: slice.iter().map(|id| hashmap! {
                String::from("primary_key") => attribute_value(id.to_string()),
                String::from("id") => attribute_value(id.to_string()),
              }).collect(),
              ..Default::default()
            }
          },
          ..Default::default()
        }).sync()
          .map_err(|err| DbError::Error(err.to_string()))?
          .responses.unwrap()
          .remove(COMMENTABLE_RS_TABLE_NAME).unwrap()
          .drain(..)
          .map(|user_attributes| User::new(user_attributes))
          .collect::<Result<Vec<Self>, DbError>>()?
      );
    }
    Ok(users)
  }
}

impl fmt::Display for User {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{} ({})", self.name, self.id)
  }
}
