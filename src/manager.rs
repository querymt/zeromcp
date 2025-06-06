use crate::config::{McpConfig, ZeroConfig};
use crate::models::{ClientNotification, DiscoveredService};
use crate::utils::hashmap_to_header_map;

use anyhow::{Context, Result};
use futures::stream::StreamExt;
use handlebars::{Handlebars, RenderErrorReason};
use mdns_sd::{ServiceDaemon, ServiceEvent};
use rmcp::service::DynService;
use rmcp::transport::sse_client::SseClientConfig;
use rmcp::{
    RoleClient, ServiceExt,
    service::RunningService,
    transport::{SseClientTransport, child_process::TokioChildProcess},
};
use serde_json::json;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};

/// The main struct for managing the service lifecycle.
pub struct ServiceManager {
    config: ZeroConfig,
    mdns: ServiceDaemon,
    notification_tx: mpsc::Sender<ClientNotification>,
    active_services:
        Arc<Mutex<HashMap<String, RunningService<RoleClient, Box<dyn DynService<RoleClient>>>>>>,
}

impl ServiceManager {
    /// Creates a new `ServiceManager`.
    pub fn new(
        config: ZeroConfig,
        notification_tx: mpsc::Sender<ClientNotification>,
    ) -> Result<Self> {
        Ok(Self {
            config,
            mdns: ServiceDaemon::new()?,
            notification_tx,
            active_services: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Runs the main event loop, continuously monitoring for services.
    /// This function runs forever until an error occurs.
    pub async fn run(&self) -> Result<()> {
        let mcp_map: HashMap<String, McpConfig> = self
            .config
            .service_mappings
            .iter()
            .map(|m| (m.zeroconf_service.clone(), m.mcp.clone()))
            .collect();

        // Browse for all unique service types from the config.
        let mut streams = Vec::new();
        for service_type in mcp_map.keys() {
            let receiver = self.mdns.browse(service_type)?;
            streams.push(receiver.into_stream());
            println!("[LIB] Browsing for '{}'...", service_type);
        }

        let mut merged_stream = futures::stream::select_all(streams);

        while let Some(service_event) = merged_stream.next().await {
            match service_event {
                // The `match` is now directly on the `ServiceEvent` enum.
                ServiceEvent::ServiceResolved(info) => {
                    println!("[LIB] Resolved: {}", info.get_fullname());
                    if let Some(mcp_config) = mcp_map.get(info.get_type()) {
                        self.handle_service_appeared(
                            DiscoveredService::from(&info),
                            mcp_config.clone(),
                        );
                    }
                }
                ServiceEvent::ServiceRemoved(fullname, _) => {
                    println!("[LIB] Removed: {}", fullname);
                    self.handle_service_disappeared(&fullname);
                }
                // Other event types like `ServiceFound` are ignored by the catch-all.
                _ => {}
            }
        }
        Ok(())
    }

    async fn process_service_config(
        cfg: &McpConfig,
        service: &DiscoveredService,
        notifier: &mpsc::Sender<ClientNotification>,
    ) -> Result<RunningService<RoleClient, Box<dyn DynService<RoleClient>>>> {
        async fn render_template(
            tpl: &str,
            ctx: &mut serde_json::Value,
            svc_name: &str,
            notifier: &mpsc::Sender<ClientNotification>,
        ) -> Option<String> {
            let mut reg = Handlebars::new();
            // FIX: Strict mode MUST be enabled to detect missing variables via errors.
            reg.set_strict_mode(true);

            loop {
                match reg.render_template(tpl, ctx) {
                    Ok(out) => {
                        return Some(out);
                    }

                    Err(e) => match &*e.reason() {
                        RenderErrorReason::MissingVariable(Some(var)) => {
                            let (tx, rx) = oneshot::channel();
                            let note = ClientNotification::InputRequired {
                                service_name: svc_name.to_string(),
                                key: var.clone(),
                                response_tx: tx,
                            };
                            if notifier.send(note).await.is_err() {
                                return None;
                            }
                            match rx.await {
                                Ok(val) => {
                                    if let Some(obj) = ctx.as_object_mut() {
                                        obj.insert(var.clone(), json!(val));
                                    }
                                }
                                Err(_) => return None,
                            }
                        }
                        _ => return None,
                    },
                }
            }
        }

        let mut ctx = json!({ "service": service });

        match cfg {
            McpConfig::Stdio {
                name,
                command,
                args,
                envs,
            } => {
                let mut final_args = Vec::with_capacity(args.len());
                for a in args {
                    let arg = render_template(a, &mut ctx, name, notifier)
                        .await
                        .context(format!("[LIB] Arg-template failed for arg: '{}'", a))?;
                    final_args.push(arg);
                }

                let mut child = tokio::process::Command::new(command);
                for (k, v_tpl) in envs {
                    let v = render_template(v_tpl, &mut ctx, name, notifier)
                        .await
                        .context(format!("[LIB] Env-template failed for env var: '{}'", k))?;
                    child.env(k, v);
                }

                child
                    .args(&final_args)
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit());

                let transport = TokioChildProcess::new(child)?;
                Ok(().into_dyn().serve(transport).await?)
            }
            McpConfig::Sse { name, url, headers } => {
                let url_str = render_template(&url, &mut ctx, &name, &notifier)
                    .await
                    .context("[LIB] SSE URL-template failed")?;

                let transport = match headers {
                    Some(hdr) => {
                        let mut rendered_map = HashMap::new();
                        for (k, v_tpl) in hdr.iter() {
                            let rendered_value: String =
                                render_template(&v_tpl, &mut ctx, &name, notifier)
                                    .await
                                    .context(format!(
                                        "[LIB] Env-template failed for env var: '{}'",
                                        k
                                    ))?;
                            rendered_map.insert(k.clone(), rendered_value);
                        }

                        let default_headers = hashmap_to_header_map(&rendered_map)?;
                        let client = reqwest::ClientBuilder::new()
                            .default_headers(default_headers)
                            .build()?;

                        SseClientTransport::start_with_client(
                            client,
                            SseClientConfig {
                                sse_endpoint: url_str.into(),
                                ..Default::default()
                            },
                        )
                        .await?
                    }
                    None => SseClientTransport::start(url_str).await?,
                };
                Ok(().into_dyn().serve(transport).await?)
            }
        }
    }

    /// Handles spawning and managing a process when a service is discovered.
    fn handle_service_appeared(&self, service: DiscoveredService, cfg: McpConfig) {
        let fullname = service.fullname.clone();
        {
            let map = self.active_services.lock().unwrap();
            if map.contains_key(&fullname) {
                println!("[LIB] '{}' already managed", fullname);
                return;
            }
        }

        let services = Arc::clone(&self.active_services);
        let notifier = self.notification_tx.clone();

        tokio::spawn(async move {
            let serve_fut = ServiceManager::process_service_config(&cfg, &service, &notifier);

            match serve_fut.await {
                Ok(service) => {
                    services.lock().unwrap().insert(fullname.clone(), service);
                    let _ = notifier
                        .send(ClientNotification::McpStarted {
                            service_name: fullname.clone(),
                        })
                        .await;
                }
                Err(e) => {
                    eprintln!("[LIB] Failed to start MCP for '{}': '{}'", fullname, e);
                }
            }
        });
    }

    /// Handles killing a managed process when its service disappears.
    fn handle_service_disappeared(&self, service_fullname: &str) {
        if let Some(service) = self
            .active_services
            .lock()
            .unwrap()
            .remove(service_fullname)
        {
            println!("[LIB] Service '{}' disappeared.", service_fullname);

            let notifier = self.notification_tx.clone();
            let service_name_owned = service_fullname.to_string();
            tokio::spawn(async move {
                match service.cancel().await {
                    Ok(_) => {
                        println!("[LIB] Process terminated successfully.");
                        let notifier = notifier;
                        let _ = notifier
                            .send(ClientNotification::McpStopped {
                                service_name: service_name_owned,
                                reason: "Zeroconf service disappeared".to_string(),
                            })
                            .await;
                    }
                    Err(e) => {
                        eprintln!(
                            "[LIB] Failed to terminate process for '{}': {}",
                            service_name_owned, e
                        );
                    }
                }
            });
        }
    }
}
