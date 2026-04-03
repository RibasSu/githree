use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("git error: {0}")]
    GitError(String),
    #[error("io error: {0}")]
    IoError(String),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("clone error: {0}")]
    CloneError(String),
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
}

impl AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            Self::CloneError(_) => StatusCode::BAD_GATEWAY,
            Self::GitError(_) | Self::IoError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn code(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "NOT_FOUND",
            Self::InvalidRequest(_) => "INVALID_REQUEST",
            Self::CloneError(_) => "CLONE_ERROR",
            Self::GitError(_) => "GIT_ERROR",
            Self::IoError(_) => "IO_ERROR",
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = ErrorResponse {
            error: self.to_string(),
            code: self.code().to_string(),
        };
        (self.status_code(), Json(body)).into_response()
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::IoError(value.to_string())
    }
}

impl From<git2::Error> for AppError {
    fn from(value: git2::Error) -> Self {
        Self::GitError(value.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        Self::InvalidRequest(value.to_string())
    }
}
