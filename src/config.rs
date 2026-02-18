use anyhow::Result;
use std::env;

use crate::wallet::{KeyMode, WalletConfig};

/// Application configuration
///
/// Chain-specific settings (RPC URLs, registry addresses) are stored in chains.rs
/// and looked up by chain_id at runtime. This config holds global settings only.
#[derive(Clone)]
pub struct Config {
    pub port: u16,
    pub default_chain_id: u64,
    pub redis_url: Option<String>,
    pub ipfs_api_url: String,
    pub ipfs_api_key: Option<String>,
    /// Wallet configuration (supports both PRIVATE_KEY and MNEMONIC modes)
    pub wallet: WalletConfig,
    /// API key for service-to-service authentication (optional)
    pub api_key: Option<String>,
    /// Admin API key for privileged operations like agent registration (optional)
    pub admin_api_key: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        // Initialize wallet from environment
        let wallet = WalletConfig::from_env()?;

        Ok(Self {
            // APP_PORT (EigenCloud TLS) takes precedence over PORT
            port: env::var("APP_PORT")
                .or_else(|_| env::var("PORT"))
                .unwrap_or_else(|_| "8080".to_string())
                .parse()?,

            // Default chain if not specified in request
            default_chain_id: env::var("DEFAULT_CHAIN_ID")
                .unwrap_or_else(|_| "8453".to_string()) // Base mainnet
                .parse()?,

            // Redis for job persistence (optional, falls back to in-memory)
            redis_url: env::var("REDIS_URL").ok(),

            ipfs_api_url: env::var("IPFS_API_URL")
                .unwrap_or_else(|_| "https://api.pinata.cloud".to_string()),

            ipfs_api_key: env::var("IPFS_API_KEY").ok(),

            wallet,

            // API key for service-to-service auth (if set, all requests must include X-API-Key header)
            api_key: env::var("API_KEY").ok(),

            // Admin API key for privileged operations (agent registration, etc.)
            admin_api_key: env::var("ADMIN_API_KEY").ok(),
        })
    }

    /// Get private key if available (for backward compatibility)
    pub fn private_key(&self) -> Option<&str> {
        self.wallet.private_key.as_deref()
    }

    /// Get signer address if available
    pub fn signer_address(&self) -> Option<&str> {
        self.wallet.address.as_deref()
    }

    /// Get the key mode
    pub fn key_mode(&self) -> &KeyMode {
        &self.wallet.mode
    }
}
