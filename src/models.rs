use mdns_sd::ServiceInfo;
use serde::Serialize;

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
