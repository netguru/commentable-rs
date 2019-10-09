use std::collections::{BTreeMap, HashMap};

use lambda_http::{lambda, Request, Response, Body, RequestExt};
use maplit::hashmap;
use rusoto_core::Region;
use rusoto_dynamodb::{DynamoDbClient};
use serde::{Serialize, Deserialize};

use commentable_rs::models::comment::{Comment as CommentRecord, CommentId};
use commentable_rs::models::user::{User, UserId};
use commentable_rs::models::reaction::{Reaction, ReactionType};
use commentable_rs::utils::current_user::CurrentUser;
use commentable_rs::utils::db::{CommentableId, DynamoDbModel};
use commentable_rs::utils::http::{ok, bad_request, internal_server_error, HttpError};

type ReactionCount = u16;

#[derive(Debug)]
struct Comment {
  id: CommentId,
  body: String,
  user_id: Option<UserId>,
  is_reply: bool,
  replies: Vec<CommentId>,
  reactions: HashMap<ReactionType, ReactionCount>,
  user_reactions: Vec<ReactionType>,
}

#[derive(Serialize, Clone)]
struct UserJson {
  id: String,
  name: String,
  picture_url: String,
}

#[derive(Serialize)]
struct CommentJson {
  id: CommentId,
  body: String,
  user: Option<UserJson>,
  replies: Vec<CommentJson>,
  reactions: HashMap<ReactionType, ReactionCount>,
  user_reactions: Vec<ReactionType>,
}

#[derive(Deserialize)]
struct Params {
  auth_token: Option<String>,
}

struct ListComments {
  db: DynamoDbClient,
  params: Params,
  comments: BTreeMap<CommentId, Comment>,
  users: HashMap<UserId, UserJson>,
  current_user: Option<User>,
}

impl CurrentUser for ListComments {
  fn db(&self) -> &DynamoDbClient {
    &self.db
  }

  fn auth_token(&self) -> Option<String> {
    self.params.auth_token.clone()
  }

  fn set_current_user(&mut self, user: Option<User>) {
    self.current_user = user;
  }
}

impl ListComments {
  pub fn respond_to(request: Request) -> Result<Response<Body>, HttpError> {
    if let Some(commentable_id) = request.path_parameters().get("id") {
      Self::new(request)?
        .try_fetch_current_user()
        .fetch_comments(commentable_id.to_string())?
        .fetch_users()?
        .fetch_reactions(commentable_id.to_string())?
        .serialize()
    } else {
      Err(bad_request("Invalid params: 'id' is required."))
    }
  }

  pub fn new(request: Request) -> Result<Self, HttpError> {
    if let Ok(params) = request.payload::<Params>() {
      Ok(Self {
        db: DynamoDbClient::new(Region::default()),
        comments: BTreeMap::new(),
        users: HashMap::new(),
        current_user: None,
        params: params.unwrap_or(Params { auth_token: None }),
      })
    } else {
      Err(bad_request("Invalid parameters"))
    }
  }

  pub fn fetch_comments(&mut self, commentable_id: CommentableId) -> Result<&mut Self, HttpError> {
    match CommentRecord::list(&self.db, commentable_id) {
      Ok(comments) => self.parse_comments(comments),
      Err(err) => Err(internal_server_error(err)),
    }
  }

  pub fn fetch_reactions(&mut self, commentable_id: CommentableId) -> Result<&mut Self, HttpError> {
    match Reaction::list(&self.db, commentable_id) {
      Ok(reactions) => self.parse_reactions(reactions),
      Err(err) => Err(internal_server_error(err)),
    }
  }

  pub fn fetch_users(&mut self) -> Result<&mut Self, HttpError> {
    let user_ids = self.comments.values().filter_map(|comment| comment.user_id.as_ref()).collect();
    println!("{:?}", user_ids);
    match User::batch_get(&self.db, user_ids) {
      Ok(users) => self.parse_users(users),
      Err(err) => Err(internal_server_error(err)),
    }
  }

  pub fn serialize(&mut self) -> Result<Response<Body>, HttpError> {
    let serializable_comments = self.comments
      .values()
      .filter(|comment| !comment.is_reply)
      .map(|comment| self.serialize_comment(comment))
      .collect::<Result<Vec<CommentJson>, HttpError>>()?;

    serde_json::to_string(&hashmap! {
      String::from("data") => serializable_comments,
    }).map_err(|err| internal_server_error(err))
      .and_then(|results| Ok(ok(results)))
  }

  fn parse_comments(&mut self, comments: Vec<CommentRecord>) -> Result<&mut Self, HttpError> {
    for comment in comments {
      let mut is_reply = false;
      // Check if the comment is a reply
      if let Some(parent_id) = comment.replies_to.as_ref() {
        is_reply = true;
        if let Some(parent) = self.comments.get_mut(parent_id) {
          parent.replies.push(comment.id.clone());
        } else {
          // Remove orphaned comment, but don't care if it fails
          // TODO: Add error reporting (Rollbar-like) to this operation
          let _ = CommentRecord::delete(&self.db, comment.primary_key.clone(), comment.id.clone());
        }
      }
      self.comments.insert(comment.id.clone(), Comment {
        id: comment.id,
        user_id: comment.user_id,
        body: comment.body,
        replies: vec![],
        reactions: hashmap!{},
        user_reactions: vec![],
        is_reply,
      });
    }
    Ok(self)
  }

  fn parse_users(&mut self, mut users: Vec<User>) -> Result<&mut Self, HttpError> {
    self.users = users
      .drain(..)
      .map(|user| (user.id.clone(), UserJson { id: user.id, name: user.name, picture_url: user.picture_url }))
      .collect::<HashMap<UserId, UserJson>>();
    Ok(self)
  }

  fn parse_reactions(&mut self, reactions: Vec<Reaction>) -> Result<&mut Self, HttpError> {
    for reaction in reactions {
      let mut is_user_reaction = false;
      if let Some(current_user) = &self.current_user {
        if reaction.user_id == current_user.id {
          is_user_reaction = true;
        }
      }
      if let Some(comment) = self.comments.get_mut(&reaction.comment_id) {
        *comment.reactions.entry(reaction.reaction_type.clone()).or_insert(0) += 1;
        if is_user_reaction {
          (*comment).user_reactions.push(reaction.reaction_type);
        }
      }
    }
    Ok(self)
  }

  fn serialize_comment(&self, comment: &Comment) -> Result<CommentJson, HttpError> {
    Ok(CommentJson {
      id: comment.id.clone(),
      body: comment.body.clone(),
      user: match comment.user_id.as_ref() {
        Some(user_id) =>
          Some(self.users
            .get(user_id)
            .ok_or(internal_server_error(format!(
              "Couldn't find a user with ID: {}. Reference in comment: {:?}",
              &user_id,
              &comment,
            )))?
            .clone()),
        None => None,
      },
      replies: comment.replies
        .iter()
        .map(|id| self.serialize_comment(self.comments.get(id).unwrap())) // safe unwrap
        .collect::<Result<Vec<CommentJson>, HttpError>>()?,
      reactions: comment.reactions.clone(),
      user_reactions: comment.user_reactions.clone(),
    })
  }
}

fn main() {
  lambda!(|request, _|
    ListComments::respond_to(request)
      .or_else(|error_response| Ok(error_response))
  );
}
