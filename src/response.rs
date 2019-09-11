use std::fmt;

use tide::response::{IntoResponse, Response, WithStatus};
use tide::http::header::CONTENT_TYPE;
use tide::http::StatusCode;

pub struct Json<R=Vec<u8>>(pub R);

impl<R: IntoResponse> IntoResponse for Json<R> {
    fn into_response(self) -> Response {
        let mut response = self.0.into_response();
        response.headers_mut().insert(CONTENT_TYPE, "application/json".parse().unwrap());
        response
    }
}

pub fn into_internal_error<E: fmt::Display>(e: E) -> WithStatus<String> {
    e.to_string().with_status(StatusCode::INTERNAL_SERVER_ERROR)
}

pub fn into_bad_request<E: fmt::Display>(e: E) -> WithStatus<String> {
    e.to_string().with_status(StatusCode::BAD_REQUEST)
}

pub fn not_found() -> WithStatus<String> {
    String::new().with_status(StatusCode::NOT_FOUND)
}
