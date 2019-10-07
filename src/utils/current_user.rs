use rusoto_dynamodb::DynamoDbClient;

use crate::{
  utils::{
    db::DynamoDbModel,
    http::{unauthorized, internal_server_error, HttpError},
  },
  models::user::{TOKEN_DELIMITER, User},
};

pub trait CurrentUser {
  fn db(&self) -> &DynamoDbClient;
  fn auth_token(&self) -> Option<String>;
  fn set_current_user(&mut self, user: Option<User>);

  fn user_id(&self) -> Result<String, HttpError> {
    Ok(format!("USER_{}",
      self.auth_token()
        .ok_or(unauthorized("Invalid access token."))?
        .split(TOKEN_DELIMITER)
        .next()
        .ok_or(unauthorized("Invalid access token."))?
    ))
  }

  fn fetch_current_user(&mut self) -> Result<&mut Self, HttpError> {
    match User::find(self.db(), self.user_id()?.clone(), self.user_id()?) {
      Ok(Some(user)) => {
        // The unwrap is safe, because self.user_id() already checks for token presence
        if user.auth_token == self.auth_token().unwrap() {
          self.set_current_user(Some(user));
        } else {
          return Err(unauthorized("Invalid access token."));
        }
      },
      Ok(None) => return Err(unauthorized("Invalid access token.")),
      Err(err) => return Err(internal_server_error(err)),
    };
    Ok(self)
  }

  fn try_fetch_current_user(&mut self) -> &mut Self {
    let _ = self.fetch_current_user();
    self
  }
}
