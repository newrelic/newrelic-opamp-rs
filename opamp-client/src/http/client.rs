//! # Synchronous HTTP Client implementation for the OpAMP trait.

use std::sync::{Arc, RwLock};

use crate::{
    Client, ClientError, ClientResult,
    common::{
        clientstate::ClientSyncedState, message_processor::ProcessResult, nextmessage::NextMessage,
    },
    opamp::proto::{AgentCapabilities, AgentDisconnect, AgentToServer, CustomCapabilities},
    operation::{callbacks::Callbacks, capabilities::Capabilities, settings::StartSettings},
};

use super::{http_client::HttpClient, managed_client::Notifier, sender::HttpSender};
use tracing::{debug, error, info_span, trace};

/// A trait for clients that do not manage their own polling.
pub trait UnManagedClient: Client {
    /// Executes a complete roundtrip of the opamp protocol.
    /// Sends a [`AgentToServer`] message, receives a [`ServerToAgent`](crate::opamp::proto::ServerToAgent) message, and processes it.
    fn poll(&self) -> ClientResult<()>;
}

/// An implementation of an OpAMP Synchronous Client using HTTP transport with [`HttpClient`].
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

/// Synchronous HTTP implementation of the Client trait.
// If there is an error sending, the syncState should still be updated, we want it to be consistent
// with the agent status, and we leave the responsibility to the OpAMP server to call ReportFullState
// if it detects a gap on sequence numbers.
impl<C, L> OpAMPHttpClient<C, L>
where
    C: Callbacks + Send + Sync,
    L: HttpClient + Send + Sync,
{
    /// Initializes a new [`OpAMPHttpClient`] with the provided arguments.
    pub(super) fn new(
        callbacks: C,
        start_settings: StartSettings,
        http_client: L,
        pending_msg: Notifier,
    ) -> ClientResult<Self> {
        let capabilities = start_settings.capabilities;
        let instance_uid = start_settings.instance_uid.clone();

        let (initial_message, synced_state) = Self::initial_message_and_state(start_settings)?;

        Ok(Self {
            sender: HttpSender::new(http_client, instance_uid.clone()),
            callbacks,
            message: Arc::new(RwLock::new(NextMessage::new(initial_message))),
            synced_state,
            capabilities,
            pending_msg,
            instance_uid: instance_uid.to_string(),
        })
    }

    /// Helper to build the initial [AgentToServer] message to be sent to the server and the corresponding
    /// internal state to keep track of sent fields (check [ClientSyncedState] for details).
    fn initial_message_and_state(
        start_settings: StartSettings,
    ) -> ClientResult<(AgentToServer, ClientSyncedState)> {
        // Destructured to get compile errors if any field is added to StartSettings
        let StartSettings {
            instance_uid,
            capabilities,
            custom_capabilities,
            agent_description,
        } = start_settings;

        // Store initial state fields
        let initial_state = ClientSyncedState::default();
        if !agent_description.is_empty() {
            initial_state.set_agent_description(agent_description.clone().into())?;
        }
        if let Some(custom_capabilities) = custom_capabilities.as_ref() {
            initial_state.set_custom_capabilities(custom_capabilities.clone())?;
        }

        // build initial message
        let initial_message = AgentToServer {
            instance_uid: instance_uid.into(),
            agent_description: Some(agent_description.into()),
            capabilities: capabilities.into(),
            custom_capabilities,
            ..Default::default()
        };

        Ok((initial_message, initial_state))
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
    /// Sets the agent description of the Agent.
    ///
    /// It uses compression and will only modify the message if there is a change.
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
    /// Returns the agent description from the synced state.
    fn get_agent_description(&self) -> ClientResult<crate::opamp::proto::AgentDescription> {
        match self.synced_state.agent_description() {
            Ok(Some(description)) => Ok(description),
            Err(e) => Err(e.into()),
            _ => Ok(crate::opamp::proto::AgentDescription::default()),
        }
    }

    /// Sets the health status of the Agent.
    ///
    /// It uses compression and will only modify the message if there is a change.
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

    /// Fetches the current local effective config using
    /// [`get_effective_config`](Callbacks::get_effective_config) callback and sends it to the Server.
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

    /// Sends the status of the remote config
    /// that was previously received from the Server.
    ///
    /// It uses compression and will only modify the message if there is a change.
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
    use assert_matches::assert_matches;
    use http::StatusCode;
    use mockall::mock;
    use rstest::rstest;
    use std::collections::HashMap;
    use tracing_test::traced_test;

    use super::super::http_client::tests::{
        MockHttpClientMockall, ResponseParts, response_from_server_to_agent,
    };

    use crate::http::HttpClientError;
    use crate::opamp::proto::AgentDescription;
    use crate::opamp::proto::any_value::Value;
    use crate::opamp::proto::{
        AgentConfigFile, AgentConfigMap, AnyValue, ComponentHealth, EffectiveConfig, KeyValue,
        RemoteConfigStatus,
    };
    use crate::operation::instance_uid::InstanceUid;
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

    impl crate::operation::settings::AgentDescription {
        /// Returns a non-empty agent-description for testing purposes
        pub fn testing_non_empty() -> Self {
            Self {
                identifying_attributes: [("id.key".to_string(), DescriptionValueType::Int(42))]
                    .into(),
                non_identifying_attributes: [(
                    "non-id.key".to_string(),
                    DescriptionValueType::String("non-identifying".to_string()),
                )]
                .into(),
            }
        }
    }

    /// Concrete `OpAMPHttpClient` configuration used across parametrized tests.
    type TestClient = OpAMPHttpClient<MockCallbacksMockall, MockHttpClientMockall>;
    /// A boxed action that exercises one client setter against a configured client.
    type ClientAction = Box<dyn Fn(&TestClient) -> ClientResult<()>>;

    /// Builds a mock HTTP client whose only expected `post` call is the disconnect
    /// fired when the `OpAMPHttpClient` is dropped at the end of the test.
    fn drop_only_http_mock() -> MockHttpClientMockall {
        let mut mock_client = MockHttpClientMockall::new();
        mock_client.expect_post().once().returning(|_| {
            Ok(response_from_server_to_agent(
                &ServerToAgent::default(),
                ResponseParts::default(),
            ))
        });
        mock_client
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

        assert_matches!(
            client.send_process().unwrap_err(),
            ClientError::ConnectFailedCallback(_)
        );
    }

    #[test]
    fn initial_message_includes_all_start_settings_fields() {
        let instance_uid = InstanceUid::create();
        let custom_capabilities = CustomCapabilities {
            capabilities: vec!["custom.capability".into()],
        };
        let capabilities = capabilities!(AgentCapabilities::ReportsStatus);
        let agent_description = crate::operation::settings::AgentDescription::testing_non_empty();
        let start_settings = StartSettings {
            instance_uid: instance_uid.clone(),
            capabilities,
            custom_capabilities: Some(custom_capabilities.clone()),
            agent_description: agent_description.clone(),
        };

        let (pending_msg, _) = Notifier::new("name".to_string());
        let client = OpAMPHttpClient::new(
            MockCallbacksMockall::new(),
            start_settings,
            drop_only_http_mock(),
            pending_msg,
        )
        .expect("Client creation expects to succeed");

        // Check initial message
        let message = client.message.write().unwrap().pop();
        assert_eq!(
            message.custom_capabilities,
            Some(custom_capabilities.clone())
        );
        assert_eq!(
            message.agent_description,
            Some(agent_description.clone().into())
        );
        assert_eq!(message.capabilities, u64::from(capabilities));
        assert_eq!(message.instance_uid, Vec::<u8>::from(instance_uid));
        // Check state
        assert_eq!(
            client.synced_state.custom_capabilities().unwrap(),
            Some(custom_capabilities)
        );
        assert_eq!(
            client.synced_state.agent_description().unwrap(),
            Some(agent_description.into())
        );
    }

    // After a poll, message fields are compressed away but the synced state retains them.
    // Specifically validates that an agent description provided via StartSettings is persisted.
    #[test]
    fn poll_compresses_message_but_synced_state_persists() {
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

        let agent_description = crate::operation::settings::AgentDescription::testing_non_empty();
        let settings = StartSettings {
            capabilities: capabilities!(
                AgentCapabilities::ReportsEffectiveConfig,
                AgentCapabilities::ReportsHealth,
                AgentCapabilities::ReportsRemoteConfig
            ),
            agent_description: agent_description.clone(),
            ..Default::default()
        };
        let (pending_msg, _) = Notifier::new("msg".to_string());
        let client =
            OpAMPHttpClient::new(mock_callbacks, settings, mock_client, pending_msg).unwrap();

        let health = ComponentHealth {
            healthy: false,
            start_time_unix_nano: 1689942447,
            last_error: "wow! what an error".to_string(),
            ..Default::default()
        };
        let remote_config_status = RemoteConfigStatus {
            status: 2,
            ..Default::default()
        };

        client.update_effective_config().unwrap();
        client.set_health(health.clone()).unwrap();
        client
            .set_remote_config_status(remote_config_status.clone())
            .unwrap();

        client.poll().unwrap();

        // After poll, the next message has its fields compressed away.
        let message = client.message.write().unwrap().pop();
        assert_eq!(message.agent_description, None);
        assert_eq!(message.health, None);
        assert_eq!(message.remote_config_status, None);
        assert_eq!(message.effective_config, None);

        // Synced state retains the values.
        assert_eq!(
            client.synced_state.agent_description().unwrap(),
            Some(agent_description.into()),
        );
        assert_eq!(client.synced_state.health().unwrap(), Some(health));
        assert_eq!(
            client.synced_state.remote_config_status().unwrap(),
            Some(remote_config_status),
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

        assert_matches!(result.unwrap_err(), ClientError::ConnectFailedCallback(_));

        assert_eq!(
            client.synced_state.remote_config_status().unwrap().unwrap(),
            remote_config_status
        );
    }

    #[traced_test]
    #[test]
    fn test_drop_success() {
        let (pending_msg, _) = Notifier::new("msg".to_string());
        let start_settings = StartSettings::default();
        let instance_uid = start_settings.instance_uid.clone();
        let client = OpAMPHttpClient::new(
            MockCallbacksMockall::new(),
            start_settings,
            drop_only_http_mock(),
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

    #[rstest]
    #[case::health(
        Box::new(|c: &TestClient| c.set_health(ComponentHealth::default())) as ClientAction,
        Box::new(|e| assert_matches!(e, ClientError::UnsetHealthCapability)),
    )]
    #[case::effective_config(
        Box::new(|c: &TestClient| c.update_effective_config()) as ClientAction,
        Box::new(|e| assert_matches!(e, ClientError::UnsetEffectConfigCapability)),
    )]
    #[case::remote_config_status(
        Box::new(|c: &TestClient| c.set_remote_config_status(RemoteConfigStatus::default())) as ClientAction,
        Box::new(|e| assert_matches!(e, ClientError::UnsetRemoteConfigStatusCapability)),
    )]
    fn setter_without_capability_returns_error(
        #[case] action: ClientAction,
        #[case] assert_expected_error: Box<dyn Fn(ClientError)>,
    ) {
        let (pending_msg, _) = Notifier::new("msg".to_string());
        let client = OpAMPHttpClient::new(
            MockCallbacksMockall::new(),
            StartSettings::default(), // capabilities are empty by default
            drop_only_http_mock(),
            pending_msg,
        )
        .unwrap();

        assert_expected_error(action(&client).unwrap_err());
    }

    /// Checks that compression works as expected for each setter by setting an attribute twice.
    /// The first call triggers a message notification while the second, that doesn't change
    /// the value, doesn't.
    #[rstest]
    #[case::agent_description(
        StartSettings::default(),
        Box::new(|c: &TestClient| c.set_agent_description(AgentDescription {
            identifying_attributes: vec![KeyValue {
                key: "k".to_string(),
                value: Some(AnyValue {
                    value: Some(Value::StringValue("v".to_string())),
                }),
            }],
            ..Default::default()
        })) as ClientAction,
    )]
    #[case::health(
        StartSettings {
            capabilities: capabilities!(AgentCapabilities::ReportsHealth),
            ..Default::default()
        },
        Box::new(|c: &TestClient| c.set_health(ComponentHealth {
            healthy: true,
            ..Default::default()
        })) as ClientAction,
    )]
    #[case::remote_config_status(
        StartSettings {
            capabilities: capabilities!(AgentCapabilities::ReportsRemoteConfig),
            ..Default::default()
        },
        Box::new(|c: &TestClient| c.set_remote_config_status(RemoteConfigStatus {
            status: 2,
            ..Default::default()
        })) as ClientAction,
    )]
    #[case::custom_capabilities(
        StartSettings::default(),
        Box::new(|c: &TestClient| c.set_custom_capabilities(CustomCapabilities {
            capabilities: vec!["custom.cap".to_string()],
        })) as ClientAction,
    )]
    fn setter_with_unchanged_value_skips_resend_notification(
        #[case] settings: StartSettings,
        #[case] action: ClientAction,
    ) {
        let (pending_msg, has_pending_msg) = Notifier::new("msg".to_string());
        let client = OpAMPHttpClient::new(
            MockCallbacksMockall::new(),
            settings,
            drop_only_http_mock(),
            pending_msg,
        )
        .unwrap();

        // First call differs from the default synced state, so it must notify.
        action(&client).unwrap();
        has_pending_msg
            .try_recv()
            .expect("first call should notify");

        // Second call with the same value should hit the unchanged guard.
        action(&client).unwrap();
        assert!(
            has_pending_msg.try_recv().is_err(),
            "unchanged value should not notify",
        );
    }

    #[test]
    fn get_agent_description_returns_synced_value() {
        let agent_description = crate::operation::settings::AgentDescription::testing_non_empty();
        let settings = StartSettings {
            agent_description: agent_description.clone(),
            ..Default::default()
        };
        let (pending_msg, _) = Notifier::new("msg".to_string());
        let client = OpAMPHttpClient::new(
            MockCallbacksMockall::new(),
            settings,
            drop_only_http_mock(),
            pending_msg,
        )
        .unwrap();

        assert_eq!(
            client.get_agent_description().unwrap(),
            agent_description.into(),
        );
    }

    #[test]
    fn get_agent_description_default_when_unset() {
        let (pending_msg, _) = Notifier::new("msg".to_string());
        let client = OpAMPHttpClient::new(
            MockCallbacksMockall::new(),
            StartSettings::default(),
            drop_only_http_mock(),
            pending_msg,
        )
        .unwrap();

        assert_eq!(
            client.get_agent_description().unwrap(),
            AgentDescription::default(),
        );
    }

    #[test]
    fn update_effective_config_callback_error_maps_to_effective_config_error() {
        use crate::operation::callbacks::tests::CallbacksMockError;

        let mut mock_callbacks = MockCallbacksMockall::new();
        mock_callbacks
            .expect_get_effective_config()
            .once()
            .returning(|| Err(CallbacksMockError));

        let settings = StartSettings {
            capabilities: capabilities!(AgentCapabilities::ReportsEffectiveConfig),
            ..Default::default()
        };
        let (pending_msg, _) = Notifier::new("msg".to_string());
        let client =
            OpAMPHttpClient::new(mock_callbacks, settings, drop_only_http_mock(), pending_msg)
                .unwrap();

        assert_matches!(
            client.update_effective_config().unwrap_err(),
            ClientError::EffectiveConfigError
        );
    }
}
