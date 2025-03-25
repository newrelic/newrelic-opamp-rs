//! # Synchronous HTTP Client implementation for the OpAMP trait.

use std::sync::{Arc, RwLock};

use crate::{
    common::{
        clientstate::ClientSyncedState, message_processor::ProcessResult, nextmessage::NextMessage,
    },
    opamp::proto::{AgentCapabilities, AgentDisconnect, AgentToServer, CustomCapabilities},
    operation::{callbacks::Callbacks, capabilities::Capabilities, settings::StartSettings},
    Client, ClientError, ClientResult,
};

use super::{http_client::HttpClient, managed_client::Notifier, sender::HttpSender};
use tracing::{debug, error, info_span, trace, trace_span};

/// `UnManagedClient` is a trait for clients that do not manage their own polling.
pub trait UnManagedClient: Client {
    /// Executes a complete roundtrip of the opamp protocol.
    /// Sends a `AgentToServer` message, receives a `ServerToAgent` message, and processes it.
    fn poll(&self) -> ClientResult<()>;
}

/// An implementation of an `OpAMP` Synchronous Client using HTTP transport with `HttpClient`.
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
    pending_msg: Notifier,
    instance_uid: String,
}

/// `OpAMPHttpClient` synchronous HTTP implementation of the Client trait.
// If there is an error sending, the syncState should still be updated, we want it to be consistent
// with the agent status, and we leave the responsibility to the OpAMP server to call ReportFullState
// if it detects a gap on sequence numbers.
impl<C, L> OpAMPHttpClient<C, L>
where
    C: Callbacks + Send + Sync,
    L: HttpClient + Send + Sync,
{
    /// Initializes a new `OpAMPHttpClient` with the provided Callbacks, uid, Capabilities, and `HttpClient`.
    pub(super) fn new(
        callbacks: C,
        start_settings: StartSettings,
        http_client: L,
        pending_msg: Notifier,
    ) -> ClientResult<Self> {
        let synced_state = ClientSyncedState::default();
        if !start_settings.agent_description.is_empty() {
            synced_state.set_agent_description(start_settings.agent_description.clone().into())?;
        }
        if let Some(custom_capabilities) = start_settings.custom_capabilities {
            synced_state.set_custom_capabilities(custom_capabilities)?;
        }
        let instance_uid = start_settings.instance_uid.to_string();

        Ok(Self {
            sender: HttpSender::new(http_client, start_settings.instance_uid.clone()),
            callbacks,
            message: Arc::new(RwLock::new(NextMessage::new(AgentToServer {
                instance_uid: start_settings.instance_uid.into(),
                agent_description: Some(start_settings.agent_description.into()),
                capabilities: u64::from(start_settings.capabilities),
                ..Default::default()
            }))),
            synced_state,
            capabilities: start_settings.capabilities,
            pending_msg,
            instance_uid,
        })
    }
}

impl<C, L> UnManagedClient for OpAMPHttpClient<C, L>
where
    C: Callbacks + Send + Sync,
    L: HttpClient + Send + Sync,
{
    fn poll(&self) -> ClientResult<()> {
        self.send_process()
    }
}

impl<C, L> OpAMPHttpClient<C, L>
where
    C: Callbacks + Send + Sync,
    L: HttpClient + Send + Sync,
{
    // A sync method that handles the process of sending an AgentToServer message,
    // receives a ServerToAgent message, and analyzes the resulting message to decide
    // whether to resend or remain synced.
    fn send_process(&self) -> ClientResult<()> {
        // send message
        let msg = self
            .message
            .write()
            .map_err(|_| ClientError::PoisonError)?
            .pop();
        trace!("Send payload: {:?}", msg);
        let server_to_agent = self.sender.send(msg).map_err(|e| {
            let err_msg = e.to_string();
            self.callbacks.on_connect_failed(e.into());
            ClientError::ConnectFailedCallback(err_msg)
        })?;

        // We consider it connected if we receive 2XX status from the Server.
        self.callbacks.on_connect();

        trace!("Received payload: {:?}", server_to_agent);

        let _span = info_span!("process_message").entered();
        if let ProcessResult::NeedsResend = crate::common::message_processor::process_message(
            server_to_agent,
            &self.callbacks,
            &self.synced_state,
            self.capabilities,
            self.message.clone(),
        )? {
            self.pending_msg.notify_or_warn();
        }

        Ok(())
    }
}

