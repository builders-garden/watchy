use tracing::{debug, warn};

use crate::types::{AgentMetadata, WatchyError};

/// IPFS gateways in order of preference
const IPFS_GATEWAYS: &[&str] = &[
    "https://dweb.link/ipfs/",
    "https://cloudflare-ipfs.com/ipfs/",
    "https://ipfs.io/ipfs/",
    "https://w3s.link/ipfs/",
    "https://gateway.pinata.cloud/ipfs/",
];

/// Arweave gateways in order of preference
const ARWEAVE_GATEWAYS: &[&str] = &[
    "https://arweave.net/",
    "https://ar-io.net/",
    "https://arweave.dev/",
];

/// Resolve a URI to a list of possible HTTP URLs (for fallback)
fn resolve_uri_with_fallbacks(uri: &str) -> Vec<String> {
    if let Some(cid) = uri.strip_prefix("ipfs://") {
        IPFS_GATEWAYS
            .iter()
            .map(|gateway| format!("{}{}", gateway, cid))
            .collect()
    } else if let Some(tx_id) = uri.strip_prefix("ar://") {
        ARWEAVE_GATEWAYS
            .iter()
            .map(|gateway| format!("{}{}", gateway, tx_id))
            .collect()
    } else {
        // Direct HTTP(S) URL - no fallbacks
        vec![uri.to_string()]
    }
}

/// Fetch and parse agent metadata from URI with gateway fallbacks
pub async fn fetch_metadata(
    client: &reqwest::Client,
    uri: &str,
) -> Result<AgentMetadata, WatchyError> {
    // Handle data: URLs (inline base64 JSON)
    if let Some(data_content) = uri.strip_prefix("data:") {
        return parse_data_uri(data_content);
    }

    let urls = resolve_uri_with_fallbacks(uri);

    debug!(
        "Fetching metadata from {} ({} gateway options)",
        uri,
        urls.len()
    );

    let mut last_error = String::new();

    for (i, url) in urls.iter().enumerate() {
        debug!("Trying gateway {}/{}: {}", i + 1, urls.len(), url);

        match try_fetch_metadata(client, url).await {
            Ok(metadata) => {
                debug!(
                    "Successfully fetched metadata for agent '{}' from {}",
                    metadata.name.as_deref().unwrap_or("unknown"),
                    url
                );
                return Ok(metadata);
            }
            Err(e) => {
                warn!("Gateway {} failed: {}", url, e);
                last_error = e;
                // Continue to next gateway
            }
        }
    }

    // All gateways failed
    Err(WatchyError::MetadataFetchFailed(format!(
        "All {} gateways failed for {}. Last error: {}",
        urls.len(),
        uri,
        last_error
    )))
}

/// Parse a data: URI containing inline JSON
/// Supports: data:application/json;base64,<base64_data>
///           data:application/json,<url_encoded_json>
fn parse_data_uri(content: &str) -> Result<AgentMetadata, WatchyError> {
    use base64::Engine;

    debug!("Parsing data: URI");

    // Expected format: application/json;base64,<data> or application/json,<data>
    if let Some(base64_data) = content
        .strip_prefix("application/json;base64,")
        .or_else(|| content.strip_prefix("application/json;charset=utf-8;base64,"))
    {
        // Decode base64
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(base64_data)
            .map_err(|e| WatchyError::MetadataFetchFailed(format!("Base64 decode error: {}", e)))?;

        let json_str = String::from_utf8(decoded)
            .map_err(|e| WatchyError::MetadataFetchFailed(format!("UTF-8 decode error: {}", e)))?;

        let metadata: AgentMetadata = serde_json::from_str(&json_str)
            .map_err(|e| WatchyError::MetadataFetchFailed(format!("JSON parse error: {}", e)))?;

        debug!(
            "Successfully parsed inline metadata for agent '{}'",
            metadata.name.as_deref().unwrap_or("unknown")
        );

        return Ok(metadata);
    }

    // Plain JSON (URL-encoded or raw)
    if let Some(json_data) = content.strip_prefix("application/json,") {
        let decoded = urlencoding::decode(json_data)
            .map_err(|e| WatchyError::MetadataFetchFailed(format!("URL decode error: {}", e)))?;

        let metadata: AgentMetadata = serde_json::from_str(&decoded)
            .map_err(|e| WatchyError::MetadataFetchFailed(format!("JSON parse error: {}", e)))?;

        return Ok(metadata);
    }

    Err(WatchyError::MetadataFetchFailed(
        "Unsupported data: URI format. Expected application/json".to_string(),
    ))
}

/// Maximum metadata size in bytes (1 MB)
const MAX_METADATA_SIZE: usize = 1024 * 1024;

/// Try to fetch metadata from a single URL
async fn try_fetch_metadata(
    client: &reqwest::Client,
    url: &str,
) -> Result<AgentMetadata, String> {
    let response = client
        .get(url)
        .header("Accept", "application/json")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }

    // Check content-length if available
    if let Some(content_length) = response.content_length() {
        if content_length as usize > MAX_METADATA_SIZE {
            return Err(format!(
                "Metadata too large: {} bytes (max {} bytes)",
                content_length, MAX_METADATA_SIZE
            ));
        }
    }

    // Read body with size limit
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    if bytes.len() > MAX_METADATA_SIZE {
        return Err(format!(
            "Metadata too large: {} bytes (max {} bytes)",
            bytes.len(), MAX_METADATA_SIZE
        ));
    }

    let metadata: AgentMetadata = serde_json::from_slice(&bytes)
        .map_err(|e| format!("JSON parse error: {}", e))?;

    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipfs_fallbacks() {
        let urls = resolve_uri_with_fallbacks("ipfs://bafkreitest123");
        assert_eq!(urls.len(), 5);
        assert!(urls[0].contains("dweb.link"));
        assert!(urls[1].contains("cloudflare-ipfs.com"));
    }

    #[test]
    fn test_arweave_fallbacks() {
        let urls = resolve_uri_with_fallbacks("ar://abc123xyz");
        assert_eq!(urls.len(), 3);
        assert!(urls[0].contains("arweave.net"));
        assert!(urls[1].contains("ar-io.net"));
    }

    #[test]
    fn test_https_no_fallbacks() {
        let urls = resolve_uri_with_fallbacks("https://example.com/metadata.json");
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "https://example.com/metadata.json");
    }

    #[test]
    fn test_data_uri_base64() {
        use base64::Engine;

        let json = r#"{"type":"test","name":"Test Agent","description":"A test agent"}"#;
        let encoded = base64::engine::general_purpose::STANDARD.encode(json);
        let data_uri_content = format!("application/json;base64,{}", encoded);

        let result = parse_data_uri(&data_uri_content);
        assert!(result.is_ok());
        let metadata = result.unwrap();
        assert_eq!(metadata.name, Some("Test Agent".to_string()));
    }
}
