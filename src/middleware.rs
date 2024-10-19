use axum::{extract::Request, http::{header::{CONTENT_LENGTH, CONTENT_TYPE}, StatusCode}, middleware::Next, response::{IntoResponse, Response}, Json};
use serde::Serialize;

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize)]
/// Cargo error reponse
/// 
/// Mostly used for errors. Can be used with a positive error code,
/// will display the messages in this to the user nontheless.
pub struct ApiErrorResponse {
    errors: Vec<ApiError>
}

impl ApiErrorResponse {
    pub fn push_error(&mut self, error: impl Into<String>) {
        self.errors.push(ApiError { detail: error.into() });
    }
    pub fn new() -> Self {
        Self::default()
    }
}

impl Extend<String> for ApiErrorResponse {
    fn extend<T: IntoIterator<Item = String>>(&mut self, iter: T) {
        for detail in iter {
            self.push_error(detail);
        }
    }
}
impl IntoResponse for ApiErrorResponse {
    fn into_response(self) -> Response {
        Json(self).into_response()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
/// Component of a multi-error cargo response 
pub struct ApiError {
    detail: String
}

pub async fn convert_errors_to_json(request: Request, next: Next) -> Response {
    let response = next.run(request).await;
    let status = response.status();
    if !status.is_client_error() && !status.is_server_error() {
        return  response;
    }

    let content_type = response.headers().get(CONTENT_TYPE);
    if content_type.is_none_or(|ct| ct != "text/plain; charset=utf-8") {
        return response;
    }

    let (mut parts, body) = response.into_parts();

    parts.headers.remove(CONTENT_TYPE);
    parts.headers.remove(CONTENT_LENGTH);

    let Ok(bytes) = axum::body::to_bytes(body, usize::MAX).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    let mut errors = ApiErrorResponse::new();
    errors.push_error(text);
    (parts, errors).into_response()
}
