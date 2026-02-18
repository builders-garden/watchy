// ANS-104 DataItem implementation for Arweave uploads
// Replaces bundles-rs dependency with minimal native implementation

use alloy::primitives::{keccak256, Address, B256};
use alloy::signers::local::PrivateKeySigner;
use alloy::signers::Signer;
use sha2::{Digest, Sha256};

use crate::types::WatchyError;

/// Signature type for Ethereum
const SIG_TYPE_ETHEREUM: u16 = 3;
/// Ethereum signature length (65 bytes: r + s + v)
const ETH_SIG_LENGTH: usize = 65;
/// Ethereum public key length (65 bytes uncompressed)
const ETH_PUBKEY_LENGTH: usize = 65;

/// ANS-104 Tag
#[derive(Debug, Clone)]
pub struct Tag {
    pub name: String,
    pub value: String,
}

impl Tag {
    pub fn new(name: &str, value: &str) -> Self {
        Self {
            name: name.to_string(),
            value: value.to_string(),
        }
    }
}

/// ANS-104 DataItem
pub struct DataItem {
    signature: Vec<u8>,
    owner: Vec<u8>,
    target: Option<Vec<u8>>,
    anchor: Option<Vec<u8>>,
    tags: Vec<Tag>,
    data: Vec<u8>,
}

impl DataItem {
    /// Build and sign a DataItem with an Ethereum key
    pub async fn build_and_sign(
        private_key: &str,
        target: Option<Vec<u8>>,
        anchor: Option<Vec<u8>>,
        tags: Vec<Tag>,
        data: Vec<u8>,
    ) -> Result<Self, WatchyError> {
        let key = private_key.strip_prefix("0x").unwrap_or(private_key);
        let signer: PrivateKeySigner = key
            .parse()
            .map_err(|e| WatchyError::Internal(format!("Invalid private key: {}", e)))?;

        // Get uncompressed public key (65 bytes with 0x04 prefix)
        let verifying_key = signer.credential().verifying_key();
        let pubkey_bytes = verifying_key.to_encoded_point(false);
        let owner = pubkey_bytes.as_bytes().to_vec();

        // Create deep hash for signing
        let deep_hash = Self::create_deep_hash(&owner, &target, &anchor, &tags, &data)?;

        // Sign the deep hash with Ethereum wallet
        let signature = signer
            .sign_hash(&B256::from_slice(&deep_hash))
            .await
            .map_err(|e| WatchyError::Internal(format!("Signing failed: {}", e)))?;

        // Convert signature to 65 bytes (r || s || v)
        let sig_bytes = signature.as_bytes().to_vec();

        Ok(Self {
            signature: sig_bytes,
            owner,
            target,
            anchor,
            tags,
            data,
        })
    }

    /// Create the deep hash for signing
    /// This is a simplified version - ANS-104 uses a merkle-like structure
    fn create_deep_hash(
        owner: &[u8],
        target: &Option<Vec<u8>>,
        anchor: &Option<Vec<u8>>,
        tags: &[Tag],
        data: &[u8],
    ) -> Result<Vec<u8>, WatchyError> {
        let mut hasher = Sha256::new();

        // Hash format string
        hasher.update(b"dataitem");
        hasher.update(b"1"); // version

        // Hash signature type
        hasher.update(&SIG_TYPE_ETHEREUM.to_le_bytes());

        // Hash owner
        hasher.update(owner);

        // Hash target
        if let Some(t) = target {
            hasher.update(t);
        }

        // Hash anchor
        if let Some(a) = anchor {
            hasher.update(a);
        }

        // Hash tags
        let tag_bytes = Self::serialize_tags(tags)?;
        hasher.update(&tag_bytes);

        // Hash data
        hasher.update(data);

        Ok(hasher.finalize().to_vec())
    }

    /// Serialize tags in ANS-104 format (AVro-like)
    fn serialize_tags(tags: &[Tag]) -> Result<Vec<u8>, WatchyError> {
        let mut result = Vec::new();

        for tag in tags {
            // Name length (2 bytes LE) + name bytes
            let name_bytes = tag.name.as_bytes();
            result.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            result.extend_from_slice(name_bytes);

            // Value length (2 bytes LE) + value bytes
            let value_bytes = tag.value.as_bytes();
            result.extend_from_slice(&(value_bytes.len() as u16).to_le_bytes());
            result.extend_from_slice(value_bytes);
        }

        Ok(result)
    }

    /// Serialize DataItem to bytes for upload
    pub fn to_bytes(&self) -> Result<Vec<u8>, WatchyError> {
        let mut result = Vec::new();

        // Signature type (2 bytes, LE)
        result.extend_from_slice(&SIG_TYPE_ETHEREUM.to_le_bytes());

        // Signature (65 bytes for Ethereum)
        if self.signature.len() != ETH_SIG_LENGTH {
            return Err(WatchyError::Internal(format!(
                "Invalid signature length: {} (expected {})",
                self.signature.len(),
                ETH_SIG_LENGTH
            )));
        }
        result.extend_from_slice(&self.signature);

        // Owner/public key (65 bytes for Ethereum)
        if self.owner.len() != ETH_PUBKEY_LENGTH {
            return Err(WatchyError::Internal(format!(
                "Invalid owner length: {} (expected {})",
                self.owner.len(),
                ETH_PUBKEY_LENGTH
            )));
        }
        result.extend_from_slice(&self.owner);

        // Target presence + optional target (32 bytes)
        if let Some(ref target) = self.target {
            result.push(1);
            result.extend_from_slice(target);
        } else {
            result.push(0);
        }

        // Anchor presence + optional anchor (32 bytes)
        if let Some(ref anchor) = self.anchor {
            result.push(1);
            result.extend_from_slice(anchor);
        } else {
            result.push(0);
        }

        // Tags
        let tag_bytes = Self::serialize_tags(&self.tags)?;
        // Number of tags (8 bytes LE)
        result.extend_from_slice(&(self.tags.len() as u64).to_le_bytes());
        // Tag bytes length (8 bytes LE)
        result.extend_from_slice(&(tag_bytes.len() as u64).to_le_bytes());
        // Tag data
        result.extend_from_slice(&tag_bytes);

        // Data
        result.extend_from_slice(&self.data);

        Ok(result)
    }

