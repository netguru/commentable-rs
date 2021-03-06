use std::collections::HashMap;

use chrono::Utc;
use lambda_http::{lambda, Request, Response, Body, RequestExt};
use maplit::hashmap;
use rusoto_core::Region;
use rusoto_dynamodb::{DynamoDbClient};
use serde::{Serialize, Deserialize};

use commentable_rs::utils::db::{CommentableId, DynamoDbModel, IntoDynamoDbAttributes};
use commentable_rs::utils::http::{ok, bad_request, internal_server_error, HttpError};
use commentable_rs::utils::current_user::CurrentUser;
use commentable_rs::models::{
  user::{AuthToken, User, UserId},
  comment::{comment_id, Comment, CommentId},
};

#[derive(Deserialize)]
struct Params {
  auth_token: AuthToken,
  replies_to: Option<CommentId>,
  body: String,
}

struct AddComment {
  db: DynamoDbClient,
  commentable_id: CommentableId,
  params: Params,
  current_user: Option<User>,
  comment: Option<Comment>,
}

#[derive(Serialize, Clone)]
struct UserJson {
  id: UserId,
  name: String,
  picture_url: String,
}

#[derive(Serialize)]
struct CommentJson {
  id: CommentId,
  body: String,
  user: Option<UserJson>,
  replies: Vec<()>,
  reactions: HashMap<(), ()>,
  user_reactions: Vec<()>,
  created_at: String,
}

impl CurrentUser for AddComment {
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

impl AddComment {
  pub fn respond_to(request: Request) -> Result<Response<Body>, HttpError> {
    if let Some(commentable_id) = request.path_parameters().get("id") {
      Self::new(request, commentable_id.to_string())?
        .validate()?
        .fetch_current_user()?
        .check_reply()?
        .save()?
        .serialize()
    } else {
      Err(bad_request("Invalid params: 'id' is required."))
    }
  }

  pub fn new(request: Request, commentable_id: CommentableId) -> Result<Self, HttpError> {
    if let Ok(Some(params)) = request.payload::<Params>() {
      Ok(Self {
        db: DynamoDbClient::new(Region::default()),
        comment: None,
        current_user: None,
        commentable_id,
        params,
      })
    } else {
      Err(bad_request("Invalid parameters"))
    }
  }

  pub fn validate(&mut self) -> Result<&mut Self, HttpError> {
    if self.params.auth_token.trim().len() == 0 {
      Err(bad_request("auth_token is required"))
    } else if self.params.body.trim().len() == 0 {
      Err(bad_request("body is required"))
    } else {
      Ok(self)
    }
  }

  pub fn check_reply(&mut self) -> Result<&mut Self, HttpError> {
    if let Some(comment_id) = &self.params.replies_to {
      match Comment::find(&self.db, self.commentable_id.clone(), comment_id.clone()) {
        Ok(Some(_)) => Ok(self),
        Ok(None) => Err(bad_request("replies_to is not a valid comment ID")),
        Err(err) => Err(internal_server_error(err)),
      }
    } else {
      Ok(self)
    }
  }

  pub fn save(&mut self) -> Result<&mut Self, HttpError> {
    let current_user_id = &self.current_user.as_ref().unwrap().id;
    let mut attributes = IntoDynamoDbAttributes {
      attributes: hashmap!{
        String::from("primary_key") => self.commentable_id.clone().into(),
        String::from("id") => comment_id(&self.commentable_id, current_user_id).into(),
        String::from("user_id") => current_user_id.clone().into(),
        String::from("body") => self.params.body.clone().into(),
        String::from("created_at") => Utc::now().to_rfc3339().into(),
      }
    };
    // String::from("replies_to") = self.params.replies_to.clone().into(),
    if let Some(parent_comment_id) = self.params.replies_to.clone() {
      attributes.attributes.insert(String::from("replies_to"), parent_comment_id.into());
    }
    match Comment::create(&self.db, attributes) {
      Ok(comment) => {
        self.comment = Some(comment);
        Ok(self)
      },
      Err(err) => Err(internal_server_error(err))
    }
  }

  pub fn serialize(&mut self) -> Result<Response<Body>, HttpError> {
    // The unwraps are safe because we check for comment presence in #save
    let comment = self.comment.as_ref().unwrap();
    let user = self.current_user.as_ref().unwrap();
    Ok(ok(serde_json::to_string(&CommentJson {
      id: comment.id.clone(),
      body: comment.body.clone(),
      user: Some(UserJson {
        id: user.id.clone(),
        name: user.name.clone(),
        picture_url: user.picture_url.clone(),
      }),
      replies: vec![],
      reactions: hashmap!{},
      user_reactions: vec![],
      created_at: comment.created_at.to_string(),
    }).unwrap()))
  }
}

fn main() {
  lambda!(|request, _|
    AddComment::respond_to(request)
      .or_else(|error_response| Ok(error_response))
  );
}
