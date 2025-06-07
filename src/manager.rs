use crate::{
    ZeroHandler,
    client::ZeroClient,
    config::{McpConfig, ZeroConfig},
    mdns::MdnsBrowser,
    models::DiscoveredService,
    utils::hashmap_to_header_map,
};
use anyhow::{Context, Result, anyhow};
use futures::stream::StreamExt;
use handlebars::{Handlebars, RenderErrorReason};
use mdns_sd::{ServiceDaemon, ServiceEvent};
use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use rmcp::{
    RoleClient, ServiceExt,
    model::{GetPromptRequestParam, GetPromptResult, Prompt, Resource, ResourceTemplate, Tool},
    service::{DynService, QuitReason, RunningService},
    transport::{
        SseClientTransport, child_process::TokioChildProcess, sse_client::SseClientConfig,
    },
};
use serde_json::json;
use std::{collections::HashMap, fmt, process::Stdio, sync::Arc};
use tokio::task::JoinHandle;
use tracing::{Span, debug, error, info, instrument, warn};

pub enum ServiceMessage {
    AddService {
        name: String,
        service: McpClient,
    },
    CancelService {
        name: String,
        reply: RpcReplyPort<Result<QuitReason>>,
    },
    ListAllTools {
        service_name: String,
        reply: RpcReplyPort<Result<Vec<Tool>>>,
    },
    ListAllPrompts {
        service_name: String,
        reply: RpcReplyPort<Result<Vec<Prompt>>>,
    },
    ListAllResources {
        service_name: String,
        reply: RpcReplyPort<Result<Vec<Resource>>>,
    },
    ListAllResourceTemplates {
        service_name: String,
        reply: RpcReplyPort<Result<Vec<ResourceTemplate>>>,
    },
    GetPrompt {
        service_name: String,
        prompt_request: GetPromptRequestParam,
        reply: RpcReplyPort<Result<GetPromptResult>>,
    },
}

impl fmt::Debug for ServiceMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // For the variant with the non-Debug field:
            Self::AddService { name, .. } => f
                .debug_struct("AddService")
                .field("name", name)
                // We provide a placeholder string for the problematic field
                .field("service", &"<McpClient>")
                .finish(),

            // For variants where all fields are Debug, we can print them normally:
            Self::CancelService { name, reply } => f
                .debug_struct("CancelService")
                .field("name", name)
                .field("reply", reply)
                .finish(),

            Self::ListAllTools {
                service_name,
                reply,
            } => f
                .debug_struct("ListTools")
                .field("service_name", service_name)
                .field("reply", reply)
                .finish(),
            Self::ListAllPrompts {
                service_name,
                reply,
            } => f
                .debug_struct("ListAllPrompts")
                .field("service_name", service_name)
                .field("reply", reply)
                .finish(),
            Self::ListAllResources {
                service_name,
                reply,
            } => f
                .debug_struct("ListAllResources")
                .field("service_name", service_name)
                .field("reply", reply)
                .finish(),
            Self::ListAllResourceTemplates {
                service_name,
                reply,
            } => f
                .debug_struct("ListAllResourceTemplates")
                .field("service_name", service_name)
                .field("reply", reply)
                .finish(),
            Self::GetPrompt {
                service_name,
                prompt_request,
                reply,
            } => f
                .debug_struct("GetPrompt")
                .field("service_name", service_name)
                .field("prompt_request", prompt_request)
                .field("reply", reply)
                .finish(),
        }
    }
}

pub struct ActorState {
    active_services: HashMap<String, McpClient>,
}

pub struct ServiceActor;
pub type McpClient = RunningService<RoleClient, Box<dyn DynService<RoleClient>>>;

