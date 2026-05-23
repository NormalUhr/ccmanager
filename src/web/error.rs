//! Web-layer error type: maps internal errors to HTTP responses.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug, thiserror::Error)]
pub enum WebError {
    #[error("not found")]
    NotFound,

    #[error("template rendering failed: {0}")]
    Template(#[from] askama::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    BadRequest(String),

    #[error("{0}")]
    Internal(String),
}

impl From<crate::error::AppError> for WebError {
    fn from(e: crate::error::AppError) -> Self {
        use crate::error::AppError as A;
        match e {
            A::Io(io) => WebError::Io(io),
            A::SessionNotFound(_) => WebError::NotFound,
            other => WebError::Internal(other.to_string()),
        }
    }
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        let (status, body) = match &self {
            WebError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            WebError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            WebError::Template(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("template error: {}", e),
            ),
            WebError::Io(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("io error: {}", e),
            ),
            WebError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };
        // Log server-side for any 5xx, keep 4xx quiet.
        if status.is_server_error() {
            eprintln!("web error [{}]: {}", status, self);
        }
        (status, body).into_response()
    }
}

pub type WebResult<T> = std::result::Result<T, WebError>;
