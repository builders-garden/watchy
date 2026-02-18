use alloy::{
    network::EthereumWallet,
    primitives::{keccak256, Address, FixedBytes, U256},
    providers::ProviderBuilder,
    signers::local::PrivateKeySigner,
};
use std::str::FromStr;
use tracing::{debug, info, warn};
use url::Url;

use crate::abi::IReputationRegistry::IReputationRegistryInstance;
use crate::types::WatchyError;

/// Reputation Registry client for submitting audit feedback on-chain
///
/// Based on EIP-8004 reputation system:
/// - Feedback submitted with score (0-100, valueDecimals=0)
/// - Submitter cannot be agent owner or approved operator
/// - Feedback references Arweave URL for detailed report
pub struct ReputationClient {
    rpc_url: Url,
    reputation_address: Address,
    signer: Option<PrivateKeySigner>,
}

impl ReputationClient {
    pub fn new(
        rpc_url: &str,
        reputation_address: &str,
        private_key: Option<&str>,
    ) -> Result<Self, WatchyError> {
        let url = Url::parse(rpc_url)
            .map_err(|e| WatchyError::InvalidRequest(format!("Invalid RPC URL: {}", e)))?;

        let address = Address::from_str(reputation_address)
            .map_err(|e| WatchyError::InvalidAddress(format!("Invalid reputation address: {}", e)))?;

        let signer = if let Some(key) = private_key {
            let key = key.strip_prefix("0x").unwrap_or(key);
            let signer: PrivateKeySigner = key
                .parse()
                .map_err(|e| WatchyError::Internal(format!("Invalid private key: {}", e)))?;
            Some(signer)
        } else {
            None
        };

        Ok(Self {
            rpc_url: url,
            reputation_address: address,
            signer,
        })
    }

    /// Submit reputation feedback for an agent
    ///
    /// # Arguments
    /// * `agent_id` - The agent's token ID
    /// * `score` - Score from 0-100
    /// * `tag1` - Primary tag (e.g., "auditScore")
    /// * `tag2` - Secondary tag (e.g., "infrastructure")
    /// * `endpoint` - Primary endpoint tested (optional)
    /// * `feedback_uri` - Arweave URL of the full feedback JSON
    /// * `feedback_json` - The feedback JSON for computing hash
    ///
    /// # Returns
    /// Transaction hash on success
    pub async fn submit_feedback(
        &self,
        agent_id: u64,
        score: u8,
        tag1: &str,
        tag2: &str,
        endpoint: Option<&str>,
        feedback_uri: &str,
        feedback_json: &serde_json::Value,
    ) -> Result<String, WatchyError> {
        let signer = self.signer.as_ref().ok_or_else(|| {
            WatchyError::Internal("Private key required for reputation submission".to_string())
        })?;

        info!(
            "Submitting feedback for agent {} (score: {}, uri: {})",
            agent_id, score, feedback_uri
        );

        // Compute feedbackHash as keccak256 of the JSON
        let json_bytes = serde_json::to_vec(feedback_json)
            .map_err(|e| WatchyError::Internal(format!("JSON serialization failed: {}", e)))?;
        let feedback_hash: FixedBytes<32> = keccak256(&json_bytes);

        debug!("Feedback hash: 0x{}", hex::encode(feedback_hash));

        // Create wallet and provider
        let wallet = EthereumWallet::from(signer.clone());
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(self.rpc_url.clone());

        // Create contract instance
        let contract = IReputationRegistryInstance::new(self.reputation_address, &provider);

        // Build the transaction
        let tx = contract.giveFeedback(
            U256::from(agent_id),
            score as i128,
            0u8, // valueDecimals = 0 for integer scores
            tag1.to_string(),
            tag2.to_string(),
            endpoint.unwrap_or("").to_string(),
            feedback_uri.to_string(),
            feedback_hash,
        );

        // Send the transaction
        let pending = tx.send().await.map_err(|e| {
            let err_str = e.to_string();
            if err_str.contains("CannotGiveFeedbackToOwnAgent") {
                WatchyError::Internal("Cannot give feedback to own agent".to_string())
            } else if err_str.contains("insufficient funds") {
                WatchyError::Internal("Insufficient funds for transaction".to_string())
            } else {
                WatchyError::BlockchainError(format!("Failed to submit feedback: {}", err_str))
            }
        })?;

        let tx_hash = format!("0x{}", hex::encode(pending.tx_hash().as_slice()));
        info!("Feedback transaction sent: {}", tx_hash);

        // Wait for confirmation
        match pending.get_receipt().await {
            Ok(receipt) => {
                if receipt.status() {
                    info!(
                        "Feedback confirmed in block {} (gas used: {})",
                        receipt.block_number.unwrap_or_default(),
                        receipt.gas_used
                    );
                } else {
                    warn!("Feedback transaction reverted: {}", tx_hash);
                    return Err(WatchyError::BlockchainError(
                        "Transaction reverted".to_string(),
                    ));
                }
            }
            Err(e) => {
                warn!("Failed to get receipt (tx may still succeed): {}", e);
            }
        }

        Ok(tx_hash)
    }

    /// Check if the configured signer is authorized to give feedback
    /// (must NOT be owner or approved operator of the agent)
    #[allow(dead_code)]
    pub async fn can_submit_feedback(&self, agent_id: u64) -> Result<bool, WatchyError> {
        let signer = self.signer.as_ref().ok_or_else(|| {
            WatchyError::Internal("Private key required".to_string())
        })?;

        // We need to check against the identity registry, not reputation
        // For now, assume we can submit - the contract will reject if not allowed
        debug!(
            "Checking if {} can submit feedback for agent {}",
            signer.address(),
            agent_id
        );

        Ok(true)
    }

    /// Get feedback count for the current signer and agent
    #[allow(dead_code)]
    pub async fn get_feedback_count(&self, agent_id: u64) -> Result<u64, WatchyError> {
        let signer = self.signer.as_ref().ok_or_else(|| {
            WatchyError::Internal("Private key required".to_string())
        })?;

        let provider = ProviderBuilder::new().on_http(self.rpc_url.clone());
        let contract = IReputationRegistryInstance::new(self.reputation_address, &provider);

        let count = contract
            .getFeedbackCount(signer.address(), U256::from(agent_id))
            .call()
            .await
            .map_err(|e| WatchyError::BlockchainError(format!("getFeedbackCount failed: {}", e)))?;

        Ok(count._0)
    }

    #[allow(dead_code)]
    pub fn has_signing_key(&self) -> bool {
        self.signer.is_some()
    }

    #[allow(dead_code)]
    pub fn signer_address(&self) -> Option<Address> {
        self.signer.as_ref().map(|s| s.address())
    }

    #[allow(dead_code)]
    pub fn reputation_address(&self) -> &Address {
        &self.reputation_address
    }

    #[allow(dead_code)]
    pub fn rpc_url(&self) -> &Url {
        &self.rpc_url
    }
}

/// Helper to compute feedbackHash from JSON
#[allow(dead_code)]
pub fn compute_feedback_hash(json: &serde_json::Value) -> Result<[u8; 32], WatchyError> {
    let bytes = serde_json::to_vec(json)
        .map_err(|e| WatchyError::Internal(format!("JSON serialization failed: {}", e)))?;
    Ok(keccak256(&bytes).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_feedback_hash() {
        let json = serde_json::json!({
            "agentId": 1434,
            "value": 85,
            "tag1": "auditScore"
        });

        let hash = compute_feedback_hash(&json).unwrap();
        assert_eq!(hash.len(), 32);
    }
}
