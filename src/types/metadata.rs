use serde::{Deserialize, Serialize};

pub const EIP8004_TYPE: &str = "https://eips.ethereum.org/EIPS/eip-8004#registration-v1";

/// EIP-8004 Agent Metadata (off-chain JSON)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetadata {
    #[serde(rename = "type")]
    pub metadata_type: Option<String>,

    pub name: Option<String>,

    pub description: Option<String>,

    pub image: Option<String>,

    #[serde(default)]
    pub services: Vec<Service>,

    #[serde(default)]
    pub registrations: Vec<Registration>,

    #[serde(default, alias = "supportedTrust")]
    pub supported_trust: Vec<String>,

    #[serde(alias = "x402Support", alias = "x402support")]
    pub x402_support: Option<bool>,

    pub active: Option<bool>,

    #[serde(alias = "updatedAt")]
    pub updated_at: Option<u64>,

    // Optional extended fields
    pub version: Option<String>,

    #[serde(alias = "agentType")]
    pub agent_type: Option<String>,

    #[serde(alias = "sourceCode")]
    pub source_code: Option<String>,

    pub documentation: Option<String>,

    pub author: Option<Author>,

    pub license: Option<String>,

    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub name: String,

    pub endpoint: Option<String>,

    pub version: Option<String>,

    // A2A specific
    #[serde(default, alias = "a2aSkills")]
    pub a2a_skills: Vec<String>,

    // MCP specific
    #[serde(default, alias = "mcpTools")]
    pub mcp_tools: Vec<String>,

    #[serde(default, alias = "mcpPrompts")]
    pub mcp_prompts: Vec<String>,

    // OASF specific
    #[serde(default)]
    pub skills: Vec<serde_json::Value>, // Can be string or object

    #[serde(default)]
    pub domains: Vec<serde_json::Value>, // Can be string or object
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registration {
    #[serde(alias = "agentId")]
    pub agent_id: u64,

    #[serde(alias = "agentRegistry")]
    pub agent_registry: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    pub name: Option<String>,
    pub url: Option<String>,
    pub twitter: Option<String>,
}

impl AgentMetadata {
    /// Check if required fields are present and non-empty
    pub fn has_required_fields(&self) -> bool {
        self.metadata_type.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false)
            && self.name.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false)
            && self.description.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false)
            && self.image.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false)
            && !self.registrations.is_empty()
    }

    /// Check if type field matches EIP-8004
    pub fn has_valid_type(&self) -> bool {
        self.metadata_type
            .as_ref()
            .map(|t| t == EIP8004_TYPE)
            .unwrap_or(false)
    }

    /// Find registration matching the given agent ID and registry
    /// Registry format expected: "eip155:<chainId>:<address>"
    pub fn find_registration(&self, agent_id: u64, registry: &str) -> Option<&Registration> {
        let registry_lower = registry.to_lowercase();

        self.registrations.iter().find(|r| {
            if r.agent_id != agent_id {
                return false;
            }

            let r_registry_lower = r.agent_registry.to_lowercase();

            // Exact match
            if r_registry_lower == registry_lower {
                return true;
            }

            // Parse CAIP-10 format: eip155:<chainId>:<address>
            // Allow matching if chain and address match (ignoring case)
            let r_parts: Vec<&str> = r_registry_lower.split(':').collect();
            let registry_parts: Vec<&str> = registry_lower.split(':').collect();

            // Both should have format eip155:chainId:address
            if r_parts.len() == 3 && registry_parts.len() == 3 {
                // Match namespace (eip155), chain_id, and full address
                r_parts[0] == registry_parts[0]
                    && r_parts[1] == registry_parts[1]
                    && r_parts[2] == registry_parts[2]
            } else {
                false
            }
        })
    }
}

/// Service type enum for easier handling
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceType {
    A2A,
    MCP,
    OASF,
    Web,
    Twitter,
    Email,
    Unknown(String),
}

impl From<&str> for ServiceType {
    fn from(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "A2A" => ServiceType::A2A,
            "MCP" => ServiceType::MCP,
            "OASF" => ServiceType::OASF,
            "WEB" => ServiceType::Web,
            "TWITTER" => ServiceType::Twitter,
            "EMAIL" => ServiceType::Email,
            other => ServiceType::Unknown(other.to_string()),
        }
    }
}
