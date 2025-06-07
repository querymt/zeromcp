use crate::models::DiscoveredService;
use anyhow::Result;
use async_trait::async_trait;
use rmcp::service::QuitReason;

/// A trait for handling service lifecycle events.
///
/// Implement this trait to react to services appearing and disappearing on the network.
#[async_trait]
pub trait ServiceEventHandler: Send + Sync {
    /// Called when a new service has been discovered, configured, and is now running.
    async fn on_service_started(&self, service: &DiscoveredService);

    /// Called when a running service has been stopped.
    async fn on_service_stopped(&self, service_name: &str, reason: QuitReason);
}

/// A trait for providing user input when required by the library.
///
/// Implement this trait to provide a mechanism (e.g., CLI prompt, GUI dialog)
/// for resolving template variables at runtime.
#[async_trait]
pub trait UserInputProvider: Send + Sync {
    /// Called when a template variable needs to be resolved.
    ///
    /// # Arguments
    /// * `service_name` - The name of the service requiring input.
    /// * `key` - The name of the variable that needs a value.
    ///
    /// # Returns
    /// A `Result` containing the string value provided by the user.
    async fn request_input(&self, service_name: &str, key: &str) -> Result<String>;
}

/// A convenient super-trait that combines `ServiceEventHandler` and `UserInputProvider`.
///
/// This is the recommended trait for your main application struct to implement.
/// It simplifies the setup process by requiring only one logical "handler" object.
#[async_trait]
pub trait ZeroHandler: ServiceEventHandler + UserInputProvider {}
