# ZeroMCP

ZeroMCP is a Rust library and CLI example that bridges mDNS/ZeroConf service discovery with Model Context Protocol (MCP) clients.
It automatically discovers services on your local network, launches or connects to their MCP endpoints (via stdio or Server‐Sent Events), manages their lifecycle,
and notifies your application about start/stop events or missing templating inputs.

## Features

• Automatic mDNS/ZeroConf service discovery
• Dynamic MCP client spawning over stdio or SSE
• Handlebars‐based templating for commands, URLs, headers & envs
• Interactive callbacks when templates reference missing variables
• Lifecycle management: start on discovery, stop on removal
• Async notification callbacks for `McpStarted`, `McpStopped` & `InputRequired`
• `ZeroClient` API to list tools or cancel services at runtime

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
zeromcp = "0.1"
```

Or point to the Git repo:

```toml
zeromcp = { git = "https://github.com/querymt/zeromcp.git" }
```

## Configuration

ZeroMCP loads a TOML file describing one or more `service_mapping` entries:

```toml
[[service_mapping]]
zeroconf_service = "_home-assistant._tcp.local."

[service_mapping.mcp]
protocol = "stdio"
name     = "HomeAssistant MCP"
command  = "uvx"
args     = [
  "mcp-proxy",
  "http://{{service.hostname}}:{{service.port}}/mcp_server/sse"
]
[service_mapping.mcp.envs]
API_TOKEN = "{{HOME_ASSISTANT_TOKEN}}"

[[service_mapping]]
zeroconf_service = "_custom-mcp._tcp.local."

[service_mapping.mcp]
protocol = "sse"
name     = "Custom MCP"
url      = "https://{{service.hostname}}:{{service.port}}/mcp/sse"
[service_mapping.mcp.headers]
Authorization = "Bearer {{API_TOKEN}}"
```

- `service.hostname`, `service.port`, `service.fullname` and `service.addresses` come from mDNS.
- Custom placeholders (e.g. `{{API_TOKEN}}`) trigger an `InputRequired` callback if missing.

## Quickstart

```rust
use anyhow::Result;
use async_trait::async_trait;
use std::{io::{self, Write}, sync::Arc};
use rmcp::service::QuitReason;
use zeromcp::{ZeroClient, ZeroConfig, ZeroHandler, DiscoveredService, ServiceEventHandler, UserInputProvider};

/// Your application holds a `ZeroClient` to interact with running services.
struct MyApp {
    client: ZeroClient,
}

#[async_trait]
impl ServiceEventHandler for MyApp {
    async fn on_service_started(&self, svc: &DiscoveredService) {
        println!("Service started: {}", svc.fullname);
        match self.client.list_all_tools(&svc.fullname).await {
            Ok(tools) => for t in tools {
                println!(" - {}: {}", t.name, t.description);
            },
            Err(e) => eprintln!("Failed to list tools: {}", e),
        }
    }

    async fn on_service_stopped(&self, svc_name: &str, reason: QuitReason) {
        println!("Service stopped: {} ({:?})", svc_name, reason);
    }
}

#[async_trait]
impl UserInputProvider for MyApp {
    async fn request_input(&self, service_name: &str, key: &str) -> Result<String> {
        print!("{} needs '{}': ", service_name, key);
        io::stdout().flush()?;
        let mut buf = String::new();
        io::stdin().read_line(&mut buf)?;
        Ok(buf.trim().to_string())
    }
}

// Combine the two into `ZeroHandler`
impl ZeroHandler for MyApp {}

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Load your TOML config
    let config = ZeroConfig::load("config.toml")?;

    // 2. Start ZeroMCP, providing a factory for your handler
    let zeromcp = zeromcp::start(config, |client| {
        Arc::new(MyApp { client })
    })
    .await?;

    println!("ZeroMCP running. Ctrl+C to exit.");

    // 3. Shutdown when done
    zeromcp.shutdown().await?;
    Ok(())
}
```

## API Overview

```rust
// Load a TOML config
let config = ZeroConfig::load("config.toml")?;

// Start the manager with your handler factory
let zeromcp = zeromcp::start(config, |client: ZeroClient| {
    Arc::new(MyHandler { client })
}).await?;

// Interact programmatically:
let tools = zeromcp.client().list_all_tools("MyService._mcp._tcp.local.").await?;
let reason = zeromcp.client().stop_service("MyService._mcp._tcp.local.").await?;

// Shutdown gracefully
zeromcp.shutdown().await?;
```

Key types:
- `ZeroConfig` – parse your service mappings from TOML
- `McpConfig` – `Stdio { command, args, envs }` or `Sse { url, headers }`
- `ZeroHandler` – your application logic (`ServiceEventHandler + UserInputProvider`)
- `ZeroClient` – async API (`list_all_tools`, `stop_service`)
- `start(config, factory)` → `ZeroMcp` with `client()` & `shutdown()`

---

Happy service‐hunting!
