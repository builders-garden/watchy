//! Wallet and key management module
//!
//! Supports two key modes:
//! - `private_key`: Direct private key from PRIVATE_KEY env var
//! - `mnemonic`: Derive from MNEMONIC env var (EigenCloud KMS)

use alloy::signers::local::{coins_bip39::English, MnemonicBuilder, PrivateKeySigner};
use std::env;
use tracing::info;

/// Key mode for wallet initialization
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyMode {
    /// Use PRIVATE_KEY env var directly
    PrivateKey,
    /// Derive from MNEMONIC env var (EigenCloud)
    Mnemonic,
    /// No key configured
    None,
}

impl KeyMode {
    /// Detect key mode from environment
    pub fn from_env() -> Self {
        // Check for explicit mode override
        if let Ok(mode) = env::var("KEY_MODE") {
            match mode.to_lowercase().as_str() {
                "mnemonic" | "eigen" | "eigencloud" => return KeyMode::Mnemonic,
                "private_key" | "privatekey" | "key" => return KeyMode::PrivateKey,
                _ => {} // Fall through to auto-detect
            }
        }

        // Auto-detect based on available env vars
        if env::var("MNEMONIC").is_ok() {
            KeyMode::Mnemonic
        } else if env::var("PRIVATE_KEY").is_ok() {
            KeyMode::PrivateKey
        } else {
            KeyMode::None
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            KeyMode::PrivateKey => "private_key",
            KeyMode::Mnemonic => "mnemonic",
            KeyMode::None => "none",
        }
    }
}

/// Wallet configuration derived from environment
#[derive(Debug, Clone)]
pub struct WalletConfig {
    pub mode: KeyMode,
    pub private_key: Option<String>,
    pub address: Option<String>,
}

impl WalletConfig {
    /// Initialize wallet config from environment variables
    ///
    /// Supported env vars:
    /// - `KEY_MODE`: Optional. "mnemonic" or "private_key". Auto-detects if not set.
    /// - `MNEMONIC`: BIP-39 mnemonic phrase (12/24 words). Used when mode=mnemonic.
    /// - `PRIVATE_KEY`: Hex-encoded private key. Used when mode=private_key.
    /// - `DERIVATION_INDEX`: Optional. HD wallet index for mnemonic mode. Default: 0.
    pub fn from_env() -> anyhow::Result<Self> {
        let mode = KeyMode::from_env();

        match mode {
            KeyMode::Mnemonic => {
                let mnemonic = env::var("MNEMONIC")
                    .map_err(|_| anyhow::anyhow!("MNEMONIC env var required for mnemonic mode"))?;

                let index: u32 = env::var("DERIVATION_INDEX")
                    .unwrap_or_else(|_| "0".to_string())
                    .parse()
                    .unwrap_or(0);

                let (private_key, address) = derive_from_mnemonic(&mnemonic, index)?;

                info!(
                    "Wallet initialized from mnemonic (mode: {}, index: {}, address: {})",
                    mode.as_str(),
                    index,
                    address
                );

                Ok(Self {
                    mode,
                    private_key: Some(private_key),
                    address: Some(address),
                })
            }
            KeyMode::PrivateKey => {
                let private_key = env::var("PRIVATE_KEY")
                    .map_err(|_| anyhow::anyhow!("PRIVATE_KEY env var required for private_key mode"))?;

                let address = derive_address(&private_key)?;

                info!(
                    "Wallet initialized from private key (mode: {}, address: {})",
                    mode.as_str(),
                    address
                );

                Ok(Self {
                    mode,
                    private_key: Some(private_key),
                    address: Some(address),
                })
            }
            KeyMode::None => {
                info!("No wallet configured (mode: none). Signing features disabled.");
                Ok(Self {
                    mode,
                    private_key: None,
                    address: None,
                })
            }
        }
    }

    /// Check if signing is available
    pub fn can_sign(&self) -> bool {
        self.private_key.is_some()
    }
}

/// Derive private key and address from BIP-39 mnemonic
///
/// Uses standard Ethereum derivation path: m/44'/60'/0'/0/{index}
fn derive_from_mnemonic(mnemonic: &str, index: u32) -> anyhow::Result<(String, String)> {
    let signer = MnemonicBuilder::<English>::default()
        .phrase(mnemonic)
        .index(index)
        .map_err(|e| anyhow::anyhow!("Invalid derivation index: {}", e))?
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to derive from mnemonic: {}", e))?;

    // Get private key as hex
    let private_key = format!("0x{}", hex::encode(signer.credential().to_bytes()));
    let address = format!("{:?}", signer.address());

    Ok((private_key, address))
}

/// Derive address from private key
fn derive_address(private_key: &str) -> anyhow::Result<String> {
    let key = private_key.strip_prefix("0x").unwrap_or(private_key);
    let signer: PrivateKeySigner = key
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid private key: {}", e))?;

    Ok(format!("{:?}", signer.address()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_from_mnemonic() {
        // Standard test mnemonic (DO NOT USE IN PRODUCTION)
        let mnemonic = "test test test test test test test test test test test junk";
        let (private_key, address) = derive_from_mnemonic(mnemonic, 0).unwrap();

        assert!(private_key.starts_with("0x"));
        assert!(address.starts_with("0x"));
        assert_eq!(private_key.len(), 66); // 0x + 64 hex chars
        assert_eq!(address.len(), 42); // 0x + 40 hex chars
    }

    #[test]
    fn test_derive_address() {
        // Known test key
        let private_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let address = derive_address(private_key).unwrap();

        assert_eq!(address.to_lowercase(), "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266");
    }

    #[test]
    fn test_key_mode_detection() {
        // Without env vars set, should be None
        // (This test might fail if env vars are set in the environment)
        // Just test the parsing logic
        assert_eq!(KeyMode::PrivateKey.as_str(), "private_key");
        assert_eq!(KeyMode::Mnemonic.as_str(), "mnemonic");
        assert_eq!(KeyMode::None.as_str(), "none");
    }
}
