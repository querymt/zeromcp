use anyhow::Result;
use std::{
    env,
    io::{self, Write},
    process,
};
use tokio::sync::mpsc;
use zeromcp::{ClientNotification, ServiceManager, ZeroConfig};

#[tokio::main]
async fn main() -> Result<()> {
    // 1. --- Load Configuration ---
    // The client application is responsible for loading the configuration.
    // The library provides a helper `AppConfig::load` for this.
    let args: Vec<String> = env::args().collect();

    // Expecting 2 arguments: program name + 1 path argument
    if args.len() != 2 {
        eprintln!("Usage: {} <path>", args[0]);
        process::exit(1);
    }

    let config_path = &args[1];
    println!("Loading configuration from '{}'...", config_path);
    let config = ZeroConfig::load(config_path)?;
    println!("Configuration loaded successfully.");

    // 2. --- Set up Communication Channel ---
    // A channel is created to allow the library's ServiceManager (running in the background)
    // to send notifications to the client (this main function).
    let (notification_tx, mut notification_rx) = mpsc::channel(10);

    // 3. --- Initialize and Run the Service Manager ---
    // Create an instance of the ServiceManager, giving it the config and the
    // sending half of the notification channel.
    let manager = ServiceManager::new(config, notification_tx)?;
    println!("\n--- Starting Service Manager ---");
    println!("The library is now continuously monitoring the network.");
    println!("Press Ctrl+C to exit.");

    // Spawn the manager's main loop as a background task. From this point on,
    // the library is independently handling all discovery and process management.
    tokio::spawn(async move {
        if let Err(e) = manager.run().await {
            eprintln!("[ERROR] Service Manager failed: {}", e);
        }
    });

    // 4. --- Handle Notifications from the Library ---
    // The client's main loop waits for messages from the library and reacts accordingly.
    // This is the core of the client-side implementation.
    while let Some(notification) = notification_rx.recv().await {
        match notification {
            // The library informs us that it started an MCP process.
            ClientNotification::McpStarted { service_name } => {
                println!("[CLIENT] ==> MCP process for '{}'", service_name);
            }
            // The library informs us that it stopped an MCP process.
            ClientNotification::McpStopped {
                service_name,
                reason,
            } => {
                println!(
                    "[CLIENT] ==> MCP process for '{}' stopped. Reason: {}",
                    service_name, reason
                );
            }
            // The library needs an API key or other secret to proceed.
            ClientNotification::InputRequired {
                service_name,
                key,
                response_tx,
            } => {
                println!("[CLIENT] ==> Input needed for service '{}'", service_name);
                print!("[CLIENT] Please provide a value for '{}': ", key);
                io::stdout().flush().unwrap();

                let mut user_input = String::new();
                io::stdin().read_line(&mut user_input).unwrap();

                // The client sends the user's input back to the library via the
                // provided one-shot channel. The library task that was paused
                // will now resume with this information.
                if response_tx.send(user_input.trim().to_string()).is_err() {
                    eprintln!(
                        "[CLIENT] Failed to send input back to the library. The request may have timed out."
                    );
                }
            }
        }
    }

    Ok(())
}
