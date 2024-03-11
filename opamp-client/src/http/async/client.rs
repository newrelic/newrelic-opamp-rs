use std::sync::{Arc, RwLock};

use super::http_client::AsyncHttpClient;
use crate::{
    common::{
        clientstate::ClientSyncedState, message_processor::ProcessResult, nextmessage::NextMessage,
    },
    opamp::proto::{AgentCapabilities, AgentToServer},
    operation::{callbacks::Callbacks, capabilities::Capabilities, settings::StartSettings},
    AsyncClient, {AsyncClientError, AsyncClientResult},
};
use async_trait::async_trait;

use tracing::{debug, trace};

use super::sender::HttpAsyncSender;

// An implementation of an OpAMP Client using HTTP transport with HttpClient.
pub struct OpAMPAsyncHttpClient<C, L>
where
    C: Callbacks + Send + Sync,
    L: AsyncHttpClient + Send + Sync,
{
    sender: HttpAsyncSender<L>,
    callbacks: C,
    message: Arc<RwLock<NextMessage>>,
    synced_state: ClientSyncedState,
    capabilities: Capabilities,
}

impl<C, L> OpAMPAsyncHttpClient<C, L>
where
    C: Callbacks + Send + Sync,
    L: AsyncHttpClient + Send + Sync,
{
    // Initializes a new OpAMPAsyncHttpClient with the provided Callbacks, uid, Capabilities, and HttpClient.
    pub(super) fn new(
        callbacks: C,
        start_settings: StartSettings,
        http_client: L,
    ) -> AsyncClientResult<Self> {
        Ok(Self {
            sender: HttpAsyncSender::new(http_client)?,
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
    pub(super) async fn poll(&self) -> AsyncClientResult<()> {
        self.send_process().await
    }
}

impl<C, L> OpAMPAsyncHttpClient<C, L>
where
    C: Callbacks + Send + Sync,
    L: AsyncHttpClient + Send + Sync,
{
    // An async method that handles the process of sending an AgentToServer message,
    // receives a ServerToAgent message, and analyzes the resulting message to decide
    // whether to resend or remain synced.
    #[async_recursion::async_recursion]
    async fn send_process(&self) -> AsyncClientResult<()> {
        // send message
        let msg = self
            .message
            .write()
            .map_err(|_| AsyncClientError::PoisonError)?
            .pop();
        let server_to_agent = self.sender.send(msg).await.map_err(|e| {
            self.callbacks.on_connect_failed(e.into());
            AsyncClientError::ConnectFailedCallback
        })?;

        trace!("Received payload: {}", server_to_agent);

        let result = crate::common::message_processor::process_message(
            server_to_agent,
            &self.callbacks,
            &self.synced_state,
            &self.capabilities,
            self.message.clone(),
        )?;

        // check if resend is needed
        match result {
            ProcessResult::NeedsResend => Ok(self.send_process().await?),
            ProcessResult::Synced => Ok(()),
        }
    }
}

#[async_trait]
impl<C, L> AsyncClient for OpAMPAsyncHttpClient<C, L>
where
    C: Callbacks + Send + Sync,
    L: AsyncHttpClient + Send + Sync,
{
    /// set_agent_description sets the agent description of the Agent.
    async fn set_agent_description(
        &self,
        description: crate::opamp::proto::AgentDescription,
    ) -> AsyncClientResult<()> {
        self.synced_state
            .set_agent_description(description.clone())?;

        // update message
        self.message
            .write()
            .map_err(|_| AsyncClientError::PoisonError)?
            .update(|msg| {
                msg.agent_description = Some(description);
            });

        debug!("sending AgentToServer with provided description");

        self.send_process().await
    }

    /// set_health sets the health status of the Agent.
    async fn set_health(
        &self,
        health: crate::opamp::proto::ComponentHealth,
    ) -> AsyncClientResult<()> {
        self.synced_state.set_health(health.clone())?;

        // update message
        self.message
            .write()
            .map_err(|_| AsyncClientError::PoisonError)?
            .update(|msg| {
                msg.health = Some(health);
            });

        debug!("sending AgentToServer with provided health");

        self.send_process().await
    }

    // update_effective_config fetches the current local effective config using
    // get_effective_config callback and sends it to the Server.
    async fn update_effective_config(&self) -> AsyncClientResult<()> {
        if !self
            .capabilities
            .has_capability(AgentCapabilities::ReportsEffectiveConfig)
        {
            return Err(AsyncClientError::UnsetEffectConfigCapability);
        }

        let config = self
            .callbacks
            .get_effective_config()
            .map_err(|_| AsyncClientError::EffectiveConfigError)?;

        // update message
        self.message
            .write()
            .map_err(|_| AsyncClientError::PoisonError)?
            .update(|msg| {
                msg.effective_config = Some(config);
            });

        debug!("sending AgentToServer with fetched effective config");

        self.send_process().await
    }

    // set_remote_config_status sends the status of the remote config
    // that was previously received from the Server
    async fn set_remote_config_status(
        &self,
        status: crate::opamp::proto::RemoteConfigStatus,
    ) -> AsyncClientResult<()> {
        if !self
            .capabilities
            .has_capability(AgentCapabilities::ReportsRemoteConfig)
        {
            return Err(AsyncClientError::UnsetRemoteConfigStatusCapability);
        }

        let synced_remote_config_status = self.synced_state.remote_config_status()?;
        if synced_remote_config_status.eq(&status) {
            return Ok(());
        }
        self.synced_state.set_remote_config_status(status.clone())?;

        self.message
            .write()
            .map_err(|_| AsyncClientError::PoisonError)?
            .update(|msg| {
                msg.remote_config_status = Some(status);
            });

        debug!("sending AgentToServer with remote");
        return self.send_process().await;
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use http::StatusCode;

    use super::super::http_client::test::{
        reqwest_response_from_server_to_agent, MockHttpClientMockall, ResponseParts,
    };

    use crate::{
        opamp::proto::ServerToAgent,
        operation::{callbacks::test::MockCallbacksMockall, settings::StartSettings},
        AsyncClientError,
    };

    #[tokio::test]
    async fn unsuccessful_http_response() {
        let mut mock_client = MockHttpClientMockall::new();
        mock_client.should_post(reqwest_response_from_server_to_agent(
            &ServerToAgent::default(),
            ResponseParts {
                status: StatusCode::FORBIDDEN,
                ..Default::default()
            },
        ));
        let mut mock_callbacks = MockCallbacksMockall::new();
        mock_callbacks.should_on_connect_failed();

        let client =
            OpAMPAsyncHttpClient::new(mock_callbacks, StartSettings::default(), mock_client)
                .unwrap();

        assert!(matches!(
            client.send_process().await.unwrap_err(),
            AsyncClientError::ConnectFailedCallback
        ));
    }
}
