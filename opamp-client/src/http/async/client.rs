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

        // We consider it connected if we receive 2XX status from the Server.
        self.callbacks.on_connect();

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

        self.message
            .write()
            .map_err(|_| AsyncClientError::PoisonError)?
            .update(|msg| {
                msg.remote_config_status = Some(status.clone());
            });

        debug!("sending AgentToServer with remote");
        let result = self.send_process().await;

        if result.is_ok() {
            self.synced_state.set_remote_config_status(status)?;
        }

        result
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use http::StatusCode;
    use std::collections::HashMap;

    use super::super::http_client::test::{
        reqwest_response_from_server_to_agent, MockHttpClientMockall, ResponseParts,
    };

    use crate::opamp::proto::any_value::Value;
    use crate::opamp::proto::AgentDescription;
    use crate::opamp::proto::{
        AgentConfigFile, AgentConfigMap, AnyValue, ComponentHealth, EffectiveConfig, KeyValue,
        RemoteConfigStatus,
    };
    use crate::{
        capabilities,
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

    #[tokio::test]
    async fn reset_message_fields_after_send() {
        let mut mock_client = MockHttpClientMockall::new();
        mock_client.expect_post().times(4).returning(|_| {
            Ok(reqwest_response_from_server_to_agent(
                &ServerToAgent::default(),
                ResponseParts::default(),
            ))
        });
        let mut mock_callbacks = MockCallbacksMockall::new();
        mock_callbacks.expect_on_connect().times(4).return_const(());
        mock_callbacks.expect_on_message().times(4).return_const(());
        mock_callbacks
            .expect_get_effective_config()
            .once()
            .returning(|| {
                Ok(EffectiveConfig {
                    config_map: Some(AgentConfigMap {
                        config_map: HashMap::from([(
                            "/test".to_string(),
                            AgentConfigFile::default(),
                        )]),
                    }),
                })
            });

        let settings = StartSettings {
            instance_id: "NOT_AN_UID".to_string(),
            capabilities: capabilities!(
                AgentCapabilities::ReportsEffectiveConfig,
                AgentCapabilities::ReportsHealth,
                AgentCapabilities::ReportsRemoteConfig
            ),
            ..Default::default()
        };

        let client = OpAMPAsyncHttpClient::new(mock_callbacks, settings, mock_client).unwrap();

        let random_value = KeyValue {
            key: "thing".to_string(),
            value: Some(AnyValue {
                value: Some(Value::StringValue("thing_value".to_string())),
            }),
        };

        let result = client
            .set_agent_description(AgentDescription {
                identifying_attributes: vec![random_value.clone()],
                non_identifying_attributes: vec![random_value],
            })
            .await;
        assert!(result.is_ok());

        let result = client.update_effective_config().await;
        assert!(result.is_ok());

        let result = client
            .set_health(ComponentHealth {
                healthy: false,
                start_time_unix_nano: 1689942447,
                last_error: "wow! what an error".to_string(),
                ..Default::default()
            })
            .await;
        assert!(result.is_ok());

        let result = client
            .set_remote_config_status(RemoteConfigStatus {
                last_remote_config_hash: vec![],
                status: 2,
                error_message: "".to_string(),
            })
            .await;
        assert!(result.is_ok());

        // this pop should return a current message with the attributes reset
        let message = client.message.write().unwrap().pop();

        assert_eq!(
            message.agent_description,
            AgentToServer::default().agent_description
        );
        assert_eq!(message.health, AgentToServer::default().health);
        assert_eq!(
            message.remote_config_status,
            AgentToServer::default().remote_config_status
        );
        assert_eq!(
            message.effective_config,
            AgentToServer::default().effective_config
        );

        assert_ne!(
            client.synced_state.agent_description(),
            ClientSyncedState::default().agent_description()
        );
        assert_ne!(
            client.synced_state.health(),
            ClientSyncedState::default().health()
        );
        assert_ne!(
            client.synced_state.remote_config_status(),
            ClientSyncedState::default().remote_config_status()
        );
    }

    #[tokio::test]
    async fn remote_config_status_not_sync_state_update_on_error() {
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

        let settings = StartSettings {
            instance_id: "NOT_AN_UID".to_string(),
            capabilities: capabilities!(AgentCapabilities::ReportsRemoteConfig),
            ..Default::default()
        };

        let client = OpAMPAsyncHttpClient::new(mock_callbacks, settings, mock_client).unwrap();

        let remote_config_status = RemoteConfigStatus {
            last_remote_config_hash: vec![],
            status: 2,
            error_message: "".to_string(),
        };
        let result = client.set_remote_config_status(remote_config_status).await;

        assert!(matches!(
            result.unwrap_err(),
            AsyncClientError::ConnectFailedCallback
        ));

        assert_eq!(
            client.synced_state.remote_config_status(),
            ClientSyncedState::default().remote_config_status()
        );
    }
}