impl<C, L> Drop for OpAMPHttpClient<C, L>
where
    C: Callbacks + Send + Sync,
    L: HttpClient + Send + Sync,
{
    fn drop(&mut self) {
        // By OpAMP protocol, AgentDisconnect must be sent in the last message.
        match self.message.write() {
            Ok(mut next_message) => {
                let mut msg = next_message.pop();
                msg.agent_disconnect = Some(AgentDisconnect::default());

                let _ = self.sender.send(msg).inspect_err(|err| {
                    error!(%err, instance_id=self.instance_uid, "sending disconnect OpAMP message");
                });

                debug!(
                    instance_uid = self.instance_uid,
                    "OpAMPHttpClient disconnected from server"
                );
            }
            Err(err) => {
                error!(%err, instance_id=self.instance_uid, "assembling disconnect OpAMP message");
            }
        };
    }
}

impl<C, L> Client for OpAMPHttpClient<C, L>
where
    C: Callbacks + Send + Sync,
    L: HttpClient + Send + Sync,
{
    /// `set_agent_description` sets the agent description of the Agent.
    // It uses compression and will only modify the message if there is a change.
    fn set_agent_description(
        &self,
        description: crate::opamp::proto::AgentDescription,
    ) -> ClientResult<()> {
        if self
            .synced_state
            .agent_description_unchanged(&description)?
        {
            return Ok(());
        }
        self.synced_state
            .set_agent_description(description.clone())?;

        // update message
        self.message
            .write()
            .map_err(|_| ClientError::PoisonError)?
            .update(|msg| {
                msg.agent_description = Some(description);
            });

        debug!(
            instance_uid = self.instance_uid,
            "sending AgentToServer with provided description"
        );
        self.pending_msg.notify_or_warn();
        Ok(())
    }
    /// `get_agent_description` returns the agent description from the synced state.
    fn get_agent_description(&self) -> ClientResult<crate::opamp::proto::AgentDescription> {
        match self.synced_state.agent_description() {
            Ok(Some(description)) => Ok(description),
            Err(e) => Err(e.into()),
            _ => Ok(crate::opamp::proto::AgentDescription::default()),
        }
    }

    /// `set_health` sets the health status of the Agent.
    // It uses compression and will only modify the message if there is a change.
    fn set_health(&self, health: crate::opamp::proto::ComponentHealth) -> ClientResult<()> {
        if !self
            .capabilities
            .has_capability(AgentCapabilities::ReportsHealth)
        {
            return Err(ClientError::UnsetHealthCapability);
        }

        if self.synced_state.health_unchanged(&health)? {
            return Ok(());
        }
        self.synced_state.set_health(health.clone())?;

        // update message
        self.message
            .write()
            .map_err(|_| ClientError::PoisonError)?
            .update(|msg| {
                msg.health = Some(health);
            });

        debug!(
            instance_uid = self.instance_uid,
            "sending AgentToServer with provided health"
        );
        self.pending_msg.notify_or_warn();
        Ok(())
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

        debug!(
            instance_uid = self.instance_uid,
            "sending AgentToServer with fetched effective config"
        );
        self.pending_msg.notify_or_warn();
        Ok(())
    }

    // set_remote_config_status sends the status of the remote config
    // that was previously received from the Server
    // It uses compression and will only modify the message if there is a change.
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

        if self.synced_state.remote_config_status_unchanged(&status)? {
            return Ok(());
        }
        self.synced_state.set_remote_config_status(status.clone())?;

        self.message
            .write()
            .map_err(|_| ClientError::PoisonError)?
            .update(|msg| {
                msg.remote_config_status = Some(status);
            });

        debug!(
            instance_uid = self.instance_uid,
            "sending AgentToServer with remote"
        );
        self.pending_msg.notify_or_warn();
        Ok(())
    }

    fn set_custom_capabilities(&self, custom_capabilities: CustomCapabilities) -> ClientResult<()> {
        if self
            .synced_state
            .custom_capabilities_unchanged(&custom_capabilities)?
        {
            return Ok(());
        }
        self.synced_state
            .set_custom_capabilities(custom_capabilities.clone())?;

        self.message
            .write()
            .map_err(|_| ClientError::PoisonError)?
            .update(|msg| {
                msg.custom_capabilities = Some(custom_capabilities);
            });

        debug!(
            instance_uid = self.instance_uid,
            "sending AgentToServer with custom capabilities"
        );
        self.pending_msg.notify_or_warn();
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use http::StatusCode;
    use mockall::mock;
    use std::collections::HashMap;
    use tracing_test::traced_test;

    use super::super::http_client::tests::{
        response_from_server_to_agent, MockHttpClientMockall, ResponseParts,
    };

    use crate::http::HttpClientError;
    use crate::opamp::proto::any_value::Value;
    use crate::opamp::proto::AgentDescription;
    use crate::opamp::proto::{
        AgentConfigFile, AgentConfigMap, AnyValue, ComponentHealth, EffectiveConfig, KeyValue,
        RemoteConfigStatus,
    };
    use crate::operation::settings::DescriptionValueType;
    use crate::{
        capabilities,
        opamp::proto::ServerToAgent,
        operation::{callbacks::tests::MockCallbacksMockall, settings::StartSettings},
    };

    mock! {
      #[derive(Debug)]
      pub(crate) UnmanagedClientMockall {}

        impl UnManagedClient for UnmanagedClientMockall {
            fn poll(&self) -> ClientResult<()>;
        }
        impl Client for UnmanagedClientMockall {
            fn set_agent_description(&self, description: AgentDescription) -> ClientResult<()>;
            fn get_agent_description(&self) -> ClientResult<AgentDescription>;
            fn set_health(&self, health: proto::proto::ComponentHealth) -> ClientResult<()>;
            fn update_effective_config(&self) -> ClientResult<()>;
            fn set_remote_config_status(&self, status: RemoteConfigStatus) -> ClientResult<()>;
            fn set_custom_capabilities(&self, custom_capabilities: CustomCapabilities) -> ClientResult<()>;
        }
    }

    #[test]
    fn unsuccessful_http_response() {
        let mut mock_client = MockHttpClientMockall::new();
        mock_client.expect_post().times(2).returning(|_| {
            Ok(response_from_server_to_agent(
                &ServerToAgent::default(),
                ResponseParts {
                    status: StatusCode::FORBIDDEN,
                    ..Default::default()
                },
            ))
        });
        let mut mock_callbacks = MockCallbacksMockall::new();
        mock_callbacks.should_on_connect_failed();

        let (pending_msg, _) = Notifier::new("msg".to_string());
        let client = OpAMPHttpClient::new(
            mock_callbacks,
            StartSettings::default(),
            mock_client,
            pending_msg,
        )
        .unwrap();

        assert!(matches!(
            client.send_process().unwrap_err(),
            ClientError::ConnectFailedCallback(_)
        ));
    }

    //Check if the Agent description is into the synced state after deleting the rest of fields.
    #[test]
    fn synced_state_contains_description_on_creation() {
        let mut mock_client = MockHttpClientMockall::new();
        mock_client.expect_post().times(2).returning(|_| {
            Ok(response_from_server_to_agent(
                &ServerToAgent::default(),
                ResponseParts::default(),
            ))
        });
        let mut mock_callbacks = MockCallbacksMockall::new();
        mock_callbacks.expect_on_connect().times(1).return_const(());
        mock_callbacks.expect_on_message().times(1).return_const(());
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

        let mut description: HashMap<String, DescriptionValueType> = HashMap::new();
        description.insert(
            "thing".to_string(),
            DescriptionValueType::String("thing_value".to_string()),
        );

        let settings = StartSettings {
            capabilities: capabilities!(
                AgentCapabilities::ReportsEffectiveConfig,
                AgentCapabilities::ReportsHealth,
                AgentCapabilities::ReportsRemoteConfig
            ),
            custom_capabilities: None,
            agent_description: crate::operation::settings::AgentDescription {
                identifying_attributes: description.clone(),
                non_identifying_attributes: description.clone(),
            },
            ..Default::default()
        };
        let (pending_msg, _) = Notifier::new("msg".to_string());
        let client =
            OpAMPHttpClient::new(mock_callbacks, settings, mock_client, pending_msg).unwrap();

        let result = client.update_effective_config();
        assert!(result.is_ok());

        let result = client.set_health(ComponentHealth {
            healthy: false,
            start_time_unix_nano: 1689942447,
            last_error: "wow! what an error".to_string(),
            ..Default::default()
        });
        assert!(result.is_ok());

        let result = client.set_remote_config_status(RemoteConfigStatus {
            last_remote_config_hash: vec![],
            status: 2,
            error_message: String::new(),
        });
        assert!(result.is_ok());

        client.poll().unwrap();

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
            client.synced_state.health().unwrap(),
            ClientSyncedState::default().health().unwrap()
        );
        assert_ne!(
            client.synced_state.remote_config_status().unwrap(),
            ClientSyncedState::default().remote_config_status().unwrap()
        );

        let random_value = KeyValue {
            key: "thing".to_string(),
            value: Some(AnyValue {
                value: Some(Value::StringValue("thing_value".to_string())),
            }),
        };

        let expected_result = AgentDescription {
            identifying_attributes: vec![random_value.clone()],
            non_identifying_attributes: vec![random_value],
        };

        assert_eq!(
            client
                .synced_state
                .agent_description()
                .unwrap()
                .unwrap()
                .identifying_attributes
                .first()
                .unwrap()
                .value,
            expected_result
                .identifying_attributes
                .first()
                .unwrap()
                .value
        );

        assert_eq!(
            client
                .synced_state
                .agent_description()
                .unwrap()
                .unwrap()
                .identifying_attributes
                .first()
                .unwrap()
                .key,
            expected_result.identifying_attributes.first().unwrap().key
        );

        assert_eq!(
            client
                .synced_state
                .agent_description()
                .unwrap()
                .unwrap()
                .non_identifying_attributes
                .first()
                .unwrap()
                .value,
            expected_result
                .non_identifying_attributes
                .first()
                .unwrap()
                .value
        );

        assert_eq!(
            client
                .synced_state
                .agent_description()
                .unwrap()
                .unwrap()
                .non_identifying_attributes
                .first()
                .unwrap()
                .key,
            expected_result
                .non_identifying_attributes
                .first()
                .unwrap()
                .key
        );
    }

    #[test]
    fn reset_message_fields_after_send() {
        let mut mock_client = MockHttpClientMockall::new();
        mock_client.expect_post().times(2).returning(|_| {
            Ok(response_from_server_to_agent(
                &ServerToAgent::default(),
                ResponseParts::default(),
            ))
        });
        let mut mock_callbacks = MockCallbacksMockall::new();
        mock_callbacks.expect_on_connect().times(1).return_const(());
        mock_callbacks.expect_on_message().times(1).return_const(());
        mock_callbacks
            .expect_get_effective_config()
            .once()
            .returning(|| Ok(EffectiveConfig::default()));

        let settings = StartSettings {
            capabilities: capabilities!(
                AgentCapabilities::ReportsEffectiveConfig,
                AgentCapabilities::ReportsHealth,
                AgentCapabilities::ReportsRemoteConfig
            ),
            ..Default::default()
        };

        let (pending_msg, has_pending_msg) = Notifier::new("msg".to_string());
        let client =
            OpAMPHttpClient::new(mock_callbacks, settings, mock_client, pending_msg).unwrap();

        let random_value = KeyValue {
            key: "thing".to_string(),
            value: Some(AnyValue {
                value: Some(Value::StringValue("thing_value".to_string())),
            }),
        };
        let agent_description = AgentDescription {
            identifying_attributes: vec![random_value.clone()],
            non_identifying_attributes: vec![random_value],
        };
        client
            .set_agent_description(agent_description.clone())
            .unwrap();
        has_pending_msg.try_recv().unwrap();

        client.update_effective_config().unwrap();

        let health = ComponentHealth {
            last_error: "wow! what an error".to_string(),
            ..Default::default()
        };
        client.set_health(health.clone()).unwrap();
        has_pending_msg.try_recv().unwrap();

        let remote_config_status = RemoteConfigStatus {
            error_message: "fake".to_string(),
            ..Default::default()
        };
        client
            .set_remote_config_status(remote_config_status.clone())
            .unwrap();
        has_pending_msg.try_recv().unwrap();

        let custom_capabilities = CustomCapabilities {
            capabilities: vec!["foo_capability".to_string()],
        };
        client
            .set_custom_capabilities(custom_capabilities.clone())
            .unwrap();
        has_pending_msg.try_recv().unwrap();

        // This should send the message and "compress" the fields
        client.poll().unwrap();

        // this pop should return a current message with the attributes reset
        let message = client.message.write().unwrap().pop();

        // Asserts the next message is going to be sent is compressed
        let expected = AgentToServer::default();
        assert_eq!(message.agent_description, expected.agent_description);
        assert_eq!(message.health, expected.health);
        assert_eq!(message.remote_config_status, expected.remote_config_status);
        assert_eq!(message.effective_config, expected.effective_config);
        assert_eq!(message.custom_capabilities, expected.custom_capabilities);

        // Asserts the state contains the last set values
        assert_eq!(
            client.synced_state.agent_description().unwrap(),
            Some(agent_description)
        );
        assert_eq!(client.synced_state.health().unwrap(), Some(health));
        assert_eq!(
            client.synced_state.remote_config_status().unwrap(),
            Some(remote_config_status)
        );
        assert_eq!(
            client.synced_state.custom_capabilities().unwrap(),
            Some(custom_capabilities)
        );
    }

    // If there is an error sending, the syncState should still be updated, we want it to be consistent
    // with the agent status, and we leave the responsibility to the OpAMP server to call ReportFullState
    // if it detects a gap on sequence numbers.
    #[test]
    fn remote_config_status_should_still_sync_state_update_on_error() {
        let mut mock_client = MockHttpClientMockall::new();
        mock_client.expect_post().times(2).returning(|_| {
            Ok(response_from_server_to_agent(
                &ServerToAgent::default(),
                ResponseParts {
                    status: StatusCode::FORBIDDEN,
                    ..Default::default()
                },
            ))
        });
        let mut mock_callbacks = MockCallbacksMockall::new();
        mock_callbacks.should_on_connect_failed();

        let settings = StartSettings {
            capabilities: capabilities!(AgentCapabilities::ReportsRemoteConfig),
            ..Default::default()
        };
        let (pending_msg, _r) = Notifier::new("msg".to_string());
        let client =
            OpAMPHttpClient::new(mock_callbacks, settings, mock_client, pending_msg).unwrap();

        let remote_config_status = RemoteConfigStatus {
            last_remote_config_hash: vec![],
            status: 2,
            error_message: String::new(),
        };
        client
            .set_remote_config_status(remote_config_status.clone())
            .unwrap();

        let result = client.poll();

        assert!(matches!(
            result.unwrap_err(),
            ClientError::ConnectFailedCallback(_)
        ));

        assert_eq!(
            client.synced_state.remote_config_status().unwrap().unwrap(),
            remote_config_status
        );
    }

    #[traced_test]
    #[test]
    fn test_drop_success() {
        let mut mock_client = MockHttpClientMockall::new();
        mock_client.expect_post().once().returning(|_| {
            Ok(response_from_server_to_agent(
                &ServerToAgent::default(),
                Default::default(),
            ))
        });

        let (pending_msg, _) = Notifier::new("msg".to_string());
        let start_settings = StartSettings::default();
        let instance_uid = start_settings.instance_uid.clone();
        let client = OpAMPHttpClient::new(
            MockCallbacksMockall::new(),
            start_settings,
            mock_client,
            pending_msg,
        )
        .unwrap();

        drop(client);

        assert!(logs_contain("OpAMPHttpClient disconnected from server"));
        assert!(logs_contain(instance_uid.to_string().as_str()));
    }
    #[traced_test]
    #[test]
    fn test_fail_to_drop() {
        let mut mock_client = MockHttpClientMockall::new();
        mock_client
            .expect_post()
            .once()
            .returning(|_| Err(HttpClientError::TransportError("some error".to_string())));

        let (pending_msg, _) = Notifier::new("msg".to_string());
        let client = OpAMPHttpClient::new(
            MockCallbacksMockall::new(),
            StartSettings::default(),
            mock_client,
            pending_msg,
        )
        .unwrap();

        drop(client);

        assert!(logs_contain("some error"));
    }
}
