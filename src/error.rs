use askama::Error as TemplateError;
use axum::http::header;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use sqlx::Error as SqlxError;
use thiserror::Error;
use tracing::error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("template rendering failed")]
    Template(#[from] TemplateError),
    #[error("database operation failed")]
    Database(#[from] SqlxError),
    #[error("filesystem operation failed")]
    Io(#[from] std::io::Error),
    #[error("password hashing failed")]
    PasswordHash,
    #[error("opds authentication required")]
    OpdsUnauthorized,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match self {
            Self::Template(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::PasswordHash => StatusCode::INTERNAL_SERVER_ERROR,
            Self::OpdsUnauthorized => StatusCode::UNAUTHORIZED,
        };

        if matches!(self, Self::OpdsUnauthorized) {
            return (
                status,
                [(header::WWW_AUTHENTICATE, "Basic realm=\"Papyrd\"")],
                self.to_string(),
            )
                .into_response();
        }

        match &self {
            Self::Template(error_value) => {
                error!(error = ?error_value, "template rendering failed")
            }
            Self::Database(error_value) => {
                error!(error = ?error_value, "database operation failed")
            }
            Self::Io(error_value) => error!(error = ?error_value, "filesystem operation failed"),
            Self::PasswordHash => error!("password hashing failed"),
            Self::OpdsUnauthorized => {}
        }

        (status, self.to_string()).into_response()
    }
}
