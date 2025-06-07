use anyhow::{Context, Result};
use serde::Deserialize;
use std::{collections::HashMap, io::Read, path::Path};

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
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("read config from {:?}", path.as_ref()))?;
        toml::from_str(&content).context("parse zeroMCP config")
    }

    pub fn from_reader<R: Read>(reader: R) -> Result<Self> {
        // Parse TOML from any reader:
        let mut buf = String::new();
        let mut rdr = reader;
        rdr.read_to_string(&mut buf)?;
        toml::from_str(&buf).context("parse zeroMCP config from reader")
    }
}
