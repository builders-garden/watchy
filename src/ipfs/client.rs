use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::types::WatchyError;

/// IPFS client for uploading audit reports
///
/// Supports multiple IPFS providers:
/// - Pinata (https://api.pinata.cloud)
/// - Infura (https://ipfs.infura.io)
/// - Local node (http://localhost:5001)

pub struct IpfsClient {
    http_client: reqwest::Client,
    api_url: String,
    api_key: Option<String>,
}

#[derive(Debug, Serialize)]
struct PinataUpload {
    pinataContent: serde_json::Value,
    pinataMetadata: PinataMetadata,
}

#[derive(Debug, Serialize)]
struct PinataMetadata {
    name: String,
}

#[derive(Debug, Deserialize)]
struct PinataResponse {
    #[serde(rename = "IpfsHash")]
    ipfs_hash: String,
}

impl IpfsClient {
    pub fn new(api_url: String, api_key: Option<String>) -> Self {
        Self {
            http_client: reqwest::Client::new(),
            api_url,
            api_key,
        }
    }

    /// Upload JSON content to IPFS
    ///
    /// Returns the CID (Content Identifier) of the uploaded content
    pub async fn upload_json(
        &self,
        content: &serde_json::Value,
        name: &str,
    ) -> Result<String, WatchyError> {
        debug!("Uploading to IPFS: {}", name);

        // Detect provider and use appropriate upload method
        if self.api_url.contains("pinata") {
            self.upload_pinata(content, name).await
        } else {
            self.upload_generic(content).await
        }
    }

    async fn upload_pinata(
        &self,
        content: &serde_json::Value,
        name: &str,
    ) -> Result<String, WatchyError> {
        let api_key = self
            .api_key
            .as_ref()
            .ok_or_else(|| WatchyError::IpfsError("Pinata API key required".to_string()))?;

        let upload = PinataUpload {
            pinataContent: content.clone(),
            pinataMetadata: PinataMetadata {
                name: name.to_string(),
            },
        };

        let response = self
            .http_client
            .post(format!("{}/pinning/pinJSONToIPFS", self.api_url))
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&upload)
            .send()
            .await
            .map_err(|e| WatchyError::IpfsError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(WatchyError::IpfsError(format!(
                "Pinata upload failed: {} - {}",
                status, body
            )));
        }

        let pinata_response: PinataResponse = response
            .json()
            .await
            .map_err(|e| WatchyError::IpfsError(format!("Failed to parse Pinata response: {}", e)))?;

        info!("Uploaded to IPFS: {}", pinata_response.ipfs_hash);

        Ok(pinata_response.ipfs_hash)
    }

    async fn upload_generic(&self, content: &serde_json::Value) -> Result<String, WatchyError> {
        // Generic IPFS HTTP API (local node or other providers)
        let json_bytes = serde_json::to_vec(content)
            .map_err(|e| WatchyError::IpfsError(format!("JSON serialization failed: {}", e)))?;

        let form = reqwest::multipart::Form::new().part(
            "file",
            reqwest::multipart::Part::bytes(json_bytes)
                .file_name("audit.json")
                .mime_str("application/json")
                .map_err(|e| WatchyError::IpfsError(e.to_string()))?,
        );

        let mut request = self
            .http_client
            .post(format!("{}/api/v0/add", self.api_url))
            .multipart(form);

        if let Some(key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        let response = request
            .send()
            .await
            .map_err(|e| WatchyError::IpfsError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(WatchyError::IpfsError(format!(
                "IPFS upload failed: {}",
                response.status()
            )));
        }

        #[derive(Deserialize)]
        struct IpfsAddResponse {
            #[serde(rename = "Hash")]
            hash: String,
        }

        let ipfs_response: IpfsAddResponse = response
            .json()
            .await
            .map_err(|e| WatchyError::IpfsError(format!("Failed to parse IPFS response: {}", e)))?;

        info!("Uploaded to IPFS: {}", ipfs_response.hash);

        Ok(ipfs_response.hash)
    }

    /// Get the gateway URL for a CID
    pub fn gateway_url(&self, cid: &str) -> String {
        // Use public gateway or configured gateway
        format!("https://ipfs.io/ipfs/{}", cid)
    }
}
