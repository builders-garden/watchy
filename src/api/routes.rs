use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use crate::AppState;

use super::handlers;

pub fn audit_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", post(handlers::request_audit))
        .route("/:audit_id", get(handlers::get_audit))
        .route("/:audit_id/report", get(handlers::get_audit_report))
}

pub fn agent_routes() -> Router<Arc<AppState>> {
    Router::new().route(
        "/:registry/:agent_id/audits",
        get(handlers::list_agent_audits),
    )
}
