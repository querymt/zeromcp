# ZeroMCP

ZeroMCP is a Rust library and example CLI application that bridges mDNS/ZeroConf service discovery with Model Context Protocol (MCP) clients. It watches for services advertised on the local network, instantiates MCP clients (over stdio or SSE), manages their lifecycles, and notifies your application about process start/stop events and runtime input requirements.

---

## Table of Contents

1. [Features](#features)
2. [Getting Started](#getting-started)
3. [Installation](#installation)
4. [Configuration](#configuration)
5. [Example CLI Usage](#example-cli-usage)
6. [Library API](#library-api)
7. [Contributing](#contributing)

---

## Features

- **Automatic Service Discovery**
  Uses mDNS/ZeroConf to browse for configured service types.

- **Dynamic MCP Client Launch**
  Supports two protocols for MCP clients:
  - **stdio**: Spawns child processes and proxies stdio streams.
  - **sse**: Connects via Server‐Sent Events.

- **Templated Configuration**
  Command arguments, environment variables, and SSE URLs are templatable using [Handlebars](https://crates.io/crates/handlebars). Missing variables trigger a callback to your application for interactive prompts.

- **Lifecycle Management**
  Automatically starts an MCP client when a service appears and terminates it when the service disappears.

- **Client Notifications**
  Sends events (`McpStarted`, `McpStopped`, `InputRequired`) back to your application via an async channel.

---

## Getting Started

1. Clone the repository:
   ```
   git clone https://github.com/your-org/zeromcp.git
   cd zeromcp
   ```

2. Prepare a TOML configuration file (see [Configuration](#configuration)).

3. Build and run the example CLI:
   ```
   cargo run --example cli -- examples/configs/ha_proxy.toml
   ```

---

## Installation

Add ZeroMCP to your `Cargo.toml`:

```toml
[dependencies]
zeromcp = { git = "https://github.com/querymt/zeromcp.git", branch = "main" }
```

---

## Configuration

ZeroMCP uses a TOML file to define one or more service mappings. Each entry must specify:

- `zeroconf_service`: the mDNS service type to browse (e.g. `_home-assistant._tcp.local.`).
- An `mcp` configuration with a `protocol` tag (`"stdio"` or `"sse"`).

### Example: stdio

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
API_ACCESS_TOKEN = "{{API_TOKEN}}"
```

### Example: SSE

```toml
[[service_mapping]]
zeroconf_service = "_custom-service._tcp.local."

[service_mapping.mcp]
protocol = "sse"
name     = "Custom MCP"
url      = "https://{{service.hostname}}:{{service.port}}/stream?token={{TOKEN}}"
```

#### Templating

- Use `{{service.<field>}}` to access discovered service properties:
  - `fullname`, `hostname`, `port`, `addresses`
- Use custom placeholders like `{{API_TOKEN}}` to inject secrets. If missing, the library emits an `InputRequired` notification for your application to supply the value at runtime.

---

## Example CLI Usage

The `examples/cli.rs` demonstrates how to integrate ZeroMCP in a simple Tokio-based application:

1. **Load Configuration**
   ```rust
   let config = ZeroConfig::load("ha_proxy.toml")?;
   ```

2. **Create Notification Channel**
   ```rust
   let (tx, mut rx) = mpsc::channel(10);
   ```

3. **Initialize ServiceManager**
   ```rust
   let manager = ServiceManager::new(config, tx)?;
   tokio::spawn(async move {
     manager.run().await.unwrap();
   });
   ```

4. **Handle Notifications**
   ```rust
   while let Some(note) = rx.recv().await {
     match note {
       ClientNotification::McpStarted { service_name } => { /* ... */ }
       ClientNotification::McpStopped { service_name, reason } => { /* ... */ }
       ClientNotification::InputRequired { service_name, key, response_tx } => {
         // Prompt user and send back via response_tx.send(value)
       }
     }
   }
   ```

---

## Library API

```rust
// Load configuration from TOML
let config = zeromcp::ZeroConfig::load("config.toml")?;

// Create a channel for notifications
let (notification_tx, mut notification_rx) = tokio::sync::mpsc::channel(16);

// Initialize the ServiceManager
let manager = zeromcp::ServiceManager::new(config, notification_tx)?;

// Spawn the background task
tokio::spawn(async move {
    manager.run().await.expect("ServiceManager failed");
});

// Process notifications in your main loop
while let Some(notification) = notification_rx.recv().await {
    // Handle ClientNotification variants...
}
```

### Key Types

- `ZeroConfig`
  Parses your TOML into a Rust structure.

- `ServiceManager`
  Drives discovery, templating, process spawning, and cleanup.

- `ClientNotification`
  Enum of events you need to handle:
  - `McpStarted { service_name }`
  - `McpStopped { service_name, reason }`
  - `InputRequired { service_name, key, response_tx }`

---

## Contributing

Contributions are welcome! Feel free to open issues or submit pull requests.

1. Fork the repo
2. Create a feature branch
3. Write tests & update docs
4. Submit a PR

---