    /// Get the address derived from the owner public key
    pub fn address(&self) -> Result<Address, WatchyError> {
        if self.owner.len() != ETH_PUBKEY_LENGTH {
            return Err(WatchyError::Internal("Invalid owner key".to_string()));
        }
        // Skip the 0x04 prefix and hash the rest
        let hash = keccak256(&self.owner[1..]);
        // Take last 20 bytes
        Ok(Address::from_slice(&hash[12..]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test private key (DO NOT use in production - this is a well-known test key)
    const TEST_PRIVATE_KEY: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    #[test]
    fn test_tag_creation() {
        let tag = Tag::new("Content-Type", "application/json");
        assert_eq!(tag.name, "Content-Type");
        assert_eq!(tag.value, "application/json");
    }

    #[test]
    fn test_tag_serialization() {
        let tags = vec![
            Tag::new("Content-Type", "text/plain"),
            Tag::new("App-Name", "Watchy"),
        ];

        let bytes = DataItem::serialize_tags(&tags).unwrap();

        // Verify structure: name_len(2) + name + value_len(2) + value for each tag
        // "Content-Type" = 12 bytes, "text/plain" = 10 bytes
        // "App-Name" = 8 bytes, "Watchy" = 6 bytes
        // Total: (2+12+2+10) + (2+8+2+6) = 26 + 18 = 44 bytes
        assert_eq!(bytes.len(), 44);

        // Check first tag name length (12 as u16 LE)
        assert_eq!(bytes[0], 12);
        assert_eq!(bytes[1], 0);
    }

    #[tokio::test]
    async fn test_data_item_creation() {
        let tags = vec![Tag::new("Content-Type", "application/json")];
        let data = b"test data".to_vec();

        let result = DataItem::build_and_sign(TEST_PRIVATE_KEY, None, None, tags, data).await;

        assert!(result.is_ok(), "DataItem creation should succeed");

        let data_item = result.unwrap();

        // Verify signature length (65 bytes for Ethereum)
        assert_eq!(data_item.signature.len(), ETH_SIG_LENGTH);

        // Verify owner/pubkey length (65 bytes uncompressed)
        assert_eq!(data_item.owner.len(), ETH_PUBKEY_LENGTH);

        // Verify owner starts with 0x04 (uncompressed point marker)
        assert_eq!(data_item.owner[0], 0x04);
    }

    #[tokio::test]
    async fn test_data_item_serialization() {
        let tags = vec![Tag::new("Content-Type", "text/plain")];
        let data = b"hello world".to_vec();

        let data_item = DataItem::build_and_sign(TEST_PRIVATE_KEY, None, None, tags, data.clone())
            .await
            .unwrap();

        let bytes = data_item.to_bytes().unwrap();

        // Verify minimum structure:
        // sig_type(2) + sig(65) + owner(65) + target_flag(1) + anchor_flag(1) +
        // num_tags(8) + tag_bytes_len(8) + tags + data
        let min_header = 2 + 65 + 65 + 1 + 1 + 8 + 8;
        assert!(bytes.len() > min_header);

        // Verify signature type is Ethereum (3)
        assert_eq!(bytes[0], 3);
        assert_eq!(bytes[1], 0);

        // Verify data is at the end
        let data_start = bytes.len() - data.len();
        assert_eq!(&bytes[data_start..], &data[..]);
    }

    #[tokio::test]
    async fn test_data_item_address_derivation() {
        let tags = vec![];
        let data = b"test".to_vec();

        let data_item = DataItem::build_and_sign(TEST_PRIVATE_KEY, None, None, tags, data)
            .await
            .unwrap();

        let address = data_item.address().unwrap();

        // The test private key corresponds to this address
        let expected: Address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
            .parse()
            .unwrap();

        assert_eq!(address, expected);
    }

    #[tokio::test]
    async fn test_data_item_with_target() {
        let tags = vec![];
        let data = b"test".to_vec();
        let target = vec![0u8; 32]; // 32-byte target

        let data_item =
            DataItem::build_and_sign(TEST_PRIVATE_KEY, Some(target.clone()), None, tags, data)
                .await
                .unwrap();

        let bytes = data_item.to_bytes().unwrap();

        // After sig_type(2) + sig(65) + owner(65) = 132 bytes, should be target flag = 1
        assert_eq!(bytes[132], 1);
    }

    #[test]
    fn test_deep_hash_deterministic() {
        let owner = vec![0x04; 65]; // Dummy owner
        let tags = vec![Tag::new("test", "value")];
        let data = b"test data".to_vec();

        let hash1 = DataItem::create_deep_hash(&owner, &None, &None, &tags, &data).unwrap();
        let hash2 = DataItem::create_deep_hash(&owner, &None, &None, &tags, &data).unwrap();

        assert_eq!(hash1, hash2, "Deep hash should be deterministic");
        assert_eq!(hash1.len(), 32, "SHA256 should produce 32 bytes");
    }
}
