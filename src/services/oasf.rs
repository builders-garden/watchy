#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// OASF (Open Agent Skills Framework) descriptor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OASFDescriptor {
    #[serde(default)]
    pub version: Option<String>,

    #[serde(default)]
    pub skills: Vec<OASFSkill>,

    #[serde(default)]
    pub domains: Vec<OASFDomain>,
}

/// OASF Skill - can be a string path or structured object
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OASFSkill {
    Path(String),
    Structured {
        name: String,
        #[serde(default)]
        id: Option<u32>,
        #[serde(default)]
        description: Option<String>,
    },
}

/// OASF Domain - can be a string path or structured object
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OASFDomain {
    Path(String),
    Structured {
        name: String,
        #[serde(default)]
        id: Option<u32>,
        #[serde(default)]
        description: Option<String>,
    },
}

impl OASFSkill {
    pub fn name(&self) -> &str {
        match self {
            OASFSkill::Path(p) => p,
            OASFSkill::Structured { name, .. } => name,
        }
    }
}

impl OASFDomain {
    pub fn name(&self) -> &str {
        match self {
            OASFDomain::Path(p) => p,
            OASFDomain::Structured { name, .. } => name,
        }
    }
}

impl OASFDescriptor {
    /// Get all skill names/paths
    pub fn skill_names(&self) -> Vec<&str> {
        self.skills.iter().map(|s| s.name()).collect()
    }

    /// Get all domain names/paths
    pub fn domain_names(&self) -> Vec<&str> {
        self.domains.iter().map(|d| d.name()).collect()
    }
}
