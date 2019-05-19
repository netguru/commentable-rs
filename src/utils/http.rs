use lambda_http::{Response, Body, http::StatusCode};

pub fn ok(body: String) -> Response<Body> {
  http_response(body, StatusCode::OK)
}

pub fn bad_request(body: String) -> Response<Body> {
  http_response(body, StatusCode::BAD_REQUEST)
}

pub fn unauthorized(body: String) -> Response<Body> {
  http_response(body, StatusCode::UNAUTHORIZED)
}

pub fn not_found(body: String) -> Response<Body> {
  http_response(body, StatusCode::NOT_FOUND)
}

pub fn internal_server_error(body: String) -> Response<Body> {
  http_response(body, StatusCode::INTERNAL_SERVER_ERROR)
}

fn http_response(body: String, status: StatusCode) -> Response<Body> {
  let mut builder = Response::builder();
  if body.is_empty() {
    builder.status(status).body(Body::Empty).unwrap()
  } else {
    builder.status(status).body(Body::Text(body)).unwrap()
  }
}
