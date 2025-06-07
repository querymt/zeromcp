use anyhow::Result;
use async_trait::async_trait;
use rmcp::service::QuitReason;
use std::{
    env,
    io::{self, Write},
    process,
    sync::Arc,
};
use tracing::{error, info, instrument};
use zeromcp::{
    DiscoveredService, ServiceEventHandler, UserInputProvider, ZeroClient, ZeroConfig, ZeroHandler,
};

struct MyApplication {
    client: ZeroClient,
}

#[async_trait]
impl ServiceEventHandler for MyApplication {
    /// This is called by the library when a service is ready.
    #[instrument(name="on_service_started_handler", skip(self, service), fields(service.name = %service.fullname))]
    async fn on_service_started(&self, service: &DiscoveredService) {
        info!("[HANDLER] ==> Service started, querying for its tools...");

        match self.client.list_all_tools(&service.fullname).await {
            Ok(tools) => {
                info!("[HANDLER] <== Successfully retrieved tools:");
                for tool in tools {
                    println!(
                        "      - Name: '{:?}', Desc: '{:?}'",
                        tool.name, tool.description
                    );
                }
            }
            Err(e) => {
                error!("[HANDLER] <== Failed to list tools: {}", e);
            }
        }
    }

    /// This is called by the library when a service stops.
    #[instrument(name="on_service_stopped_handler", skip(self), fields(service.name = %service_name))]
    async fn on_service_stopped(&self, service_name: &str, reason: QuitReason) {
        info!("[HANDLER] ==> Service stopped. Reason: {:?}", reason);
    }
}

#[async_trait]
impl UserInputProvider for MyApplication {
    /// This is called by the library when it needs input for a template.
    async fn request_input(&self, service_name: &str, key: &str) -> Result<String> {
        info!("[HANDLER] ==> Input needed for service '{}'", service_name);
        print!("[HANDLER] Please provide a value for '{}': ", key);
        io::stdout().flush()?;

        let mut user_input = String::new();
        io::stdin().read_line(&mut user_input)?;

        Ok(user_input.trim().to_string())
    }
}

// Implement the blanket `App` trait for our struct.
impl ZeroHandler for MyApplication {}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <path>", args[0]);
        process::exit(1);
    }

    let config_path = &args[1];
    println!("Loading configuration from '{}'...", config_path);
    let config = ZeroConfig::load(config_path)?;
    println!("Configuration loaded successfully.");

    // --- Initialize and Run the Service Manager ---
    // The `start` function now takes a factory closure. This is where the magic happens.
    // The library creates the client, passes it to our closure, and we construct
    // our application state with it.
    let mcp = zeromcp::start(config, |client| Arc::new(MyApplication { client })).await?;

    info!("\n--- Starting ZeroMCP ---");
    info!("The library is now continuously monitoring the network.");
    info!("Press Ctrl+C to exit.");

    // Wait for the manager to finish (e.g., on Ctrl+C or a critical error).
    mcp.shutdown().await?;

    Ok(())
}
