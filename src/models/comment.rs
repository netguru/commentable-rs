use chrono::{DateTime, Utc};
use maplit::hashmap;
use rusoto_dynamodb::{
  DynamoDb,
  DynamoDbClient,
  QueryInput,
  UpdateItemInput,
};
use serde::Serialize;

use crate::models::user::UserId;
use crate::utils::db::{
  COMMENTABLE_RS_TABLE_NAME,
  REPLIES_INDEX_NAME,
  CommentableId,
  DynamoDbModel,
  DynamoDbListableModel,
  DynamoDbAttributes,
  DynamoDbRecord,
  DbError,
  attribute_value,
  hash,
};

pub type CommentId = String;

pub static COMMENT_ID_PREFIX: &str = "COMMENT_";

#[derive(Serialize, Debug)]
pub struct Comment {
  pub primary_key: CommentableId,
  pub id: CommentId,
  pub user_id: Option<UserId>,
  pub replies_to: Option<CommentId>,
  pub body: String,
  pub is_deleted: Option<bool>,
  pub created_at: DateTime<Utc>,
}

impl DynamoDbModel for Comment {
  fn new(mut attributes: DynamoDbAttributes) -> Result<Self, DbError> {
    Ok(Self {
      primary_key: attributes.string("primary_key")?,
      id: attributes.string("id")?,
      user_id: attributes.optional_string("user_id"),
      replies_to: attributes.optional_string("replies_to"),
      body: attributes.string("body")?,
      is_deleted: None,
      created_at: attributes.timestamp("created_at")?,
    })
  }
}

impl DynamoDbListableModel for Comment {
  fn id_prefix() -> String {
    COMMENT_ID_PREFIX.to_string()
  }
}

impl Comment {
  pub fn has_replies(&self, db: &DynamoDbClient) -> Result<bool, DbError> {
    let replies = Self::query(&db, QueryInput {
      table_name: COMMENTABLE_RS_TABLE_NAME.to_string(),
      index_name: Some(REPLIES_INDEX_NAME.to_string()),
      key_condition_expression: String::from("primary_key = :v1 and replies_to = :v2").into(),
      expression_attribute_values: hashmap!{
        String::from(":v1") => attribute_value(self.primary_key.clone()),
        String::from(":v2") => attribute_value(self.id.clone()),
      }.into(),
      ..Default::default()
    })?;

    if replies.len() > 0 { Ok(true) } else { Ok(false) }
  }

  pub fn erase(&mut self, db: &DynamoDbClient) -> Result<(), DbError> {
    db.update_item(UpdateItemInput {
      table_name: COMMENTABLE_RS_TABLE_NAME.to_string(),
      key: hashmap!{
        String::from("primary_key") => attribute_value(self.primary_key.clone()),
        String::from("id") => attribute_value(self.id.clone()),
      },
      update_expression: Some(String::from("SET is_deleted = :is_deleted, body = :body REMOVE user_id")),
      expression_attribute_values: Some(hashmap!{
        String::from(":is_deleted") => attribute_value(true),
        String::from(":body") => attribute_value("This comment has been deleted.".to_string()),
      }),
      ..Default::default()
    }).sync()
      .map_err(|err| DbError::Error(err.to_string()))
      .and_then(|_| {
        self.body = "This comment has been deleted.".to_string();
        self.is_deleted = Some(true);
        self.user_id = None;
        Ok(())
      })
  }
}

pub fn comment_id(commentable_id: &CommentableId, user_id: &UserId) -> String {
  let id = hash(&format!("{}{}{}", commentable_id, user_id, Utc::now().to_string()));
  format!("{}{}{}", COMMENT_ID_PREFIX, Utc::now().timestamp_millis(), id)
}
