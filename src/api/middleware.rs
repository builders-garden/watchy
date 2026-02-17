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