#[async_trait::async_trait]
impl Actor for ServiceActor {
    type Msg = ServiceMessage;
    type State = ActorState;
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(ActorState {
            active_services: HashMap::new(),
        })
    }

    #[instrument(name = "service_actor_handle", skip(self, _myself, state), fields(message_type = std::any::type_name::<ServiceMessage>()))]
    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            ServiceMessage::AddService { name, service } => {
                info!("Tracking new active service: {}", name);
                state.active_services.insert(name, service);
            }
            ServiceMessage::CancelService { name, reply } => {
                let result = if let Some(service) = state.active_services.remove(&name) {
                    service.cancel().await.map_err(|e| e.into())
                } else {
                    Err(anyhow!("Service '{}' not found for cancellation.", name))
                };
                if let Err(e) = &result {
                    warn!(
                        "Failed to cleanly cancel service '{}': {}",
                        name,
                        e.to_string()
                    );
                }
                let _ = reply.send(result);
            }
            ServiceMessage::ListAllTools {
                service_name,
                reply,
            } => {
                let result = if let Some(service) = state.active_services.get(&service_name) {
                    service.list_all_tools().await.map_err(|e| e.into())
                } else {
                    Err(anyhow!(
                        "Service '{}' not found to list tools.",
                        service_name
                    ))
                };
                let _ = reply.send(result);
            }
            ServiceMessage::ListAllPrompts {
                service_name,
                reply,
            } => {
                let result = if let Some(service) = state.active_services.get(&service_name) {
                    service.list_all_prompts().await.map_err(|e| e.into())
                } else {
                    Err(anyhow!(
                        "Service '{}' not found to list prompts.",
                        service_name
                    ))
                };
                let _ = reply.send(result);
            }
            ServiceMessage::ListAllResources {
                service_name,
                reply,
            } => {
                let result = if let Some(service) = state.active_services.get(&service_name) {
                    service.list_all_resources().await.map_err(|e| e.into())
                } else {
                    Err(anyhow!(
                        "Service '{}' not found to list resources.",
                        service_name
                    ))
                };
                let _ = reply.send(result);
            }
            ServiceMessage::ListAllResourceTemplates {
                service_name,
                reply,
            } => {
                let result = if let Some(service) = state.active_services.get(&service_name) {
                    service
                        .list_all_resource_templates()
                        .await
                        .map_err(|e| e.into())
                } else {
                    Err(anyhow!(
                        "Service '{}' not found to list resource templates.",
                        service_name
                    ))
                };
                let _ = reply.send(result);
            }
            ServiceMessage::GetPrompt {
                service_name,
                prompt_request,
                reply,
            } => {
                let result = if let Some(service) = state.active_services.get(&service_name) {
                    service
                        .get_prompt(prompt_request)
                        .await
                        .map_err(|e| e.into())
                } else {
                    Err(anyhow!(
                        "Service '{}' not found to get prompt '{:?}'.",
                        service_name,
                        prompt_request
                    ))
                };
                let _ = reply.send(result);
            }
        }
        Ok(())
    }
}

pub struct ServiceManager<M: MdnsBrowser> {
    actor: ActorRef<ServiceMessage>,
    config: ZeroConfig,
    mdns: M,
    app_handler: Arc<dyn ZeroHandler>,
}

impl<M: MdnsBrowser> fmt::Debug for ServiceManager<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceManager")
            .field("actor", &self.actor)
            .field("config", &self.config)
            .field("mdns", &"<ServiceDaemon>")
            .field("app_handler", &"<dyn ZeroHandler>")
            .finish()
    }
}

