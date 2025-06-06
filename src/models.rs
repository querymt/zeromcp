use mdns_sd::ServiceInfo;
use serde::Serialize;
use tokio::sync::oneshot;

/// Represents a discovered service, simplified for this library's use.
#[derive(Debug, Clone, Serialize)]
pub struct DiscoveredService {
    pub fullname: String,
    pub hostname: String,
    pub port: u16,
    pub addresses: Vec<String>,
}

impl From<&ServiceInfo> for DiscoveredService {
    fn from(info: &ServiceInfo) -> Self {
        DiscoveredService {
            fullname: info.get_fullname().to_string(),
            hostname: info.get_hostname().to_string(),
            port: info.get_port(),
            addresses: info
                .get_addresses()
                .iter()
                .map(|ip| ip.to_string())
                .collect(),
        }
    }
}

/// Events sent from the library back to the client application.
#[derive(Debug)]
pub enum ClientNotification {
    /// A managed MCP process has been successfully started.
    McpStarted { service_name: String },
    /// A managed MCP process has been stopped.
    McpStopped {
        service_name: String,
        reason: String,
    },
    /// The library requires input from the user to proceed.
    InputRequired {
        service_name: String,
        key: String,
        /// The client MUST use this channel to send the required string back.
        response_tx: oneshot::Sender<String>,
    },
}
