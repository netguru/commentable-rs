use rusoto_dynamodb::DynamoDbClient;

use crate::{
  utils::{
    db::DynamoDbModel,
    http::{not_found, internal_server_error, HttpError},
  },
  models::comment::Comment,
};

pub trait CurrentComment {
  fn db(&self) -> &DynamoDbClient;
  fn comment_id(&self) -> String;
  fn set_current_comment(&mut self, comment: Comment);

  fn fetch_current_comment(&mut self, commentable_id: String) -> Result<&mut Self, HttpError> {
    match Comment::find(self.db(), commentable_id, self.comment_id()) {
      Ok(Some(comment)) => self.set_current_comment(comment),
      Ok(None) => return Err(not_found("Comment not found")),
      Err(err) => return Err(internal_server_error(err)),
    }
    Ok(self)
  }
}
