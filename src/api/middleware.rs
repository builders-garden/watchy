use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use tracing::warn;

use crate::AppState;

/// Middleware to validate API key for service-to-service authentication.
///
/// If `API_KEY` is configured, all requests must include a matching `X-API-Key` header.
/// If `API_KEY` is not set, all requests are allowed (open mode).
pub async fn require_api_key(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // If no API key configured, allow all requests
    let Some(expected_key) = &state.config.api_key else {
        return Ok(next.run(request).await);
    };

    // Check X-API-Key header
    let provided_key = request
        .headers()
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok());

    match provided_key {
        Some(key) if key == expected_key => Ok(next.run(request).await),
        Some(_) => {
            warn!("Invalid API key provided");
            Err(StatusCode::UNAUTHORIZED)
        }
        None => {
            warn!("Missing X-API-Key header");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

/// Middleware to validate Admin API key for privileged operations.
///
/// ADMIN_API_KEY is REQUIRED for admin endpoints. If not configured, admin endpoints are disabled.
pub async fn require_admin_api_key(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Admin API key is required - if not configured, deny all requests
    let Some(expected_key) = &state.config.admin_api_key else {
        warn!("Admin endpoint called but ADMIN_API_KEY is not configured");
        return Err(StatusCode::FORBIDDEN);
    };

    // Check X-Admin-API-Key header
    let provided_key = request
        .headers()
        .get("X-Admin-API-Key")
        .and_then(|v| v.to_str().ok());

    match provided_key {
        Some(key) if key == expected_key => Ok(next.run(request).await),
        Some(_) => {
            warn!("Invalid admin API key provided");
            Err(StatusCode::UNAUTHORIZED)
        }
        None => {
            warn!("Missing X-Admin-API-Key header");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}
