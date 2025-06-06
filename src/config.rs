use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;

/// Represents the top-level configuration loaded from a TOML file.
#[derive(Deserialize, Debug, Clone)]
pub struct ZeroConfig {
    #[serde(rename = "service_mapping")]
    pub service_mappings: Vec<ServiceMcpMapping>,
}

/// Defines a mapping between a Zeroconf service and its MCP configuration.
#[derive(Deserialize, Debug, Clone)]
pub struct ServiceMcpMapping {
    pub zeroconf_service: String,
    #[serde(flatten)]
    pub mcp: McpConfig,
}

/// Contains the template for launching an MCP server process.
#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "protocol", rename_all = "lowercase")]
pub enum McpConfig {
    Stdio {
        name: String,
        command: String,
        args: Vec<String>,
        #[serde(default)]
        envs: HashMap<String, String>,
    },
    Sse {
        name: String,
        url: String,
        headers: Option<HashMap<String, String>>,
    },
}

impl ZeroConfig {
    /// Loads configuration from a TOML file.
    pub fn load(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file at '{}'", path))?;
        toml::from_str(&content).with_context(|| format!("Failed to parse TOML from '{}'", path))
    }
}
