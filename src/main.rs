use anyhow::Result;
use axum::{middleware, routing::get, Router};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::signal;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

mod abi;
mod api;
mod arweave;
mod audit;
mod blockchain;
mod chains;
mod config;
mod ipfs;
mod services;
mod store;
mod types;
mod wallet;

use config::Config;
use store::AuditStore;

pub struct AppState {
    pub config: Config,
    pub http_client: reqwest::Client,
    pub audit_store: AuditStore,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("watchy=debug".parse()?),
        )
        .json()
        .init();

    // Load configuration
    dotenvy::dotenv().ok();
    let config = Config::from_env()?;

    info!("Starting Watchy v{}", env!("CARGO_PKG_VERSION"));
    info!("Default chain: {}", config.default_chain_id);
    info!(
        "Supported chains: {:?}",
        chains::supported_chain_ids()
    );
    info!(
        "Chains with registry: {:?}",
        chains::chains_with_registry()
            .iter()
            .map(|c| c.name)
            .collect::<Vec<_>>()
    );

    // Initialize audit store (with Redis if configured)
    let audit_store = AuditStore::new(config.redis_url.as_deref()).await;
    info!(
        "Storage backend: {}",
        if audit_store.has_redis() { "Redis" } else { "In-memory" }
    );
    info!(
        "Wallet mode: {} (address: {})",
        config.key_mode().as_str(),
        config.signer_address().unwrap_or("none")
    );

    // Create shared state
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let state = Arc::new(AppState {
        config: config.clone(),
        http_client,
        audit_store,
    });

    // Log API key status
    if config.api_key.is_some() {
        info!("API key authentication enabled");
    } else {
        info!("API key authentication disabled (open mode)");
    }

    // Build router
    // Protected routes (require API key if configured)
    let protected_routes = Router::new()
        .nest("/audit", api::routes::audit_routes())
        .nest("/agents", api::routes::agent_routes())
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            api::middleware::require_api_key,
        ));

    let app = Router::new()
        .route("/health", get(api::handlers::health))
        .merge(protected_routes)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state);

    // Start server with graceful shutdown
    let addr = format!("0.0.0.0:{}", config.port);
    info!("Listening on {}", addr);

    let listener = TcpListener::bind(&addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("Server shutdown complete");
    Ok(())
}

/// Wait for shutdown signal (Ctrl+C or SIGTERM)
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, starting graceful shutdown...");
        }
        _ = terminate => {
            info!("Received SIGTERM, starting graceful shutdown...");
        }
    }
}
