use chrono::Utc;
use lambda_http::{lambda, Request, Response, Body, RequestExt};
use maplit::hashmap;
use rusoto_core::Region;
use rusoto_dynamodb::{DynamoDbClient};
use serde::Deserialize;

use ::commentable_rs::utils::http::{ok, bad_request, unauthorized, internal_server_error};
use ::commentable_rs::utils::db::{hash, DynamoDbModel, IntoDynamoDbAttributes};
use ::commentable_rs::models::user::{User, TOKEN_DELIMITER};

#[derive(Deserialize)]
struct Params {
  id_token: String,
}

#[derive(Deserialize)]
struct AuthData {
  email: String,
  name: String,
  picture: String,
}

impl From<AuthData> for IntoDynamoDbAttributes {
  fn from(auth_data: AuthData) -> Self {
    let user_id = hash(&auth_data.email);
    IntoDynamoDbAttributes {
      attributes: hashmap!{
        String::from("primary_key") => format!("USER_{}", user_id).into(),
        String::from("id") => format!("USER_{}", user_id).into(),
        String::from("email") => auth_data.email.into(),
        String::from("name") => auth_data.name.into(),
        String::from("picture_url") => auth_data.picture.into(),
        String::from("auth_token") => format!("{}{}{}",
          user_id,
          TOKEN_DELIMITER,
          hash(&Utc::now().to_string()),
        ).into(),
        String::from("created_at") => Utc::now().to_rfc3339().into(),
      }
    }
  }
}

pub fn auth(request: Request) -> Response<Body> {
  if let Ok(Some(params)) = request.payload::<Params>() {
    let url = format!("https://oauth2.googleapis.com/tokeninfo?id_token={}", params.id_token);
    // Validate the token using Google API
    match reqwest::get(&url) {
      Ok(mut response) => {
        if let Ok(google_user) = response.json::<AuthData>() {
          let db = DynamoDbClient::new(Region::default());
          // Look for an existing user (id = hashed email)
          match User::find(&db, format!("USER_{}", hash(&google_user.email)), google_user.email.clone()) {
            Ok(Some(user)) => ok(user.json()),
            // Create a new user
            Ok(None) => match User::create(&db, google_user.into()) {
              Ok(user) => ok(user.json()),
              Err(err) => internal_server_error(format!("Error creating a user: {}", err)),
            },
            Err(err) => internal_server_error(format!("Error finding a user: {}", err)),
          }
        } else {
          unauthorized("Invalid id_token")
        }
      },
      Err(error) => internal_server_error(error),
    }
  } else {
    bad_request("Invalid params.")
  }
}

fn main() {
  lambda!(|request, _| Ok(auth(request)));
}
