use chrono::Utc;
use lambda_http::{lambda, Request, Response, Body, RequestExt};
use maplit::hashmap;
use rusoto_core::Region;
use rusoto_dynamodb::{DynamoDbClient};
use serde::Deserialize;

use ::commentable_rs::utils::db::{CommentableId, DynamoDbModel, IntoDynamoDbAttributes};
use ::commentable_rs::utils::http::{ok, bad_request, internal_server_error, HttpError};
use ::commentable_rs::utils::current_comment::CurrentComment;
use ::commentable_rs::utils::current_user::CurrentUser;
use ::commentable_rs::models::{
  user::{AuthToken, User, UserId},
  comment::{Comment, CommentId},
  reaction::{reaction_id, Reaction, ReactionType},
};

#[derive(Deserialize)]
struct Params {
  auth_token: AuthToken,
  comment_id: CommentId,
  reaction_type: ReactionType,
}

struct AddReaction {
  db: DynamoDbClient,
  commentable_id: CommentableId,
  params: Params,
  current_user: Option<User>,
  current_comment: Option<Comment>,
  reaction: Option<Reaction>,
}

impl CurrentUser for AddReaction {
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

impl CurrentComment for AddReaction {
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

impl AddReaction {
  pub fn respond_to(request: Request) -> Result<Response<Body>, HttpError> {
    if let Some(commentable_id) = request.path_parameters().get("id") {
      Self::new(request, commentable_id.to_string())?
        .validate_params()?
        .fetch_current_user()?
        .fetch_current_comment()?
        .validate_reaction()?
        .save()?
        .serialize()
    } else {
      Err(bad_request("Invalid path parameters: 'id' is required."))
    }
  }

  pub fn new(request: Request, commentable_id: CommentableId) -> Result<Self, HttpError> {
    if let Ok(Some(params)) = request.payload::<Params>() {
      Ok(Self {
        db: DynamoDbClient::new(Region::default()),
        current_comment: None,
        current_user: None,
        reaction: None,
        commentable_id,
        params,
      })
    } else {
      Err(bad_request("Invalid parameters"))
    }
  }

  fn current_user_id(&self) -> &UserId {
    &self.current_user.as_ref().unwrap().id
  }

  fn current_comment_id(&self) -> &CommentId {
    &self.current_comment.as_ref().unwrap().id
  }

  pub fn validate_params(&mut self) -> Result<&mut Self, HttpError> {
    if self.params.auth_token.trim().len() == 0 {
      Err(bad_request("Invalid request parameters: auth_token is required"))
    } else if self.params.comment_id.trim().len() == 0 {
      Err(bad_request("Invalid request parameters: comment_id is required"))
    } else if self.params.reaction_type.trim().len() == 0 {
      Err(bad_request("Invalid request parameters: reaction_type is required"))
    } else {
      Ok(self)
    }
  }

  pub fn validate_reaction(&mut self) -> Result<&mut Self, HttpError> {
    let reaction_id = reaction_id(self.current_comment_id(), self.current_user_id(), &self.params.reaction_type);

    match Reaction::find(&self.db, self.commentable_id.clone(), reaction_id) {
      Ok(None) => Ok(self),
      Ok(Some(_)) => Err(bad_request("Reaction already exists")),
      Err(err) => Err(internal_server_error(err)),
    }
  }

  pub fn save(&mut self) -> Result<&mut Self, HttpError> {
    let reaction_id = reaction_id(self.current_comment_id(), self.current_user_id(), &self.params.reaction_type);

    let attributes = IntoDynamoDbAttributes {
      attributes: hashmap!{
        String::from("primary_key") => self.commentable_id.clone().into(),
        String::from("id") => reaction_id.into(),
        String::from("user_id") => self.current_user_id().to_owned().into(),
        String::from("comment_id") => self.current_comment_id().to_owned().into(),
        String::from("type") => self.params.reaction_type.clone().into(),
        String::from("created_at") => Utc::now().to_rfc3339().into(),
      }
    };

    match Reaction::create(&self.db, attributes) {
      Ok(reaction) => {
        self.reaction = Some(reaction);
        Ok(self)
      },
      Err(err) => Err(internal_server_error(err))
    }
  }

  pub fn serialize(&mut self) -> Result<Response<Body>, HttpError> {
    // The unwrap is safe because we check for comment presence in #save
    Ok(ok(self.reaction.as_ref().unwrap().json()))
  }
}

fn main() {
  lambda!(|request, _|
    AddReaction::respond_to(request)
      .or_else(|error_response| Ok(error_response))
  );
}
