use lambda_http::{lambda, Request, Response, Body, RequestExt};
use rusoto_core::Region;
use rusoto_dynamodb::{DynamoDbClient};
use serde::Deserialize;

use commentable_rs::utils::db::{DynamoDbModel, CommentableId};
use commentable_rs::utils::http::{ok, bad_request, forbidden, internal_server_error, HttpError};
use commentable_rs::utils::current_user::CurrentUser;
use commentable_rs::utils::current_comment::CurrentComment;
use commentable_rs::models::{
  user::{AuthToken, User},
  comment::{CommentId, Comment},
  reaction::Reaction,
};

#[derive(Deserialize)]
struct Params {
  auth_token: AuthToken,
  comment_id: CommentId,
}

struct DeleteComment {
  db: DynamoDbClient,
  commentable_id: CommentableId,
  params: Params,
  current_user: Option<User>,
  comment: Option<Comment>,
  has_replies: bool,
}

impl CurrentUser for DeleteComment {
  fn db(&self) -> &DynamoDbClient {
    &self.db
  }

  fn auth_token(&self) -> Option<AuthToken> {
    Some(self.params.auth_token.clone())
  }

  fn set_current_user(&mut self, user: Option<User>) {
    self.current_user = user;
  }
}

impl CurrentComment for DeleteComment {
  fn db(&self) -> &DynamoDbClient {
    &self.db
  }

  fn commentable_id(&self) -> CommentableId {
    self.commentable_id.clone()
  }

  fn comment_id(&self) -> CommentId {
    self.params.comment_id.clone()
  }

  fn set_current_comment(&mut self, comment: Comment) {
    self.comment = Some(comment);
  }
}

impl DeleteComment {
  pub fn respond_to(request: Request) -> Result<Response<Body>, HttpError> {
    if let Some(commentable_id) = request.path_parameters().get("id") {
      Self::new(request, commentable_id.to_string())?
        .validate_params()?
        .fetch_current_user()?
        .fetch_current_comment()?
        .authorize()?
        .check_replies()?
        .delete_or_erase()?
        .serialize()
    } else {
      Err(bad_request("Invalid path parameters: 'id' is required."))
    }
  }

  pub fn new(request: Request, commentable_id: CommentableId) -> Result<Self, HttpError> {
    if let Ok(Some(params)) = request.payload::<Params>() {
      Ok(Self {
        db: DynamoDbClient::new(Region::default()),
        comment: None,
        current_user: None,
        has_replies: false,
        commentable_id,
        params,
      })
    } else {
      Err(bad_request("Invalid parameters"))
    }
  }

  pub fn validate_params(&mut self) -> Result<&mut Self, HttpError> {
    if self.params.auth_token.trim().len() == 0 {
      Err(bad_request("Parameter 'auth_token' is required."))
    } else if self.params.comment_id.trim().len() == 0 {
      Err(bad_request("Parameter 'comment_id' is required."))
    } else {
      Ok(self)
    }
  }

  pub fn authorize(&mut self) -> Result<&mut Self, HttpError> {
    // The unwraps are safe because presence is guaranteed by calls
    // to #fetch_current_user and #fetch_current_comment
    if self.comment.as_ref().unwrap().user_id == Some(self.current_user.as_ref().unwrap().id.clone()) {
      Ok(self)
    } else {
      Err(forbidden("Cannot delete comment."))
    }
  }

  pub fn check_replies(&mut self) -> Result<&mut Self, HttpError> {
    match self.comment.as_ref().unwrap().has_replies(&self.db) {
      Ok(has_replies) => self.has_replies = has_replies,
      Err(error) => return Err(internal_server_error(error))
    }
    Ok(self)
  }

  pub fn delete_or_erase(&mut self) -> Result<&mut Self, HttpError> {
    if self.has_replies {
      self.comment.as_mut().unwrap().erase(&self.db)
        .map_err(|err| internal_server_error(err))?;
    } else {
      Comment::delete(&self.db, self.commentable_id.clone(), self.params.comment_id.clone())
        .map_err(|err| internal_server_error(err))?;
      self.comment = None;
    }
    Reaction::remove_all_for_comment(&self.db, self.commentable_id.clone(), self.params.comment_id.clone())
      .map_err(|err| internal_server_error(err))?;
    Ok(self)
  }

  pub fn serialize(&self) -> Result<Response<Body>, HttpError> {
    Ok(ok(serde_json::to_string(&self.comment).unwrap()))
  }
}

fn main() {
  lambda!(|request, _|
    DeleteComment::respond_to(request)
      .or_else(|error_response| Ok(error_response))
  );
}
