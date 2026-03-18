use axum::response::{IntoResponse, Response};
use http::StatusCode;
use serde::Serialize;
use thiserror::Error;

/// Structured JSON error response returned by all API endpoints.
#[derive(Debug, Clone, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
    pub suggestion: String,
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("volume not found: {0}")]
    VolumeNotFound(String),

    #[error("volume already exists: {0}")]
    VolumeAlreadyExists(String),

    #[error("volume is mounted: {0}")]
    VolumeIsMounted(String),

    #[error("volume is not mounted: {0}")]
    VolumeNotMounted(String),

    #[error("authentication required")]
    AuthRequired,

    #[error("telegram error: {0}")]
    Telegram(String),

    #[error("crypto error: {0}")]
    Crypto(String),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    #[error("session not open: {0}")]
    SessionNotOpen(String),

    #[error("session already open: {0}")]
    SessionAlreadyOpen(String),

    #[error("internal error: {0}")]
    Internal(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl ApiError {
    /// Returns the appropriate HTTP status code for this error.
    pub fn status_code(&self) -> StatusCode {
        match self {
            ApiError::VolumeNotFound(_) => StatusCode::NOT_FOUND,
            ApiError::VolumeAlreadyExists(_) => StatusCode::CONFLICT,
            ApiError::VolumeIsMounted(_) => StatusCode::CONFLICT,
            ApiError::VolumeNotMounted(_) => StatusCode::BAD_REQUEST,
            ApiError::AuthRequired => StatusCode::UNAUTHORIZED,
            ApiError::Telegram(_) => StatusCode::BAD_GATEWAY,
            ApiError::Crypto(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::SessionNotOpen(_) => StatusCode::BAD_REQUEST,
            ApiError::SessionAlreadyOpen(_) => StatusCode::CONFLICT,
            ApiError::InvalidArgument(_) => StatusCode::BAD_REQUEST,
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Returns a user-facing suggestion for how to resolve this error.
    pub fn suggestion(&self) -> &'static str {
        match self {
            ApiError::VolumeNotFound(_) => "Check the volume ID with GET /api/v1/volumes",
            ApiError::VolumeAlreadyExists(_) => {
                "Use a different name or delete the existing volume first"
            }
            ApiError::VolumeIsMounted(_) => {
                "Unmount the volume first with POST /api/v1/volumes/:id/unmount"
            }
            ApiError::VolumeNotMounted(_) => {
                "Mount the volume first with POST /api/v1/volumes/:id/mount"
            }
            ApiError::AuthRequired => "Authenticate first with POST /api/v1/auth/session",
            ApiError::Telegram(_) => "Check your Telegram connection and try again",
            ApiError::Crypto(_) => "Verify your password is correct",
            ApiError::Storage(_) => "The storage backend encountered an error; check disk space",
            ApiError::SessionNotOpen(_) => {
                "Open the volume first with POST /api/v1/volumes/:id/open"
            }
            ApiError::SessionAlreadyOpen(_) => {
                "The volume is already open; close it first with POST /api/v1/volumes/:id/close"
            }
            ApiError::InvalidArgument(_) => "Check the request body and query parameters",
            ApiError::Internal(_) => "An unexpected error occurred; check server logs for details",
            ApiError::Io(_) => "Check file permissions and available disk space",
        }
    }

    /// Returns a short error code string for programmatic consumption.
    pub fn error_code(&self) -> &'static str {
        match self {
            ApiError::VolumeNotFound(_) => "VOLUME_NOT_FOUND",
            ApiError::VolumeAlreadyExists(_) => "VOLUME_EXISTS",
            ApiError::VolumeIsMounted(_) => "VOLUME_MOUNTED",
            ApiError::VolumeNotMounted(_) => "VOLUME_NOT_MOUNTED",
            ApiError::AuthRequired => "AUTH_REQUIRED",
            ApiError::Telegram(_) => "TELEGRAM_ERROR",
            ApiError::Crypto(_) => "CRYPTO_ERROR",
            ApiError::Storage(_) => "STORAGE_ERROR",
            ApiError::SessionNotOpen(_) => "SESSION_NOT_OPEN",
            ApiError::SessionAlreadyOpen(_) => "SESSION_ALREADY_OPEN",
            ApiError::InvalidArgument(_) => "INVALID_ARGUMENT",
            ApiError::Internal(_) => "INTERNAL_ERROR",
            ApiError::Io(_) => "IO_ERROR",
        }
    }

    /// Builds the structured ErrorResponse for this error.
    pub fn to_error_response(&self) -> ErrorResponse {
        ErrorResponse {
            error: self.to_string(),
            code: self.error_code().to_string(),
            suggestion: self.suggestion().to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = self.to_error_response();
        let json = axum::Json(body);
        (status, json).into_response()
    }
}

pub type Result<T> = std::result::Result<T, ApiError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_codes_are_correct() {
        assert_eq!(
            ApiError::VolumeNotFound("x".into()).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ApiError::VolumeAlreadyExists("x".into()).status_code(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            ApiError::AuthRequired.status_code(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            ApiError::InvalidArgument("x".into()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            ApiError::Telegram("x".into()).status_code(),
            StatusCode::BAD_GATEWAY
        );
        assert_eq!(
            ApiError::Internal("x".into()).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn error_response_serializes_to_json() {
        let err = ApiError::VolumeNotFound("test-vol".into());
        let resp = err.to_error_response();
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("VOLUME_NOT_FOUND"));
        assert!(json.contains("test-vol"));
        assert!(json.contains("suggestion"));
    }

    #[test]
    fn all_variants_have_suggestions() {
        let errors: Vec<ApiError> = vec![
            ApiError::VolumeNotFound("x".into()),
            ApiError::VolumeAlreadyExists("x".into()),
            ApiError::VolumeIsMounted("x".into()),
            ApiError::VolumeNotMounted("x".into()),
            ApiError::AuthRequired,
            ApiError::Telegram("x".into()),
            ApiError::Crypto("x".into()),
            ApiError::Storage("x".into()),
            ApiError::SessionNotOpen("x".into()),
            ApiError::SessionAlreadyOpen("x".into()),
            ApiError::InvalidArgument("x".into()),
            ApiError::Internal("x".into()),
        ];

        for err in &errors {
            assert!(!err.suggestion().is_empty(), "Empty suggestion for: {err}");
            assert!(!err.error_code().is_empty(), "Empty error code for: {err}");
        }
    }

    #[test]
    fn auth_required_suggests_auth_endpoint() {
        let err = ApiError::AuthRequired;
        assert!(err.suggestion().contains("/api/v1/auth"));
    }

    #[test]
    fn into_response_produces_correct_status() {
        let err = ApiError::VolumeNotFound("test".into());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
