//! # Synchronous HTTP Client implementation for the OpAMP trait.

use std::sync::{Arc, RwLock};

use crate::{
    common::{
        clientstate::ClientSyncedState, message_processor::ProcessResult, nextmessage::NextMessage,
    },
    opamp::proto::{AgentCapabilities, AgentToServer},
    operation::{callbacks::Callbacks, capabilities::Capabilities, settings::StartSettings},
    Client, ClientError, ClientResult,
};

use super::{http_client::HttpClient, sender::HttpSender};
use tracing::debug;

/// An implementation of an OpAMP Synchronous Client using HTTP transport with HttpClient.
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

/// OpAMPHttpClient synchronous HTTP implementation of the Client trait.
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
    pub(super) fn poll(&self) -> ClientResult<()> {
        self.send_process()
    }
}

impl<C, L> OpAMPHttpClient<C, L>
where
    C: Callbacks + Send + Sync,
    L: HttpClient + Send + Sync,
{
    // An sync method that handles the process of sending an AgentToServer message,
    // receives a ServerToAgent message, and analyzes the resulting message to decide
    // whether to resend or remain synced.
    fn send_process(&self) -> ClientResult<()> {
        // send message
        let msg = self
            .message
            .write()
            .map_err(|_| ClientError::PoisonError)?
            .pop();
        let server_to_agent = self.sender.send(msg).map_err(|e| {
            let err_msg = e.to_string();
            self.callbacks.on_connect_failed(e.into());
            ClientError::ConnectFailedCallback(err_msg)
        })?;

        tracing::trace!("Received payload: {}", server_to_agent);

        let result = crate::common::message_processor::process_message(
            server_to_agent,
            &self.callbacks,
            &self.synced_state,
            &self.capabilities,
            self.message.clone(),
        )?;

        // check if resend is needed
        match result {
            ProcessResult::NeedsResend => Ok(self.send_process()?),
            ProcessResult::Synced => Ok(()),
        }
    }
}

impl<C, L> Client for OpAMPHttpClient<C, L>
where
    C: Callbacks + Send + Sync,
    L: HttpClient + Send + Sync,
{
    /// set_agent_description sets the agent description of the Agent.
    fn set_agent_description(
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

        self.send_process()
    }

    /// set_health sets the health status of the Agent.
    fn set_health(&self, health: crate::opamp::proto::AgentHealth) -> ClientResult<()> {
        self.synced_state.set_health(health.clone())?;

        // update message
        self.message
            .write()
            .map_err(|_| ClientError::PoisonError)?
            .update(|msg| {
                msg.health = Some(health);
            });

        debug!("sending AgentToServer with provided health");

        self.send_process()
    }

    // update_effective_config fetches the current local effective config using
    // get_effective_config callback and sends it to the Server.
    fn update_effective_config(&self) -> ClientResult<()> {
        if !self
            .capabilities
            .has_capability(AgentCapabilities::ReportsEffectiveConfig)
        {
            return Err(ClientError::UnsetEffectConfigCapability);
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

        self.send_process()
    }

    // set_remote_config_status sends the status of the remote config
    // that was previously received from the Server
    fn set_remote_config_status(
        &self,
        status: crate::opamp::proto::RemoteConfigStatus,
    ) -> ClientResult<()> {
        if !self
            .capabilities
            .has_capability(AgentCapabilities::ReportsRemoteConfig)
        {
            return Err(ClientError::UnsetRemoteConfigStatusCapability);
        }

        let synced_remote_config_status = self.synced_state.remote_config_status()?;
        if synced_remote_config_status.eq(&status) {
            return Ok(());
        }
        self.synced_state.set_remote_config_status(status.clone())?;

        self.message
            .write()
            .map_err(|_| ClientError::PoisonError)?
            .update(|msg| {
                msg.remote_config_status = Some(status);
            });

        debug!("sending AgentToServer with remote");
        self.send_process()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use http::StatusCode;

    use super::super::http_client::test::{
        response_from_server_to_agent, MockHttpClientMockall, ResponseParts,
    };

    use crate::{
        opamp::proto::ServerToAgent,
        operation::{callbacks::test::MockCallbacksMockall, settings::StartSettings},
    };

    #[test]
    fn unsuccessful_http_response() {
        let mut mock_client = MockHttpClientMockall::new();
        mock_client.should_post(response_from_server_to_agent(
            &ServerToAgent::default(),
            ResponseParts {
                status: StatusCode::FORBIDDEN,
                ..Default::default()
            },
        ));
        let mut mock_callbacks = MockCallbacksMockall::new();
        mock_callbacks.should_on_connect_failed();

        let client =
            OpAMPHttpClient::new(mock_callbacks, StartSettings::default(), mock_client).unwrap();

        assert!(matches!(
            client.send_process().unwrap_err(),
            ClientError::ConnectFailedCallback(_)
        ));
    }
}
