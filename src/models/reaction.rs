use chrono::{DateTime, Utc};
use maplit::hashmap;
use rusoto_dynamodb::{
  DynamoDb,
  DynamoDbClient,
  QueryInput,
};
use serde::Serialize;

use crate::models::comment::CommentId;
use crate::models::user::UserId;
use crate::utils::db::{
  COMMENTABLE_RS_TABLE_NAME,
  CommentableId,
  DynamoDbModel,
  DynamoDbAttributes,
  DynamoDbRecord,
  DbError,
  attribute_value,
};

pub type ReactionId = String;
pub type ReactionType = String;

pub static REACTION_ID_PREFIX: &str = "REACTION_";

#[derive(Serialize, Debug)]
pub struct Reaction {
  pub primary_key: CommentableId,
  pub id: ReactionId,
  pub user_id: UserId,
  pub comment_id: CommentId,
  pub reaction_type: ReactionType,
  pub created_at: DateTime<Utc>,
}

impl DynamoDbModel for Reaction {
  fn new(mut attributes: DynamoDbAttributes) -> Result<Self, DbError> {
    Ok(Self {
      primary_key: attributes.string("primary_key")?,
      id: attributes.string("id")?,
      user_id: attributes.string("user_id")?,
      comment_id: attributes.string("comment_id")?,
      reaction_type: attributes.string("type")?,
      created_at: attributes.timestamp("created_at")?,
    })
  }
}

impl Reaction {
  pub fn list(db: &DynamoDbClient, commentable_id: CommentableId) -> Result<Vec<Self>, DbError> {
    db.query(QueryInput {
      table_name: COMMENTABLE_RS_TABLE_NAME.to_string(),
      key_condition_expression: String::from("primary_key = :v1 and begins_with(id, :v2)").into(),
      expression_attribute_values: hashmap!{
        String::from(":v1") => attribute_value(commentable_id),
        String::from(":v2") => attribute_value(REACTION_ID_PREFIX.to_string()),
      }.into(),
      ..Default::default()
    }).sync()
      .map_err(|err| DbError::Error(err.to_string()))
      .and_then(|query_output|
        query_output
          .items
          .and_then(|mut replies|
            replies
              .drain(..)
              .map(|reply_attributes| Self::new(reply_attributes))
              .collect::<Result<Vec<Self>, DbError>>()
              .into()
          )
          .unwrap_or(Ok(vec![]))
      )
  }
}
