use std::fmt;

use chrono::{DateTime, Utc};
use maplit::hashmap;
use rusoto_dynamodb::{
  DynamoDb,
  DynamoDbClient,
  QueryInput,
};
use serde::Serialize;

use crate::utils::db::{
  COMMENTABLE_RS_TABLE_NAME,
  DynamoDbModel,
  DynamoDbAttributes,
  DynamoDbRecord,
  DbError,
  attribute_value,
};

pub type CommentId = String;

#[derive(Serialize, Debug)]
pub struct Comment {
  pub primary_key: String,
  pub id: CommentId,
  pub user_id: String,
  pub replies_to: Option<String>,
  pub body: String,
  pub created_at: DateTime<Utc>,
}

impl DynamoDbModel for Comment {
  fn new(mut attributes: DynamoDbAttributes) -> Result<Comment, DbError> {
    Ok(Comment {
      primary_key: attributes.string("primary_key")?,
      id: attributes.string("id")?,
      user_id: attributes.string("user_id")?,
      replies_to: attributes.optional_string("replies_to"),
      body: attributes.string("body")?,
      created_at: attributes.timestamp("created_at")?,
    })
  }
}

impl Comment {
  pub fn list(db: &DynamoDbClient, commentable_id: String) -> Result<Vec<Self>, DbError> {
    db.query(QueryInput {
      table_name: COMMENTABLE_RS_TABLE_NAME.to_string(),
      key_condition_expression: String::from("primary_key = :value").into(),
      expression_attribute_values: hashmap!{
        String::from(":value") => attribute_value(commentable_id),
      }.into(),
      ..Default::default()
    }).sync()
      .map_err(|err| DbError::Error(err.to_string()))
      .and_then(|query_output|
        query_output
          .items
          .and_then(|mut comments|
            comments
              .drain(..)
              .map(|comment_attributes| Comment::new(comment_attributes))
              .collect::<Result<Vec<Self>, DbError>>()
              .into()
          )
          .unwrap_or(Ok(vec![]))
      )
  }
}

impl fmt::Display for Comment {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "\"{}\"", self.body)
  }
}
