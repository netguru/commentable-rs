use chrono::{DateTime, Utc};
use maplit::hashmap;
use rusoto_dynamodb::{
  DynamoDbClient,
  QueryInput,
};
use serde::Serialize;

use crate::models::comment::CommentId;
use crate::models::user::UserId;
use crate::utils::db::{
  COMMENTABLE_RS_TABLE_NAME,
  REACTIONS_INDEX_NAME,
  CommentableId,
  DynamoDbModel,
  DynamoDbListableModel,
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

impl DynamoDbListableModel for Reaction {
  fn id_prefix() -> String {
    REACTION_ID_PREFIX.to_string()
  }
}

impl Reaction {
  pub fn remove_all_for_comment(db: &DynamoDbClient, commentable_id: CommentableId, comment_id: CommentId) -> Result<(), DbError> {
    let reactions =
      Self::query(&db, QueryInput {
        table_name: COMMENTABLE_RS_TABLE_NAME.to_string(),
        index_name: Some(REACTIONS_INDEX_NAME.to_string()),
        key_condition_expression: String::from("primary_key = :v1 and comment_id = :v2").into(),
        expression_attribute_values: hashmap!{
          String::from(":v1") => attribute_value(commentable_id),
          String::from(":v2") => attribute_value(comment_id),
        }.into(),
        ..Default::default()
      })?.drain(..)
         .map(|mut key: DynamoDbAttributes| (key.string("primary_key").unwrap(), key.string("id").unwrap()))
         .collect::<Vec<(CommentableId, ReactionId)>>();

    if reactions.len() > 0 {
      Reaction::batch_delete(&db, reactions)
    } else {
      Ok(())
    }
  }
}
