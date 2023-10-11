use std::sync::{Arc, RwLock};

use crate::{
    common::{
        clientstate::ClientSyncedState, message_processor::ProcessResult, nextmessage::NextMessage,
    },
    error::{ClientError, ClientResult},
    opamp::proto::{AgentCapabilities, AgentToServer},
    operation::{callbacks::Callbacks, capabilities::Capabilities, settings::StartSettings},
    Client,
};

// An implementation of an OpAMP Client using HTTP transport with HttpClient.
pub struct OpAMPHttpClient<C, L>
where
    C: Callbacks + Send + Sync,
    L: HttpClient + Send + Sync,
{
    sender: HttpSender<L>,
    callbacks: C,
    message: Arc<RwLock<NextMessage>>,
    synced_state: ClientSyncedState,
    capabilities: Capabilities,
}

impl<C, L> OpAMPHttpClient<C, L>
where
    C: Callbacks + Send + Sync,
    L: HttpClient + Send + Sync,
{
    // Initializes a new OpAMPHttpClient with the provided Callbacks, uid, Capabilities, and HttpClient.
    pub(super) fn new(
        callbacks: C,
        start_settings: StartSettings,
        http_client: L,
    ) -> ClientResult<Self> {
        Ok(Self {
            sender: HttpSender::new(http_client)?,
            callbacks,
            message: Arc::new(RwLock::new(NextMessage::new(AgentToServer {
                instance_uid: start_settings.instance_id,
                agent_description: Some(start_settings.agent_description.into()),
                capabilities: u64::from(start_settings.capabilities),
                ..Default::default()
            }))),
            synced_state: ClientSyncedState::default(),
            capabilities: start_settings.capabilities,
        })
    }

    // Prompt the client to poll for updates and process messages.
    pub(super) async fn poll(&self) -> ClientResult<()> {
        self.send_process().await
    }
}

impl<C, L> OpAMPHttpClient<C, L>
where
    C: Callbacks + Send + Sync,
    L: HttpClient + Send + Sync,
{
    // An async method that handles the process of sending an AgentToServer message,
    // receives a ServerToAgent message, and analyzes the resulting message to decide
    // whether to resend or remain synced.
    #[async_recursion::async_recursion]
    async fn send_process(&self) -> ClientResult<()> {
        // send message
        let msg = self
            .message
            .write()
            .map_err(|_| ClientError::PoisonError)?
            .pop();
        // TODO: propagate unwrawp
        let server_to_agent = self.sender.send(msg).await?;

        let result = crate::common::message_processor::process_message(
            server_to_agent,
            &self.callbacks,
            &self.synced_state,
            &self.capabilities,
            self.message.clone(),
        )
        .await?;

        // check if resend is needed
        match result {
            ProcessResult::NeedsResend => Ok(self.send_process().await?),
            ProcessResult::Synced => Ok(()),
        }
    }
}

use crate::http::http_client::HttpClient;
use async_trait::async_trait;
use tracing::debug;

use super::sender::HttpSender;
#[async_trait]
impl<C, L> Client for OpAMPHttpClient<C, L>
where
    C: Callbacks + Send + Sync,
    L: HttpClient + Send + Sync,
{
    /// set_agent_description sets the agent description of the Agent.
    async fn set_agent_description(
        &self,
        description: crate::opamp::proto::AgentDescription,
    ) -> ClientResult<()> {
        self.synced_state
            .set_agent_description(description.clone())?;

        // update message
        self.message
            .write()
            .map_err(|_| ClientError::PoisonError)?
            .update(|msg| {
                msg.agent_description = Some(description);
            });

        debug!("sending AgentToServer with provided description");

        self.send_process().await
    }

    /// set_health sets the health status of the Agent.
    async fn set_health(&self, health: crate::opamp::proto::AgentHealth) -> ClientResult<()> {
        self.synced_state.set_health(health.clone())?;

        // update message
        self.message
            .write()
            .map_err(|_| ClientError::PoisonError)?
            .update(|msg| {
                msg.health = Some(health);
            });

        debug!("sending AgentToServer with provided health");

        self.send_process().await
    }

    // update_effective_config fetches the current local effective config using
    // get_effective_config callback and sends it to the Server.
    async fn update_effective_config(&self) -> ClientResult<()> {
        if !self
            .capabilities
            .has_capability(AgentCapabilities::ReportsEffectiveConfig)
        {
            return Err(ClientError::UnsetRemoteCapabilities);
        }

        let config = self
            .callbacks
            .get_effective_config()
            .map_err(|_| ClientError::EffectiveConfigError)?;

        // update message
        self.message
            .write()
            .map_err(|_| ClientError::PoisonError)?
            .update(|msg| {
                msg.effective_config = Some(config);
            });

        debug!("sending AgentToServer with fetched effective config");

        self.send_process().await
    }
}
