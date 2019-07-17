use std::collections::{BTreeMap, HashMap};

use lambda_http::{lambda, Request, Response, Body, RequestExt};
use rusoto_core::Region;
use rusoto_dynamodb::{DynamoDbClient};
use serde::Serialize;

use ::commentable_rs::models::comment::Comment;
use ::commentable_rs::utils::http::{ok, bad_request, internal_server_error};

#[derive(Serialize)]
struct CommentJson {
  id: String,
  body: String,
  replies: BTreeMap<String, CommentJson>
}

pub fn list_comments(request: Request) -> Response<Body> {
  if let Some(commentable_id) = request.path_parameters().get("id") {
    let db = DynamoDbClient::new(Region::default());
    match Comment::list(&db, commentable_id.to_string()) {
      Ok(comments) => {
        let mut comments_json: BTreeMap<String, CommentJson> = BTreeMap::new();
        // parents_tree maps a comment_id to a sorted list of it's parents' ids.
        // It doesn't contain top-level comments.
        let mut parents_tree: HashMap<String, Vec<String>> = HashMap::new();
        for comment in comments {
          // If the comment is a reply
          if let Some(parent_id) = comment.replies_to {
            // Check if it's a reply to a top-level comment
            if let Some(ids) = parents_tree.get(&parent_id) {
              // Iterate over the parent ids list to find the current comment's direct parent
              let mut current_parent = comments_json.get_mut(&ids[0]).unwrap();
              for id in &ids[1..] {
                if let Some(next_parent) = current_parent.replies.get_mut(id) {
                  current_parent = next_parent;
                } else {
                  return internal_server_error("Corrupted comment".to_string());
                }
              }
              (*current_parent).replies.insert(comment.id.clone(), CommentJson {
                id: comment.id,
                body: comment.body,
                replies: BTreeMap::new(),
              });
            } else {
              if let Some(parent) = comments_json.get_mut(&parent_id) {
                parents_tree.insert(comment.id.clone(), vec![parent_id.clone()]);
                parent.replies.insert(comment.id.clone(), CommentJson {
                  id: comment.id,
                  body: comment.body,
                  replies: BTreeMap::new(),
                });
              } else {
                return internal_server_error("Corrupted comment".to_string());
              }
            }
          } else {
            comments_json.insert(comment.id.clone(), CommentJson {
              id: comment.id,
              body: comment.body,
              replies: BTreeMap::new(),
            });
          }
        }
        ok(serde_json::to_string(&comments_json).unwrap())
      },
      Err(err) => internal_server_error(err.to_string()),
    }
  } else {
    bad_request("Invalid params: 'id' is required.".to_string())
  }
}

fn main() {
  lambda!(|request, _| Ok(list_comments(request)));
}
