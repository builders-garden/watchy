use tracing::{debug, info, warn};

use crate::blockchain::registry::RegistryClient;
use crate::chains::get_all_rpcs;
use crate::types::WatchyError;

/// On-chain data fetched for an agent
pub struct OnchainData {
    pub exists: bool,
    pub metadata_uri: String,
    pub owner: String,
    pub wallet: Option<String>,
    pub block_number: u64,
}

/// Fetch on-chain data for an agent with RPC failover
pub async fn fetch_onchain_data(
    chain_id: u64,
    agent_id: u64,
    registry_address: &str,
) -> Result<OnchainData, WatchyError> {
    debug!(
        "Fetching on-chain data for agent {} from registry {} on chain {}",
        agent_id, registry_address, chain_id
    );

    // Get all RPC URLs for chain (env override + defaults)
    let rpcs = get_all_rpcs(chain_id);
    if rpcs.is_empty() {
        return Err(WatchyError::Internal(format!(
            "No RPC URLs available for chain {}",
            chain_id
        )));
    }

    let mut last_error = String::new();

    // Try each RPC until one succeeds
    for (i, rpc_url) in rpcs.iter().enumerate() {
        debug!("Trying RPC {}/{}: {}", i + 1, rpcs.len(), rpc_url);

        match try_fetch_onchain_data(rpc_url, registry_address, agent_id).await {
            Ok(data) => {
                if i > 0 {
                    info!("RPC {} succeeded after {} failures", rpc_url, i);
                }
                return Ok(data);
            }
            Err(e) => {
                warn!("RPC {} failed: {}", rpc_url, e);
                last_error = e.to_string();
            }
        }
    }

    Err(WatchyError::Internal(format!(
        "All {} RPCs failed for chain {}. Last error: {}",
        rpcs.len(),
        chain_id,
        last_error
    )))
}

/// Try to fetch on-chain data from a single RPC
async fn try_fetch_onchain_data(
    rpc_url: &str,
    registry_address: &str,
    agent_id: u64,
) -> Result<OnchainData, WatchyError> {
    // Create registry client
    let registry = RegistryClient::new(rpc_url, registry_address)?;

    // Get current block number first
    let block_number = registry.block_number().await?;
    info!("Current block number: {}", block_number);

    // Check if agent exists
    let exists = registry.agent_exists(agent_id).await?;
    if !exists {
        return Err(WatchyError::AgentNotFound(agent_id));
    }

    // Get owner
    let owner = registry.owner_of(agent_id).await?;
    debug!("Agent {} owner: {}", agent_id, owner);

    // Get metadata URI
    let metadata_uri = registry.token_uri(agent_id).await?;
    debug!("Agent {} metadata URI: {}", agent_id, metadata_uri);

    // Get agent wallet (optional)
    let wallet = registry.get_agent_wallet(agent_id).await?;
    debug!("Agent {} wallet: {:?}", agent_id, wallet);

    Ok(OnchainData {
        exists,
        metadata_uri,
        owner: format!("{:?}", owner),
        wallet: wallet.map(|w| format!("{:?}", w)),
        block_number,
    })
}
