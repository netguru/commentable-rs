use std::fmt;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::utils::db::{DynamoDbAttributes, DynamoDbRecord, DbError, DbValidate};

#[derive(Serialize)]
pub struct User {
  id: String,
  name: String,
  email: String,
  picture_url: String,
  auth_token: String,
  created_at: DateTime<Utc>,
  updated_at: DateTime<Utc>
}

impl DynamoDbRecord for User {
  fn table_name() -> String {
    "Users".to_string()
  }

  fn new(mut attributes: DynamoDbAttributes) -> Result<User, DbError> {
    Ok(User {
      id: attributes.string("id")?,
      name: attributes.string("name")?,
      email: attributes.string("email")?,
      auth_token: attributes.string("auth_token")?,
      picture_url: attributes.string("picture_url")?,
      created_at: attributes.timestamp("created_at")?,
      updated_at: attributes.timestamp("updated_at")?,
    })
  }
}

impl fmt::Display for User {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{} ({})", self.name, self.email)
  }
}
