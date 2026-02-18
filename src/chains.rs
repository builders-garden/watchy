use std::collections::HashMap;
use std::sync::LazyLock;

/// Chain type for different blockchain ecosystems
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainType {
    Evm,
    Solana,
}

/// Configuration for a supported chain
#[derive(Debug, Clone)]
pub struct ChainConfig {
    pub chain_id: u64,
    pub name: &'static str,
    pub chain_type: ChainType,
    pub registry_address: Option<&'static str>,
    pub reputation_address: Option<&'static str>,
    pub rpcs: Vec<&'static str>,
    #[allow(dead_code)]
    pub block_explorer: &'static str,
}

impl ChainConfig {
    /// Get the first available RPC URL
    pub fn primary_rpc(&self) -> Option<&str> {
        self.rpcs.first().copied()
    }

    /// Check if this chain has a deployed identity registry
    pub fn has_registry(&self) -> bool {
        self.registry_address.is_some()
    }

    /// Check if this chain has a deployed reputation registry
    #[allow(dead_code)]
    pub fn has_reputation(&self) -> bool {
        self.reputation_address.is_some()
    }
}

/// Static registry of all supported chains
pub static CHAINS: LazyLock<HashMap<u64, ChainConfig>> = LazyLock::new(|| {
    let chains = vec![
        // ===== MAINNETS =====
        ChainConfig {
            chain_id: 8453,
            name: "base",
            chain_type: ChainType::Evm,
            registry_address: Some("0x8004A169FB4a3325136EB29fA0ceB6D2e539a432"),
            reputation_address: Some("0x8004BAa17C55a88189AE136b182e5fdA19dE9b63"),
            rpcs: vec![
                "https://mainnet.base.org",
                "https://base.llamarpc.com",
                "https://base.drpc.org",
                "https://base-mainnet.public.blastapi.io",
            ],
            block_explorer: "https://basescan.org",
        },
        ChainConfig {
            chain_id: 1,
            name: "ethereum",
            chain_type: ChainType::Evm,
            registry_address: Some("0x8004A169FB4a3325136EB29fA0ceB6D2e539a432"),
            reputation_address: Some("0x8004BAa17C55a88189AE136b182e5fdA19dE9b63"),
            rpcs: vec![
                "https://eth.llamarpc.com",
                "https://ethereum.publicnode.com",
                "https://rpc.ankr.com/eth",
                "https://eth.drpc.org",
            ],
            block_explorer: "https://etherscan.io",
        },
        // ===== TESTNETS =====
        ChainConfig {
            chain_id: 84532,
            name: "base-sepolia",
            chain_type: ChainType::Evm,
            registry_address: Some("0x8004A818BFB912233c491871b3d84c89A494BD9e"),
            reputation_address: Some("0x8004B663056A597Dffe9eCcC1965A193B7388713"),
            rpcs: vec![
                "https://sepolia.base.org",
                "https://base-sepolia.drpc.org",
                "https://base-sepolia.publicnode.com",
            ],
            block_explorer: "https://sepolia.basescan.org",
        },
        ChainConfig {
            chain_id: 11155111,
            name: "sepolia",
            chain_type: ChainType::Evm,
            registry_address: Some("0x8004A818BFB912233c491871b3d84c89A494BD9e"),
            reputation_address: Some("0x8004B663056A597Dffe9eCcC1965A193B7388713"),
            rpcs: vec![
                "https://sepolia.drpc.org",
                "https://ethereum-sepolia.publicnode.com",
                "https://rpc.ankr.com/eth_sepolia",
            ],
            block_explorer: "https://sepolia.etherscan.io",
        },
        // ===== SOLANA =====
        ChainConfig {
            chain_id: 101, // Solana mainnet-beta (unofficial ID for our purposes)
            name: "solana",
            chain_type: ChainType::Solana,
            registry_address: None, // Solana program address when deployed
            reputation_address: None,
            rpcs: vec![
                "https://api.mainnet-beta.solana.com",
                "https://solana-api.projectserum.com",
            ],
            block_explorer: "https://solscan.io",
        },
        ChainConfig {
            chain_id: 103, // Solana devnet (unofficial ID for our purposes)
            name: "solana-devnet",
            chain_type: ChainType::Solana,
            registry_address: None,
            reputation_address: None,
            rpcs: vec![
                "https://api.devnet.solana.com",
            ],
            block_explorer: "https://solscan.io/?cluster=devnet",
        },
    ];

    chains.into_iter().map(|c| (c.chain_id, c)).collect()
});

/// Get chain config by chain ID
pub fn get_chain(chain_id: u64) -> Option<&'static ChainConfig> {
    CHAINS.get(&chain_id)
}

/// Get chain config by name
#[allow(dead_code)]
pub fn get_chain_by_name(name: &str) -> Option<&'static ChainConfig> {
    CHAINS.values().find(|c| c.name == name)
}

/// List all supported chain IDs
pub fn supported_chain_ids() -> Vec<u64> {
    CHAINS.keys().copied().collect()
}

/// List all EVM chains with deployed registries
pub fn chains_with_registry() -> Vec<&'static ChainConfig> {
    CHAINS
        .values()
        .filter(|c| c.has_registry())
        .collect()
}

/// Get RPC URL for a chain, with optional env override
/// Checks for RPC_URL_{CHAIN_NAME} env var first
pub fn get_rpc_url(chain_id: u64) -> Option<String> {
    let chain = get_chain(chain_id)?;

    // Check for env override: RPC_URL_BASE, RPC_URL_ETHEREUM, etc.
    let env_key = format!("RPC_URL_{}", chain.name.to_uppercase().replace('-', "_"));
    if let Ok(url) = std::env::var(&env_key) {
        return Some(url);
    }

    // Fall back to first configured RPC
    chain.primary_rpc().map(|s| s.to_string())
}

/// Get all RPC URLs for a chain (env override + defaults)
pub fn get_all_rpcs(chain_id: u64) -> Vec<String> {
    let Some(chain) = get_chain(chain_id) else {
        return vec![];
    };

    let mut rpcs = Vec::new();

    // Add env override first if present
    let env_key = format!("RPC_URL_{}", chain.name.to_uppercase().replace('-', "_"));
    if let Ok(url) = std::env::var(&env_key) {
        rpcs.push(url);
    }

    // Add all default RPCs
    rpcs.extend(chain.rpcs.iter().map(|s| s.to_string()));

    rpcs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_base_chain() {
        let chain = get_chain(8453).unwrap();
        assert_eq!(chain.name, "base");
        assert!(chain.has_registry());
        assert_eq!(
            chain.registry_address,
            Some("0x8004A169FB4a3325136EB29fA0ceB6D2e539a432")
        );
    }

    #[test]
    fn test_get_chain_by_name() {
        let chain = get_chain_by_name("ethereum").unwrap();
        assert_eq!(chain.chain_id, 1);
    }

    #[test]
    fn test_chains_with_registry() {
        let chains = chains_with_registry();
        // All EVM chains have registries deployed
        assert!(chains.iter().any(|c| c.name == "base"));
        assert!(chains.iter().any(|c| c.name == "ethereum"));
        assert!(chains.iter().any(|c| c.name == "base-sepolia"));
        assert!(chains.iter().any(|c| c.name == "sepolia"));
        // Solana doesn't have a registry yet
        assert!(!chains.iter().any(|c| c.name == "solana"));
    }

    #[test]
    fn test_solana_chain() {
        let chain = get_chain(101).unwrap();
        assert_eq!(chain.name, "solana");
        assert_eq!(chain.chain_type, ChainType::Solana);
    }
}
