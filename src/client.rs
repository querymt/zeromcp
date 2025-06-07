use crate::manager::ServiceMessage;

use anyhow::{Result, anyhow};
use ractor::{ActorRef, RpcReplyPort, rpc::CallResult};
use rmcp::{
    model::{GetPromptRequestParam, GetPromptResult, Prompt, Resource, ResourceTemplate, Tool},
    service::QuitReason,
};
use std::fmt::Debug;

/// The main client for interacting with discovered MCP services.
///
/// This client provides a high-level, asynchronous API for performing
/// operations on services managed by the `zeromcp` system.
#[derive(Clone, Debug)]
pub struct ZeroClient {
    pub(crate) actor: ActorRef<ServiceMessage>,
}

impl ZeroClient {
    async fn call_actor<TRequest, TResponse>(
        &self,
        msg_builder: impl FnOnce(RpcReplyPort<Result<TResponse>>) -> TRequest,
    ) -> Result<TResponse>
    where
        TRequest: Send,
        TResponse: Send + 'static + Debug,
        ServiceMessage: From<TRequest>,
    {
        let rpc_result = self
            .actor
            .call(
                |reply_port| {
                    let request: TRequest = msg_builder(reply_port);
                    ServiceMessage::from(request)
                },
                None,
            )
            .await;

        match rpc_result {
            Ok(app_level_result) => match app_level_result {
                CallResult::Success(r) => r,
                other => Err(anyhow!(
                    "Actor returned non-success call result: {:?}",
                    other
                )),
            },
            Err(e) => Err(anyhow!("Actor RPC call failed: {}", e)),
        }
    }

    /// Lists all available tools for a given service.
    ///
    /// # Arguments
    ///
    /// * `service_name` - The full name of the service (e.g., "MyTool._mcp._tcp.local.").
    pub async fn list_all_tools(&self, service_name: impl Into<String>) -> Result<Vec<Tool>> {
        self.call_actor(|reply| ServiceMessage::ListAllTools {
            service_name: service_name.into(),
            reply,
        })
        .await
    }

    /// Lists all available prompts for a given service.
    ///
    /// # Arguments
    ///
    /// * `service_name` - The full name of the service (e.g., "MyService._mcp._tcp.local.").
    pub async fn list_all_prompts(&self, service_name: impl Into<String>) -> Result<Vec<Prompt>> {
        self.call_actor(|reply| ServiceMessage::ListAllPrompts {
            service_name: service_name.into(),
            reply,
        })
        .await
    }

    /// Lists all available resources for a given service.
    ///
    /// # Arguments
    ///
    /// * `service_name` - The full name of the service (e.g., "MyService._mcp._tcp.local.").
    pub async fn list_all_resources(
        &self,
        service_name: impl Into<String>,
    ) -> Result<Vec<Resource>> {
        self.call_actor(|reply| ServiceMessage::ListAllResources {
            service_name: service_name.into(),
            reply,
        })
        .await
    }

    /// Lists all available resource templates for a given service.
    ///
    /// # Arguments
    ///
    /// * `service_name` - The full name of the service (e.g., "MyService._mcp._tcp.local.").
    pub async fn list_all_resource_templates(
        &self,
        service_name: impl Into<String>,
    ) -> Result<Vec<ResourceTemplate>> {
        self.call_actor(|reply| ServiceMessage::ListAllResourceTemplates {
            service_name: service_name.into(),
            reply,
        })
        .await
    }

    /// Gets a specific prompt by its ID from a given service.
    ///
    /// # Arguments
    ///
    /// * `service_name` - The full name of the service (e.g., "MyService._mcp._tcp.local.").
    /// * `prompt_id` - The ID of the prompt to retrieve.
    pub async fn get_prompt(
        &self,
        service_name: impl Into<String>,
        prompt_request: GetPromptRequestParam,
    ) -> Result<GetPromptResult> {
        self.call_actor(|reply| ServiceMessage::GetPrompt {
            service_name: service_name.into(),
            prompt_request,
            reply,
        })
        .await
    }

    /// Stops and removes a managed service.
    ///
    /// # Arguments
    ///
    /// * `service_name` - The full name of the service to stop.
    pub async fn stop_service(&self, service_name: impl Into<String>) -> Result<QuitReason> {
        self.call_actor(|reply| ServiceMessage::CancelService {
            name: service_name.into(),
            reply,
        })
        .await
    }
}
