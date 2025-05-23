use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use std::str::Utf8Error;

#[allow(dead_code)]
pub enum Errors {
    /// 413
    TooBig(usize),
    /// 500
    SqlxError(sqlx::Error),
    /// 500
    InternalServerError(anyhow::Error),
    /// 501
    Unimplemented,
    /// 400
    InvalidPublicKey,
    /// 401
    Unauthorized,
    /// 404
    NotFound,
}

pub enum AppError {
    AnyhowError(AnyhowError),
    Error(Errors),
    Generic(StatusCode, String),
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::AnyhowError(AnyhowError(e))
    }
}

impl From<sqlx::types::uuid::Error> for AppError {
    fn from(e: sqlx::types::uuid::Error) -> Self {
        AppError::Error(Errors::SqlxError(sqlx::Error::Decode(e.into())))
    }
}

impl From<Errors> for AppError {
    fn from(e: Errors) -> Self {
        AppError::Error(e)
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        AppError::Error(Errors::SqlxError(e))
    }
}

impl From<Utf8Error> for AppError {
    fn from(e: Utf8Error) -> Self {
        AppError::Error(Errors::InternalServerError(anyhow::Error::from(e)))
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::AnyhowError(e) => e.into_response(),

            AppError::Error(e) => match e {
                Errors::TooBig(size_limit) => (
                    StatusCode::BAD_REQUEST,
                    format!("Value cannot be greater than {} bytes", size_limit),
                )
                    .into_response(),
                Errors::SqlxError(_) => {
                    (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong").into_response()
                }
                Errors::InternalServerError(e) => {
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
                Errors::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized").into_response(),
                Errors::Unimplemented => {
                    (StatusCode::NOT_IMPLEMENTED, "Not implemented").into_response()
                }
                Errors::InvalidPublicKey => {
                    (StatusCode::BAD_REQUEST, "Invalid public key").into_response()
                }
                Errors::NotFound => (StatusCode::NOT_FOUND, "Not found").into_response(),
            },
            AppError::Generic(status_code, string) => (status_code, string).into_response(),
        }
    }
}

// Make our own error that wraps `anyhow::Error`.
pub struct AnyhowError(anyhow::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AnyhowError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AnyhowError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
