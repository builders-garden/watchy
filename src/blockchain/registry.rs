use alloy::{
    network::{Ethereum, EthereumWallet},
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder, RootProvider},
    signers::local::PrivateKeySigner,
    transports::http::{Client, Http},
};
use std::str::FromStr;
use tracing::{debug, error, info};
use url::Url;

use crate::abi::IIdentityRegistry::IIdentityRegistryInstance;
use crate::types::WatchyError;

type HttpProvider = RootProvider<Http<Client>, Ethereum>;

/// EIP-8004 Registry contract client
pub struct RegistryClient {
    rpc_url: Url,
    registry_address: Address,
}

impl RegistryClient {
    pub fn new(rpc_url: &str, registry_address: &str) -> Result<Self, WatchyError> {
        let url = Url::parse(rpc_url)
            .map_err(|e| WatchyError::InvalidRequest(format!("Invalid RPC URL: {}", e)))?;

        let address = Address::from_str(registry_address)
            .map_err(|e| WatchyError::InvalidAddress(format!("Invalid registry address: {}", e)))?;

        Ok(Self {
            rpc_url: url,
            registry_address: address,
        })
    }

    /// Create a provider instance
    fn provider(&self) -> HttpProvider {
        ProviderBuilder::new().on_http(self.rpc_url.clone())
    }

    /// Check if an agent exists by calling ownerOf
    pub async fn agent_exists(&self, agent_id: u64) -> Result<bool, WatchyError> {
        let provider = self.provider();
        let contract = IIdentityRegistryInstance::new(self.registry_address, provider);

        match contract.ownerOf(U256::from(agent_id)).call().await {
            Ok(_) => Ok(true),
            Err(e) => {
                let err_str = e.to_string();
                // ERC721NonexistentToken error means agent doesn't exist
                if err_str.contains("NonexistentToken") || err_str.contains("nonexistent") {
                    Ok(false)
                } else {
                    error!("ownerOf call failed: {}", err_str);
                    Err(WatchyError::BlockchainError(format!(
                        "Failed to check agent existence: {}",
                        err_str
                    )))
                }
            }
        }
    }

    /// Get the owner of an agent
    pub async fn owner_of(&self, agent_id: u64) -> Result<Address, WatchyError> {
        debug!("Fetching owner for agent {}", agent_id);

        let provider = self.provider();
        let contract = IIdentityRegistryInstance::new(self.registry_address, provider);

        let owner = contract
            .ownerOf(U256::from(agent_id))
            .call()
            .await
            .map_err(|e| {
                let err_str = e.to_string();
                if err_str.contains("NonexistentToken") || err_str.contains("nonexistent") {
                    WatchyError::AgentNotFound(agent_id)
                } else {
                    WatchyError::BlockchainError(format!("ownerOf failed: {}", err_str))
                }
            })?;

        Ok(owner._0)
    }

    /// Get the metadata URI for an agent
    pub async fn token_uri(&self, agent_id: u64) -> Result<String, WatchyError> {
        debug!("Fetching tokenURI for agent {}", agent_id);

        let provider = self.provider();
        let contract = IIdentityRegistryInstance::new(self.registry_address, provider);

        let uri = contract
            .tokenURI(U256::from(agent_id))
            .call()
            .await
            .map_err(|e| {
                let err_str = e.to_string();
                if err_str.contains("NonexistentToken") || err_str.contains("nonexistent") {
                    WatchyError::AgentNotFound(agent_id)
                } else {
                    WatchyError::BlockchainError(format!("tokenURI failed: {}", err_str))
                }
            })?;

        Ok(uri._0)
    }

    /// Get the agent wallet address
    pub async fn get_agent_wallet(&self, agent_id: u64) -> Result<Option<Address>, WatchyError> {
        debug!("Fetching agent wallet for agent {}", agent_id);

        let provider = self.provider();
        let contract = IIdentityRegistryInstance::new(self.registry_address, provider);

        let wallet = contract
            .getAgentWallet(U256::from(agent_id))
            .call()
            .await
            .map_err(|e| WatchyError::BlockchainError(format!("getAgentWallet failed: {}", e)))?;

        // Return None if wallet is zero address
        if wallet._0.is_zero() {
            Ok(None)
        } else {
            Ok(Some(wallet._0))
        }
    }

    /// Get metadata value for a key
    #[allow(dead_code)]
    pub async fn get_metadata(&self, agent_id: u64, key: &str) -> Result<Vec<u8>, WatchyError> {
        debug!("Fetching metadata '{}' for agent {}", key, agent_id);

        let provider = self.provider();
        let contract = IIdentityRegistryInstance::new(self.registry_address, provider);

        let metadata = contract
            .getMetadata(U256::from(agent_id), key.to_string())
            .call()
            .await
            .map_err(|e| WatchyError::BlockchainError(format!("getMetadata failed: {}", e)))?;

        Ok(metadata._0.to_vec())
    }

    /// Get current block number
    pub async fn block_number(&self) -> Result<u64, WatchyError> {
        let provider = self.provider();

        let block_num = provider
            .get_block_number()
            .await
            .map_err(|e| WatchyError::BlockchainError(format!("get_block_number failed: {}", e)))?;

        Ok(block_num)
    }

