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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_config_from_reader_valid() {
        let toml_content = r#"
            [[service_mapping]]
            zeroconf_service = "_my-service._mcp._tcp.local."
            protocol = "stdio"
            name = "My Stdio Tool"
            command = "/usr/bin/my_tool"
            args = ["--stdio"]

            [[service_mapping]]
            zeroconf_service = "_sse-service._mcp._tcp.local."
            protocol = "sse"
            name = "My SSE Tool"
            url = "http://localhost:8080/sse"
        "#;
        let config = ZeroConfig::from_reader(toml_content.as_bytes()).unwrap();

        assert_eq!(config.service_mappings.len(), 2);

        let stdio_mapping = &config.service_mappings[0];
        assert_eq!(
            stdio_mapping.zeroconf_service,
            "_my-service._mcp._tcp.local."
        );
        if let McpConfig::Stdio { command, .. } = &stdio_mapping.mcp {
            assert_eq!(command, "/usr/bin/my_tool");
        } else {
            panic!("Expected Stdio config");
        }

        let sse_mapping = &config.service_mappings[1];
        assert_eq!(
            sse_mapping.zeroconf_service,
            "_sse-service._mcp._tcp.local."
        );
        if let McpConfig::Sse { url, .. } = &sse_mapping.mcp {
            assert_eq!(url, "http://localhost:8080/sse");
        } else {
            panic!("Expected Sse config");
        }
    }

    #[test]
    fn test_load_config_from_reader_invalid_toml() {
        let toml_content = "this is not toml";
        let result = ZeroConfig::from_reader(toml_content.as_bytes());
        assert!(result.is_err());
    }

    #[test]
    fn test_load_config_missing_required_field() {
        let toml_content = r#"
            [[service_mapping]]
            # Missing zeroconf_service
            protocol = "stdio"
            name = "My Stdio Tool"
            command = "/usr/bin/my_tool"
            args = ["--stdio"]
        "#;
        let result = ZeroConfig::from_reader(toml_content.as_bytes());
        assert!(result.is_err());
    }
}
