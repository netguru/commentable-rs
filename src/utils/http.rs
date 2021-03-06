use lambda_http::{Response, Body, http::StatusCode};

pub type HttpError = Response<Body>;

pub fn ok<T>(body: T) -> Response<Body>
where T: ToString {
  http_response(body.to_string(), StatusCode::OK)
}

pub fn bad_request<T>(body: T) -> Response<Body>
where T: ToString {
  http_response(body.to_string(), StatusCode::BAD_REQUEST)
}

pub fn unauthorized<T>(body: T) -> Response<Body>
where T: ToString {
  http_response(body.to_string(), StatusCode::UNAUTHORIZED)
}

pub fn forbidden<T>(body: T) -> Response<Body>
where T: ToString {
  http_response(body.to_string(), StatusCode::FORBIDDEN)
}

pub fn not_found<T>(body: T) -> Response<Body>
where T: ToString {
  http_response(body.to_string(), StatusCode::NOT_FOUND)
}

pub fn internal_server_error<T>(body: T) -> Response<Body>
where T: ToString {
  http_response(body.to_string(), StatusCode::INTERNAL_SERVER_ERROR)
}

fn http_response(body: String, status: StatusCode) -> Response<Body> {
  let mut builder = Response::builder();
  // Setup CORS
  builder.header("Access-Control-Allow-Origin", "*");
  builder.header("Access-Control-Allow-Headers", "Content-Type,Authorization");

  if body.is_empty() {
    builder.status(status).body(Body::Empty).unwrap()
  } else {
    builder.status(status).body(Body::Text(body)).unwrap()
  }
}

pub fn missing_path_param(param: &str) -> String {
  format!("Invalid path parameters: {} is required", param)
}

pub fn missing_request_param(param: &str) -> String {
  format!("Invalid request parameters: {} is required", param)
}