impl<M: MdnsBrowser + 'static> ServiceManager<M> {
    #[instrument(name = "service_manager_run", skip(self))]
    pub async fn run(&self) -> Result<()> {
        let mcp_map: HashMap<String, McpConfig> = self
            .config
            .service_mappings
            .iter()
            .map(|m| (m.zeroconf_service.clone(), m.mcp.clone()))
            .collect();

        let mut streams = Vec::new();
        for service_type in mcp_map.keys() {
            let receiver = self.mdns.browse(service_type)?;
            streams.push(receiver.into_stream());
            info!("Browsing for Zeroconf service type '{}'...", service_type);
        }

        let mut merged_stream = futures::stream::select_all(streams);
        info!("Service discovery started. Awaiting events.");

        while let Some(event) = merged_stream.next().await {
            match event {
                ServiceEvent::ServiceResolved(info) => {
                    let service_fullname = info.get_fullname().to_string();
                    let service_type = info.get_type().to_string();
                    let span = tracing::info_span!("service_resolved", service.fullname = %service_fullname, service.type = %service_type);
                    let _enter = span.enter();

                    info!("Resolved service");
                    if let Some(mcp_config) = mcp_map.get(info.get_type()) {
                        let service = DiscoveredService::from(&info);
                        self.handle_service_appeared(service, mcp_config.clone());
                    } else {
                        warn!("No mapping found in config for service type");
                    }
                }
                ServiceEvent::ServiceRemoved(service_name, reason) => {
                    let span =
                        tracing::info_span!("service_removed", service.fullname = %service_name);
                    let _enter = span.enter();

                    info!("Service '{}' removed {}", service_name, reason);
                    self.handle_service_disappeared(&service_name);
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Renders a Handlebars template, prompting for user input if variables are missing.
    #[instrument(name = "render_template", skip(ctx, app_handler), fields(service.name = %service_name, template = %tpl))]
    async fn render_template_with_input(
        tpl: &str,
        ctx: &mut serde_json::Value,
        service_name: &str,
        app_handler: &Arc<dyn ZeroHandler>,
    ) -> Result<String> {
        let mut reg = Handlebars::new();
        reg.set_strict_mode(true); // Ensures we fail on missing variables.

        loop {
            match reg.render_template(tpl, ctx) {
                Ok(rendered) => return Ok(rendered),
                Err(e) => match &*e.reason() {
                    RenderErrorReason::MissingVariable(Some(var)) => {
                        info!(variable = %var, "Template requires input");
                        let val = app_handler
                            .request_input(service_name, var)
                            .await
                            .with_context(|| {
                                format!("Failed to get user input for key '{}'", var)
                            })?;

                        if let Some(obj) = ctx.as_object_mut() {
                            obj.insert(var.clone(), json!(val));
                        }
                    }
                    _ => return Err(e).context("Failed to render Handlebars template"),
                },
            }
        }
    }

    /// Processes a discovered service's configuration to launch it.
    #[instrument(name = "process_service", skip(cfg, service, app_handler), fields(service.name = %service.fullname))]
    async fn process_service_config(
        cfg: &McpConfig,
        service: &DiscoveredService,
        app_handler: &Arc<dyn ZeroHandler>,
    ) -> Result<McpClient> {
        let mut ctx = json!({ "service": service });

        match cfg {
            McpConfig::Stdio {
                command,
                args,
                envs,
                ..
            } => {
                let mut final_args = Vec::with_capacity(args.len());
                for a_tpl in args {
                    let arg = Self::render_template_with_input(
                        a_tpl,
                        &mut ctx,
                        &service.fullname,
                        app_handler,
                    )
                    .await?;
                    final_args.push(arg);
                }

                let mut child_cmd = tokio::process::Command::new(command);
                for (k, v_tpl) in envs {
                    let v = Self::render_template_with_input(
                        v_tpl,
                        &mut ctx,
                        &service.fullname,
                        app_handler,
                    )
                    .await?;
                    child_cmd.env(k, v);
                }

                info!(command = %command, args = ?final_args, "Spawning stdio process");
                child_cmd
                    .args(&final_args)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                let transport = TokioChildProcess::new(child_cmd)?;
                Ok(().into_dyn().serve(transport).await?)
            }
            McpConfig::Sse { url, headers, .. } => {
                let url_str =
                    Self::render_template_with_input(url, &mut ctx, &service.fullname, app_handler)
                        .await?;
                let client_builder = reqwest::ClientBuilder::new();

                let client = if let Some(hdr) = headers {
                    let mut rendered_map = HashMap::new();
                    for (k, v_tpl) in hdr.iter() {
                        let v = Self::render_template_with_input(
                            v_tpl,
                            &mut ctx,
                            &service.fullname,
                            app_handler,
                        )
                        .await?;
                        rendered_map.insert(k.clone(), v);
                    }
                    let default_headers = hashmap_to_header_map(&rendered_map)?;
                    client_builder.default_headers(default_headers).build()?
                } else {
                    client_builder.build()?
                };

                info!(url = %url_str, "Starting SSE transport");
                let transport = SseClientTransport::start_with_client(
                    client,
                    SseClientConfig {
                        sse_endpoint: url_str.into(),
                        ..Default::default()
                    },
                )
                .await?;
                Ok(().into_dyn().serve(transport).await?)
            }
        }
    }

    fn handle_service_appeared(&self, service: DiscoveredService, cfg: McpConfig) {
        let actor_ref = self.actor.clone();
        let app_handler = self.app_handler.clone();

        tokio::spawn(async move {
            // Inherit the span from the parent task for better context in logs
            let span = Span::current();
            let _enter = span.enter();

            let service_fullname = service.fullname.clone();
            let process_fut = Self::process_service_config(&cfg, &service, &app_handler);

            match process_fut.await {
                Ok(mcp_client) => {
                    let msg = ServiceMessage::AddService {
                        name: service_fullname.clone(),
                        service: mcp_client,
                    };

                    if let Err(e) = actor_ref.cast(msg) {
                        error!(error = %e, "Failed to send AddService message to actor");
                    } else {
                        // Notify the user's application logic.
                        app_handler.on_service_started(&service).await;
                    }
                }
                Err(e) => {
                    error!(error = ?e, "Failed to start MCP for service");
                }
            }
        });
    }

    fn handle_service_disappeared(&self, service_fullname: &str) {
        let client = ZeroClient {
            actor: self.actor.clone(),
        };
        let name = service_fullname.to_string();
        let app_handler = self.app_handler.clone();

        tokio::spawn(async move {
            let span = Span::current();
            let _enter = span.enter();

            match client.stop_service(&name).await {
                Ok(reason) => {
                    info!(reason = ?reason, "Service stopped successfully");
                    app_handler.on_service_stopped(&name, reason).await;
                }
                Err(e) => {
                    debug!(error = %e, "Error stopping service (it may have already been removed)");
                }
            }
        });
    }
}

pub struct ZeroMcp {
    client: ZeroClient,
    // this handle will resolve when the manager finishes (signal or error)
    task: JoinHandle<anyhow::Result<()>>,
}

impl ZeroMcp {
    /// Returns the client you use to talk to running services.
    pub fn client(&self) -> &ZeroClient {
        &self.client
    }

    /// Signal the manager to shut down (if you build in a shutdown channel).
    pub async fn shutdown(self) -> anyhow::Result<()> {
        // e.g. drop client, send shutdown, await task.
        self.task.await?
    }
}

/// Start ZeroMCP, wiring your application logic into the background manager.
/// Returns `(client, manager_handle)`.
pub async fn start<H, F>(config: ZeroConfig, make_handler: F) -> Result<ZeroMcp>
where
    H: ZeroHandler + 'static,
    F: FnOnce(ZeroClient) -> Arc<H>,
{
    let mdns = ServiceDaemon::new()?;
    start_with_mdns(config, make_handler, mdns).await
}

/// Start ZeroMCP with a specific `MdnsBrowser` implementation.
///
/// This is the core startup logic, made generic for testability. The public `start`
/// function provides the real `mdns_sd::ServiceDaemon`.
pub(crate) async fn start_with_mdns<H, F, M>(
    config: ZeroConfig,
    make_handler: F,
    mdns: M,
) -> Result<ZeroMcp>
where
    H: ZeroHandler + 'static,
    F: FnOnce(ZeroClient) -> Arc<H>,
    M: MdnsBrowser + 'static,
{
    let (actor, _handle) = Actor::spawn(None, ServiceActor, ()).await?;

    let client = ZeroClient {
        actor: actor.clone(),
    };

    let handler = make_handler(client.clone());

    let manager = ServiceManager {
        actor,
        config,
        mdns,
        app_handler: handler,
    };

    let handle = tokio::spawn(async move { manager.run().await });

    Ok(ZeroMcp {
        client,
        task: handle,
    })
}