    /// Check if an address is authorized or owner of an agent
    #[allow(dead_code)]
    pub async fn is_authorized_or_owner(
        &self,
        spender: &str,
        agent_id: u64,
    ) -> Result<bool, WatchyError> {
        let spender_addr = Address::from_str(spender)
            .map_err(|e| WatchyError::InvalidAddress(format!("Invalid spender address: {}", e)))?;

        let provider = self.provider();
        let contract = IIdentityRegistryInstance::new(self.registry_address, provider);

        let is_auth = contract
            .isAuthorizedOrOwner(spender_addr, U256::from(agent_id))
            .call()
            .await
            .map_err(|e| {
                WatchyError::BlockchainError(format!("isAuthorizedOrOwner failed: {}", e))
            })?;

        Ok(is_auth._0)
    }

    #[allow(dead_code)]
    pub fn registry_address(&self) -> &Address {
        &self.registry_address
    }

    /// Register a new agent (mints NFT with empty URI)
    ///
    /// # Arguments
    /// * `private_key` - The private key to sign the transaction (from TEE wallet)
    ///
    /// # Returns
    /// * `(agent_id, tx_hash)` - The newly minted agent ID and transaction hash
    pub async fn register_agent(
        &self,
        private_key: &str,
    ) -> Result<(u64, String), WatchyError> {
        let key = private_key.strip_prefix("0x").unwrap_or(private_key);
        let signer: PrivateKeySigner = key
            .parse()
            .map_err(|e| WatchyError::Internal(format!("Invalid private key: {}", e)))?;

        info!("Registering new agent (empty URI)");

        // Create wallet and provider
        let wallet = EthereumWallet::from(signer);
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(self.rpc_url.clone());

        // Create contract instance
        let contract = IIdentityRegistryInstance::new(self.registry_address, &provider);

        // Call register() - no URI version
        let tx = contract.register_0();

        // Send the transaction
        let pending = tx.send().await.map_err(|e| {
            WatchyError::BlockchainError(format!("Failed to register agent: {}", e))
        })?;

        let tx_hash = format!("0x{}", hex::encode(pending.tx_hash().as_slice()));
        info!("Registration transaction sent: {}", tx_hash);

        // Wait for confirmation and get receipt
        let receipt = pending.get_receipt().await.map_err(|e| {
            WatchyError::BlockchainError(format!("Failed to get receipt: {}", e))
        })?;

        if !receipt.status() {
            return Err(WatchyError::BlockchainError(
                "Registration transaction reverted".to_string(),
            ));
        }

        // Parse the Registered event to get the agent ID
        // The event signature: Registered(uint256 indexed agentId, string agentURI, address indexed owner)
        let agent_id = receipt
            .inner
            .logs()
            .iter()
            .find_map(|log| {
                // The agentId is the first indexed topic (topic[1] after event signature)
                if log.topics().len() >= 2 {
                    let id_bytes = log.topics()[1];
                    Some(U256::from_be_bytes(id_bytes.0).try_into().unwrap_or(0u64))
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                WatchyError::BlockchainError("Could not parse agent ID from event".to_string())
            })?;

        info!(
            "Agent registered: ID {} (tx: {}, block: {})",
            agent_id,
            tx_hash,
            receipt.block_number.unwrap_or_default()
        );

        Ok((agent_id, tx_hash))
    }

    /// Update the metadata URI for an existing agent
    ///
    /// # Arguments
    /// * `agent_id` - The agent token ID
    /// * `uri` - The new URI (e.g., "data:application/json;base64,..." or IPFS/Arweave URL)
    /// * `private_key` - The private key to sign the transaction (from TEE wallet)
    ///
    /// # Returns
    /// * `tx_hash` - The transaction hash
    pub async fn set_agent_uri(
        &self,
        agent_id: u64,
        uri: &str,
        private_key: &str,
    ) -> Result<String, WatchyError> {
        let key = private_key.strip_prefix("0x").unwrap_or(private_key);
        let signer: PrivateKeySigner = key
            .parse()
            .map_err(|e| WatchyError::Internal(format!("Invalid private key: {}", e)))?;

        info!(
            "Updating URI for agent {} ({} bytes)",
            agent_id,
            uri.len()
        );

        // Create wallet and provider
        let wallet = EthereumWallet::from(signer);
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(self.rpc_url.clone());

        // Create contract instance
        let contract = IIdentityRegistryInstance::new(self.registry_address, &provider);

        // Call setAgentURI
        let tx = contract.setAgentURI(U256::from(agent_id), uri.to_string());

        // Send the transaction
        let pending = tx.send().await.map_err(|e| {
            let err_str = e.to_string();
            if err_str.contains("NotAuthorized") || err_str.contains("not authorized") {
                WatchyError::Internal("Not authorized to update this agent's URI".to_string())
            } else {
                WatchyError::BlockchainError(format!("Failed to set agent URI: {}", err_str))
            }
        })?;

        let tx_hash = format!("0x{}", hex::encode(pending.tx_hash().as_slice()));
        info!("setAgentURI transaction sent: {}", tx_hash);

        // Wait for confirmation
        let receipt = pending.get_receipt().await.map_err(|e| {
            WatchyError::BlockchainError(format!("Failed to get receipt: {}", e))
        })?;

        if !receipt.status() {
            return Err(WatchyError::BlockchainError(
                "setAgentURI transaction reverted".to_string(),
            ));
        }

        info!(
            "Agent {} URI updated (tx: {}, block: {})",
            agent_id,
            tx_hash,
            receipt.block_number.unwrap_or_default()
        );

        Ok(tx_hash)
    }
}
