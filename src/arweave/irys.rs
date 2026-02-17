use alloy::primitives::{keccak256, Address, PrimitiveSignature};
use alloy::signers::local::PrivateKeySigner;
use alloy::signers::Signer;
use bundles_rs::ans104::{data_item::DataItem, tags::Tag};
use bundles_rs::crypto::ethereum::EthereumSigner;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::types::WatchyError;

/// Turbo upload endpoint for Ethereum
const TURBO_UPLOAD_URL: &str = "https://turbo.ardrive.io/tx/ethereum";

/// Irys client for uploading data to Arweave via Turbo
pub struct IrysClient {
    http_client: reqwest::Client,
    signer: Option<EthereumSigner>,
    address: Option<Address>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurboUploadResponse {
    pub id: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub data_caches: Vec<String>,
    #[serde(default)]
    pub fast_finality_indexes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct UploadResult {
    pub tx_id: String,
    pub arweave_url: String,
    pub gateway_url: String,
}

impl IrysClient {
    /// Create a new Irys client
    ///
    /// If private_key is provided, uploads will be signed.
    /// Without a key, uploads will fail.
    pub fn new(private_key: Option<&str>) -> Result<Self, WatchyError> {
        let (signer, address) = if let Some(key) = private_key {
            let key = key.strip_prefix("0x").unwrap_or(key);
            let key_bytes = hex::decode(key)
                .map_err(|e| WatchyError::Internal(format!("Invalid private key hex: {}", e)))?;

            if key_bytes.len() != 32 {
                return Err(WatchyError::Internal(format!(
                    "Private key must be 32 bytes, got {}",
                    key_bytes.len()
                )));
            }

            let signer = EthereumSigner::from_bytes(&key_bytes)
                .map_err(|e| WatchyError::Internal(format!("Failed to create signer: {}", e)))?;

            // Get the address from the signer
            let addr_bytes = signer.address();
            let address: Address = Address::from_slice(&addr_bytes);

            (Some(signer), Some(address))
        } else {
            (None, None)
        };

        Ok(Self {
            http_client: reqwest::Client::new(),
            signer,
            address,
        })
    }

    /// Get the signer's address if available
    pub fn address(&self) -> Option<Address> {
        self.address
    }

    /// Upload data to Arweave via Turbo using ANS-104 DataItem format
    ///
    /// Uses the bundles-rs crate for proper DataItem creation and signing.
    pub async fn upload(
        &self,
        data: &[u8],
        content_type: &str,
        tags: Vec<(&str, &str)>,
    ) -> Result<UploadResult, WatchyError> {
        let size = data.len();
        debug!(
            "Uploading {} bytes to Turbo (content-type: {})",
            size, content_type
        );

        let signer = self.signer.as_ref().ok_or_else(|| {
            WatchyError::Internal("Turbo upload requires a signer (PRIVATE_KEY)".to_string())
        })?;

        // Build tags - always include Content-Type first
        let mut all_tags = vec![Tag::new("Content-Type", content_type)];
        for (name, value) in tags {
            if name != "Content-Type" {
                all_tags.push(Tag::new(name, value));
            }
        }

        // Create and sign the DataItem using bundles-rs
        let data_item = DataItem::build_and_sign(
            signer,
            None, // no target
            None, // no anchor
            all_tags,
            data.to_vec(),
        )
        .map_err(|e| WatchyError::Internal(format!("Failed to create DataItem: {}", e)))?;

        // Serialize the DataItem to bytes
        let item_bytes = data_item
            .to_bytes()
            .map_err(|e| WatchyError::Internal(format!("Failed to serialize DataItem: {}", e)))?;

        debug!(
            "Built DataItem: {} bytes total (data: {} bytes)",
            item_bytes.len(),
            size
        );

        // Upload to Turbo
        let response = self
            .http_client
            .post(TURBO_UPLOAD_URL)
            .header("Content-Type", "application/octet-stream")
            .body(item_bytes)
            .send()
            .await
            .map_err(|e| WatchyError::Internal(format!("Turbo upload failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            warn!("Turbo upload failed: HTTP {} - {}", status, body);
            return Err(WatchyError::Internal(format!(
                "Turbo upload failed: HTTP {} - {}",
                status, body
            )));
        }

        let upload_response: TurboUploadResponse = response
            .json()
            .await
            .map_err(|e| WatchyError::Internal(format!("Failed to parse Turbo response: {}", e)))?;

        let result = UploadResult {
            tx_id: upload_response.id.clone(),
            arweave_url: format!("https://arweave.net/{}", upload_response.id),
            gateway_url: format!("https://arweave.net/{}", upload_response.id),
        };

        info!(
            "Uploaded to Arweave via Turbo: {} (size: {} bytes)",
            result.tx_id, size
        );

        Ok(result)
    }

    /// Upload JSON data with appropriate tags
    pub async fn upload_json(
        &self,
        json: &serde_json::Value,
        filename: &str,
    ) -> Result<UploadResult, WatchyError> {
        let data = serde_json::to_vec_pretty(json)
            .map_err(|e| WatchyError::Internal(format!("JSON serialization failed: {}", e)))?;

        self.upload(
            &data,
            "application/json",
            vec![
                ("filename", filename),
                ("App-Name", "Watchy"),
                ("App-Version", env!("CARGO_PKG_VERSION")),
            ],
        )
        .await
    }

    /// Upload Markdown data with appropriate tags
    pub async fn upload_markdown(
        &self,
        markdown: &str,
        filename: &str,
    ) -> Result<UploadResult, WatchyError> {
        self.upload(
            markdown.as_bytes(),
            "text/markdown",
            vec![
                ("filename", filename),
                ("App-Name", "Watchy"),
                ("App-Version", env!("CARGO_PKG_VERSION")),
            ],
        )
        .await
    }
}

/// Sign an audit report and return the signature
pub async fn sign_report(
    report_json: &serde_json::Value,
    private_key: &str,
) -> Result<String, WatchyError> {
    let key = private_key.strip_prefix("0x").unwrap_or(private_key);
    let signer: PrivateKeySigner = key
        .parse()
        .map_err(|e| WatchyError::Internal(format!("Invalid private key: {}", e)))?;

    // Create a deterministic hash of the report
    let report_bytes = serde_json::to_vec(report_json)
        .map_err(|e| WatchyError::Internal(format!("Serialization failed: {}", e)))?;

    let hash = keccak256(&report_bytes);

    // Sign the hash
    let signature = signer
        .sign_hash(&hash)
        .await
        .map_err(|e| WatchyError::Internal(format!("Signing failed: {}", e)))?;

    Ok(format!("0x{}", hex::encode(signature.as_bytes())))
}

/// Verify a report signature
#[allow(dead_code)]
pub fn verify_report_signature(
    report_json: &serde_json::Value,
    signature: &str,
    expected_address: &str,
) -> Result<bool, WatchyError> {
    let report_bytes = serde_json::to_vec(report_json)
        .map_err(|e| WatchyError::Internal(format!("Serialization failed: {}", e)))?;

    let hash = keccak256(&report_bytes);

    let sig_bytes = hex::decode(signature.strip_prefix("0x").unwrap_or(signature))
        .map_err(|e| WatchyError::Internal(format!("Invalid signature hex: {}", e)))?;

    let signature = PrimitiveSignature::try_from(sig_bytes.as_slice())
        .map_err(|e| WatchyError::Internal(format!("Invalid signature: {}", e)))?;

    let recovered = signature
        .recover_address_from_prehash(&hash)
        .map_err(|e| WatchyError::Internal(format!("Recovery failed: {}", e)))?;

    let expected: Address = expected_address
        .parse()
        .map_err(|e| WatchyError::Internal(format!("Invalid address: {}", e)))?;

    Ok(recovered == expected)
}
