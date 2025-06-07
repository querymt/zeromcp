//! # ZeroMCP: A Zeroconf-based Service Manager for MCP
//!
//! This library provides a high-level manager for discovering and interacting with
//! services on the local network using Zeroconf (mDNS-SD) and the Media Control Protocol (MCP).
//!
//! It handles the complexities of network discovery, service lifecycle management,
//! and communication, allowing you to focus on your application's logic.
//!
//! ## Key Concepts
//!
//! - **`ZeroConfig`**: A configuration struct, typically loaded from a TOML file, that maps
//!   Zeroconf service types (e.g., `_mcp._tcp`) to instructions on how to launch and
//!   interact with the corresponding MCP service process (e.g., via stdio or SSE).
//!
//! - **`ZeroHandler` Trait**: A user-implemented trait that combines `ServiceEventHandler` and
//!   `UserInputProvider`. This is the primary way your application logic integrates with the
//!   library, reacting to events and providing data when needed.
//!
//! - **`ZeroClient`**: A client handle that provides an async API to interact with discovered
//!   and running services (e.g., listing tools, stopping services). An instance of this is
//!   passed to your `ZeroHandler` implementation upon creation.
//!
//! ## Quickstart Example
//!
//! ```no_run
//! use anyhow::Result;
//! use async_trait::async_trait;
//! use rmcp::service::QuitReason;
//! use std::{sync::Arc, io::{self, Write}};
//! use zeromcp::{ZeroClient, ZeroConfig, ZeroHandler, DiscoveredService, ServiceEventHandler, UserInputProvider};
//! use tracing::{info, error};
//!
//! // 1. Define your application state.
//! // It can hold the ZeroClient to interact with the manager.
//! struct MyApplication {
//!     client: ZeroClient,
//! }
//!
//! // 2. Implement the ServiceEventHandler and UserInputProvider traits.
//! #[async_trait]
//! impl ServiceEventHandler for MyApplication {
//!     async fn on_service_started(&self, service: &DiscoveredService) {
//!         info!("[HANDLER] Service started: {}", service.fullname);
//!         // Now you can use the client!
//!         match self.client.list_tools(&service.fullname).await {
//!             Ok(tools) => info!("[HANDLER] Found tools: {:?}", tools),
//!             Err(e) => error!("[HANDLER] Failed to list tools: {}", e),
//!         }
//!     }
//!
//!     async fn on_service_stopped(&self, service_name: &str, reason: QuitReason) {
//!         info!("[HANDLER] Service stopped: {}. Reason: {:?}", service_name, reason);
//!     }
//! }
//!
//! #[async_trait]
//! impl UserInputProvider for MyApplication {
//!     async fn request_input(&self, service_name: &str, key: &str) -> Result<String> {
//!         print!("[HANDLER] Please provide a value for '{}' for service '{}': ", key, service_name);
//!         io::stdout().flush()?;
//!         let mut input = String::new();
//!         io::stdin().read_line(&mut input)?;
//!         Ok(input.trim().to_string())
//!     }
//! }
//!
//! // 3. Combine them using the `ZeroHandler` trait.
//! impl ZeroHandler for MyApplication {}
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     tracing_subscriber::fmt::init();
//!
//!     // 4. Load configuration.
//!     let config = ZeroConfig::load("path/to/your/config.toml")?;
//!
//!     // 5. Start the manager with a factory closure.
//!     // The library creates the client and passes it to your app's constructor.
//!     let zeromcp = zeromcp::start(config, |client| {
//!         Arc::new(MyApplication { client })
//!     }).await?;
//!
//!     info!("ZeroMCP is running. Press Ctrl+C to exit.");
//!
//!     // When ready to shut down:
//!     zeromcp.shutdown().await?;
//!
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod config;
pub mod events;
pub mod manager;
pub mod models;
mod utils;

// Re-export public-facing components.
pub use client::ZeroClient;
pub use config::ZeroConfig;
pub use events::{ServiceEventHandler, UserInputProvider, ZeroHandler};
pub use manager::start;
pub use models::DiscoveredService;
