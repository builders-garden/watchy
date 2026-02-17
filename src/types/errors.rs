use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WatchyError {
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    #[error("Agent not found: {0}")]
    AgentNotFound(u64),

    #[error("Audit not found: {0}")]
    AuditNotFound(String),

    #[error("Metadata fetch failed: {0}")]
    MetadataFetchFailed(String),

    #[error("Blockchain error: {0}")]
    BlockchainError(String),

    #[error("IPFS error: {0}")]
    IpfsError(String),

    #[error("Rate limited")]
    RateLimited,

    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after: Option<u64>,
}

impl IntoResponse for WatchyError {
    fn into_response(self) -> Response {
        let (status, error_code, message) = match &self {
            WatchyError::InvalidRequest(msg) => {
                (StatusCode::BAD_REQUEST, "invalid_request", msg.clone())
            }
            WatchyError::InvalidAddress(msg) => {
                (StatusCode::BAD_REQUEST, "invalid_address", msg.clone())
            }
            WatchyError::AgentNotFound(id) => (
                StatusCode::NOT_FOUND,
                "agent_not_found",
                format!("Agent {} not found", id),
            ),
            WatchyError::AuditNotFound(id) => (
                StatusCode::NOT_FOUND,
                "audit_not_found",
                format!("Audit {} not found", id),
            ),
            WatchyError::MetadataFetchFailed(msg) => {
                (StatusCode::BAD_GATEWAY, "metadata_fetch_failed", msg.clone())
            }
            WatchyError::BlockchainError(msg) => {
                (StatusCode::BAD_GATEWAY, "blockchain_error", msg.clone())
            }
            WatchyError::IpfsError(msg) => {
                (StatusCode::BAD_GATEWAY, "ipfs_error", msg.clone())
            }
            WatchyError::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "Too many requests".to_string(),
            ),
            WatchyError::Internal(msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", msg.clone())
            }
        };

        let body = ErrorResponse {
            error: error_code.to_string(),
            message,
            details: None,
            retry_after: if matches!(self, WatchyError::RateLimited) {
                Some(3600)
            } else {
                None
            },
        };

        (status, Json(body)).into_response()
    }
}

impl From<reqwest::Error> for WatchyError {
    fn from(err: reqwest::Error) -> Self {
        WatchyError::MetadataFetchFailed(err.to_string())
    }
}

impl From<serde_json::Error> for WatchyError {
    fn from(err: serde_json::Error) -> Self {
        WatchyError::InvalidRequest(format!("JSON parse error: {}", err))
    }
}
