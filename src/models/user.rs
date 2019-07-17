use std::fmt;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::utils::db::{DynamoDbModel, DynamoDbAttributes, DbError, DynamoDbRecord};

#[derive(Serialize)]
pub struct User {
  primary_key: String,
  id: String,
  name: String,
  picture_url: String,
  auth_token: String,
  created_at: DateTime<Utc>,
}

impl DynamoDbModel for User {
  fn new(mut attributes: DynamoDbAttributes) -> Result<User, DbError> {
    Ok(User {
      primary_key: attributes.string("primary_key")?,
      id: attributes.string("id")?,
      name: attributes.string("name")?,
      auth_token: attributes.string("auth_token")?,
      picture_url: attributes.string("picture_url")?,
      created_at: attributes.timestamp("created_at")?
    })
  }
}

impl fmt::Display for User {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{} ({})", self.name, self.id)
  }
}
