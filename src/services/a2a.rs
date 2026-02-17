use serde::{Deserialize, Serialize};

/// A2A Agent Card structure (Google's Agent-to-Agent protocol)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AAgentCard {
    pub name: String,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub skills: Vec<A2ASkill>,

    #[serde(default)]
    pub capabilities: Option<A2ACapabilities>,

    #[serde(default)]
    pub url: Option<String>,

    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2ASkill {
    pub id: String,

    #[serde(default)]
    pub name: Option<String>,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2ACapabilities {
    #[serde(default)]
    pub streaming: bool,

    #[serde(default)]
    pub push_notifications: bool,

    #[serde(default)]
    pub state_transition_history: bool,
}

impl A2AAgentCard {
    /// Validate the agent card has minimum required fields
    pub fn is_valid(&self) -> bool {
        !self.name.is_empty()
    }

    /// Get all skill identifiers
    pub fn skill_ids(&self) -> Vec<&str> {
        self.skills.iter().map(|s| s.id.as_str()).collect()
    }
}
