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
  if body.is_empty() {
    builder.status(status).body(Body::Empty).unwrap()
  } else {
    builder.status(status).body(Body::Text(body)).unwrap()
  }
}
