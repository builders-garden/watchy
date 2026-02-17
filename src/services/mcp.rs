use serde::{Deserialize, Serialize};

/// MCP (Model Context Protocol) server manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPManifest {
    #[serde(default)]
    pub name: Option<String>,

    #[serde(default)]
    pub version: Option<String>,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub tools: Vec<MCPTool>,

    #[serde(default)]
    pub prompts: Vec<MCPPrompt>,

    #[serde(default)]
    pub resources: Vec<MCPResource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPTool {
    pub name: String,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub input_schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPPrompt {
    pub name: String,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub arguments: Vec<MCPPromptArgument>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPPromptArgument {
    pub name: String,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPResource {
    pub uri: String,

    #[serde(default)]
    pub name: Option<String>,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub mime_type: Option<String>,
}

impl MCPManifest {
    /// Get all tool names
    pub fn tool_names(&self) -> Vec<&str> {
        self.tools.iter().map(|t| t.name.as_str()).collect()
    }

    /// Get all prompt names
    pub fn prompt_names(&self) -> Vec<&str> {
        self.prompts.iter().map(|p| p.name.as_str()).collect()
    }

    /// Check if manifest has the declared tools
    pub fn has_tools(&self, declared: &[String]) -> bool {
        let actual: Vec<&str> = self.tool_names();
        declared.iter().all(|d| actual.contains(&d.as_str()))
    }
}
