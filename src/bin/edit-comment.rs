use maplit::hashmap;
use lambda_http::{lambda, Request, Response, Body, RequestExt};
use rusoto_core::Region;
use rusoto_dynamodb::{DynamoDbClient};
use serde::Deserialize;

use ::commentable_rs::utils::db::{attribute_value, DynamoDbModel, CommentableId};
use ::commentable_rs::utils::http::{
  bad_request,
  forbidden,
  internal_server_error,
  missing_request_param,
  missing_path_param,
  ok,
  HttpError,
};
use ::commentable_rs::utils::current_user::CurrentUser;
use ::commentable_rs::utils::current_comment::CurrentComment;
use ::commentable_rs::models::{
  user::{AuthToken, User},
  comment::{CommentId, Comment},
};

#[derive(Deserialize)]
struct Params {
  auth_token: AuthToken,
  comment_id: CommentId,
  body: String,
}

struct EditComment {
  db: DynamoDbClient,
  commentable_id: CommentableId,
  params: Params,
  current_user: Option<User>,
  current_comment: Option<Comment>,
}

impl CurrentUser for EditComment {
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

impl CurrentComment for EditComment {
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
    self.current_comment = Some(comment);
  }
}

impl EditComment {
  pub fn respond_to(request: Request) -> Result<Response<Body>, HttpError> {
    if let Some(commentable_id) = request.path_parameters().get("id") {
      Self::new(request, commentable_id.to_string())?
        .validate_params()?
        .fetch_current_user()?
        .fetch_current_comment()?
        .authorize()?
        .update()?
        .serialize()
    } else {
      Err(bad_request(missing_path_param("id")))
    }
  }

  pub fn new(request: Request, commentable_id: CommentableId) -> Result<Self, HttpError> {
    if let Ok(Some(params)) = request.payload::<Params>() {
      Ok(Self {
        db: DynamoDbClient::new(Region::default()),
        current_comment: None,
        current_user: None,
        commentable_id,
        params,
      })
    } else {
      Err(bad_request("Invalid parameters"))
    }
  }

  pub fn validate_params(&mut self) -> Result<&mut Self, HttpError> {
    if self.params.auth_token.trim().len() == 0 {
      Err(bad_request(missing_request_param("auth_token")))
    } else if self.params.comment_id.trim().len() == 0 {
      Err(bad_request(missing_request_param("comment_id")))
    } else if self.params.body.trim().len() == 0 {
      Err(bad_request(missing_request_param("body")))
    } else {
      Ok(self)
    }
  }

  pub fn authorize(&mut self) -> Result<&mut Self, HttpError> {
    // The unwraps are safe because presence is guaranteed by calls
    // to #fetch_current_user and #fetch_current_comment
    if self.current_comment.as_ref().unwrap().user_id == Some(self.current_user.as_ref().unwrap().id.clone()) {
      Ok(self)
    } else {
      Err(forbidden("Cannot update comment"))
    }
  }

  pub fn update(&mut self) -> Result<&mut Self, HttpError> {
    match Comment::update(
      &self.db,
      self.commentable_id.clone(),
      self.comment_id(),
      "SET body = :body".to_owned(),
      hashmap!{ String::from(":body") => attribute_value(self.params.body.clone()) },
    ) {
      Ok(updated_comment) => {
        self.current_comment = Some(updated_comment);
        Ok(self)
      },
      Err(err) => Err(internal_server_error(err)),
    }
  }

  pub fn serialize(&self) -> Result<Response<Body>, HttpError> {
    Ok(ok(serde_json::to_string(&self.current_comment).unwrap()))
  }
}

fn main() {
  lambda!(|request, _|
    EditComment::respond_to(request)
      .or_else(|error_response| Ok(error_response))
  );
}
