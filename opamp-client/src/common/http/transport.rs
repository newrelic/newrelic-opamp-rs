use std::{
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use http::{
    header::{InvalidHeaderName, InvalidHeaderValue},
    HeaderMap, HeaderName, HeaderValue,
};
use reqwest::Client;
use thiserror::Error;
use tokio::{
    select,
    sync::mpsc::Receiver,
    time::{interval, Interval},
};
use tracing::{debug, error, info, warn};
use url::Url;

use AgentCapabilities::*;

use crate::{
    common::{
        clientstate::ClientSyncedState,
        http::compression::{decode_message, encode_message},
        nextmessage::NextMessage,
        transport::{TransportError, TransportRunner},
    },
    opamp::proto::{
        AgentToServer, ConnectionSettingsOffers, OtherConnectionSettings, ServerToAgent,
        ServerToAgentFlags, TelemetryConnectionSettings,
    },
    operation::{
        callbacks::{Callbacks, MessageData},
        capabilities::Capabilities,
    },
};
use crate::{opamp::proto::AgentCapabilities, operation::syncedstate::SyncedState};

use super::compression::{Compressor, CompressorError, DecoderError, EncoderError};

#[async_trait]
pub trait Transport {
    async fn send(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, TransportError>;
}

pub struct ReqwestSender;

#[async_trait]
impl Transport for ReqwestSender {
    async fn send(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, TransportError> {
        let res = request.send().await?;
        Ok(res)
    }
}

// opamp_headers returns a HeaderMap with the common HTTP headers used in an
// OpAMP connection
fn opamp_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();

    headers.insert(
        "Content-Type",
        HeaderValue::from_static("application/x-protobuf"),
    );

    headers
}

// HttpConfig wraps configuration parameters for the internal HTTP client
// gzip compression is enabled by default
pub(crate) struct HttpConfig {
    url: Url,
    headers: HeaderMap,
    compression: bool,
}

impl HttpConfig {
    pub(crate) fn new(url: Url) -> Self {
        Self {
            url,
            headers: opamp_headers(),
            compression: false,
        }
    }

    // with_headers allows to include custom headers into the http requests
    pub(crate) fn with_headers<'a, I>(mut self, headers: I) -> Result<Self, HttpError>
    where
        I: IntoIterator<Item = (&'a str, &'a str)>,
    {
        for (key, val) in headers {
            // do nothing if value already in internal headers map
            let _ = self
                .headers
                .insert(HeaderName::from_str(key)?, val.parse()?);
        }
        Ok(self)
    }

    // enables gzip compression
    pub(crate) fn with_gzip_compression(mut self) -> Self {
        self.headers
            .insert("Content-Encoding", HeaderValue::from_static("gzip"));
        self.headers
            .insert("Accept-Encoding", HeaderValue::from_static("gzip"));
        self.compression = true;
        self
    }
}

impl TryFrom<HttpConfig> for reqwest::Client {
    type Error = HttpError;
    fn try_from(value: HttpConfig) -> Result<Self, Self::Error> {
        Ok(Client::builder().default_headers(value.headers).build()?)
    }
}

impl From<&HttpConfig> for Compressor {
    fn from(value: &HttpConfig) -> Self {
        if value.compression {
            return Compressor::Gzip;
        }
        Compressor::Plain
    }
}

pub struct HttpTransport<C, T: Transport = ReqwestSender>
where
    C: Callbacks,
    T: Transport,
{
    pub(crate) pending_messages: Receiver<()>,
    http_client: reqwest::Client,
    url: url::Url,
    compressor: Compressor,
    sender: T,

    // callbacks function when a new message is received
    callbacks: C,

    // polling interval has passed. Force a status update.
    polling: Interval,
    // next message structure for forced status updates
    next_message: Arc<Mutex<NextMessage>>,
}

#[derive(Error, Debug)]
pub enum HttpError {
    #[error("`{0}`")]
    ReqwestError(#[from] reqwest::Error),
    #[error("`{0}`")]
    InvalidHeader(#[from] InvalidHeaderValue),
    #[error("`{0}`")]
    InvalidHeaderName(#[from] InvalidHeaderName),
    #[error("`{0}`")]
    Encode(#[from] EncoderError),
    #[error("`{0}`")]
    Decode(#[from] DecoderError),
    #[error("`{0}`")]
    Compress(#[from] CompressorError),
    #[error("`{0}`")]
    Transport(#[from] TransportError),
}

impl<C> HttpTransport<C, ReqwestSender>
where
    C: Callbacks,
{
    pub(crate) fn new(
        client_config: HttpConfig,
        polling: Duration,
        pending_messages: Receiver<()>,
        next_message: Arc<Mutex<NextMessage>>,
        callbacks: C,
    ) -> Result<Self, HttpError> {
        let url = client_config.url.clone();
        let compressor = Compressor::from(&client_config);
        Ok(HttpTransport {
            http_client: reqwest::Client::try_from(client_config)?,
            compressor,
            sender: ReqwestSender {},
            pending_messages,
            url,
            polling: interval(polling),
            next_message,
            callbacks,
        })
    }
}

#[derive(Debug, PartialEq)]
struct ScheduleMessage;

impl<C: Callbacks, T: Transport> HttpTransport<C, T> {
    #[cfg(test)]
    pub(crate) fn with_transport(
        client_config: HttpConfig,
        polling: Duration,
        transport: T,
        pending_messages: Receiver<()>,
        next_message: Arc<Mutex<NextMessage>>,
        callbacks: C,
    ) -> Result<HttpTransport<C, T>, HttpError> {
        let url = client_config.url.clone();
        let compressor = Compressor::from(&client_config);
        Ok(HttpTransport {
            http_client: reqwest::Client::try_from(client_config)?,
            compressor,
            sender: transport,
            pending_messages,
            url,
            polling: interval(polling),
            next_message,
            callbacks,
        })
    }

    async fn send_message(&self, msg: AgentToServer) -> Result<ServerToAgent, HttpError> {
        // Serialize the message to bytes
        let bytes = encode_message(&self.compressor, msg)?;

        let response = self
            .sender
            .send(
                self.http_client
                    .post(self.url.clone())
                    .body(reqwest::Body::from(bytes)),
            )
            .await?;

        let compression = match response.headers().get("Content-Encoding") {
            Some(algorithm) => Compressor::try_from(algorithm.as_ref())?,
            None => Compressor::Plain,
        };

        let response = decode_message::<ServerToAgent>(&compression, &response.bytes().await?)?;

        self.callbacks.on_connect();

        Ok(response)
    }

    // next_send returns the unit type if a new message should be send, none if notifying
    // channel has been closed. Asynchronous function which waits for the internal ticker
    // or a pushed notification in the internal pending_messages channel.
    async fn next_send(&mut self) -> Option<()> {
        select! {
            _ = self.polling.tick() => Some(()),
            recv_result = self.pending_messages.recv() => { self.polling.reset(); recv_result }
        }
    }

    async fn receive(
        &self,
        state: Arc<ClientSyncedState>,
        capabilities: Capabilities,
        msg: ServerToAgent,
    ) -> Option<ScheduleMessage> {
        if msg
            .command
            .as_ref()
            .filter(|_| report_capability("Command", capabilities, AcceptsRestartCommand))
            .map(|c| {
                self.callbacks
                    .on_command(c)
                    .map_err(|e| error!("on_command callback returned an error: {e}"))
            })
            .is_some()
        {
            return None;
        }

        let msg_data = message_data(&msg, capabilities);

        if let Some(id) = &msg_data.agent_identification {
            self.next_message
                .lock()
                .unwrap()
                .update(move |msg: &mut AgentToServer| {
                    msg.instance_uid = id.new_instance_uid.clone();
                });
        }

        self.callbacks.on_message(msg_data);

        if let Some(o) = msg
            .connection_settings
            .and_then(|s| s.opamp)
            .filter(|_| report_capability("opamp", capabilities, AcceptsOpAmpConnectionSettings))
        {
            self.callbacks
                .on_opamp_connection_settings(&o)
                .is_ok()
                .then(|| self.callbacks.on_opamp_connection_settings_accepted(&o));
        }

        if let Some(e) = msg.error_response {
            error!("Received an error from server: {e:?}");
        }

        self.rcv_flags(state, msg.flags).await
    }

    async fn rcv_flags(
        &self,
        state: Arc<ClientSyncedState>,
        flags: u64,
    ) -> Option<ScheduleMessage> {
        let can_report_full_state = flags & ServerToAgentFlags::ReportFullState as u64 != 0;
        can_report_full_state.then(|| {
            self.next_message
                .lock()
                .unwrap()
                .update(|msg: &mut AgentToServer| {
                    msg.agent_description = state.agent_description().ok();
                    msg.health = state.health().ok();
                    msg.remote_config_status = state.remote_config_status().ok();
                    msg.package_statuses = state.package_statuses().ok();
                    msg.effective_config = self
                        .callbacks
                        .get_effective_config()
                        .map_err(|e| error!("Cannot get effective config: {e}"))
                        .ok();
                });
            ScheduleMessage
        })
    }
}

fn message_data(msg: &ServerToAgent, capabilities: Capabilities) -> MessageData {
    let remote_config = msg
        .remote_config
        .clone()
        .filter(|_| report_capability("remote_config", capabilities, AcceptsRemoteConfig));

    let (own_metrics, own_traces, own_logs, other_connection_settings) =
        get_telemetry_connection_settings(msg.connection_settings.clone(), capabilities);

    // TODO: package_syncer feature
    // let packages_available;
    // let package_syncer;
    // if let Some(packages_available) = msg.packages_available.filter(|_| report_capability("PackagesAvailable", capabilities, AgentCapabilities::AcceptsPackages)) {
    //     let packages_available = packages_available;
    //     let package_syncer = PackageSyncer::new(packages_available, self.sender.clone());
    // }

    let agent_identification = msg.agent_identification.clone().filter(|id| {
        let is_empty_string = id.new_instance_uid.is_empty();
        if is_empty_string {
            warn!("Empty instance UID is not allowed. Ignoring agent identification.");
        }
        !is_empty_string
    });

    MessageData {
        remote_config,
        own_metrics,
        own_traces,
        own_logs,
        other_connection_settings,
        agent_identification,
        // packages_available,
        // package_syncer,
    }
}

fn report_capability(
    opt_name: &str,
    capabilities: Capabilities,
    capability: AgentCapabilities,
) -> bool {
    let has_cap = capabilities.has_capability(capability);
    if !has_cap {
        debug!(
            "Ignoring {opt_name}, agent does not have {} capability",
            capability.as_str_name()
        );
    }
    has_cap
}

type ConnectionSettings = (
    Option<TelemetryConnectionSettings>,
    Option<TelemetryConnectionSettings>,
    Option<TelemetryConnectionSettings>,
    Option<HashMap<String, OtherConnectionSettings>>,
);

fn get_telemetry_connection_settings(
    settings: Option<ConnectionSettingsOffers>,
    capabilities: Capabilities,
) -> ConnectionSettings {
    if let Some(s) = settings {
        (
            s.own_metrics
                .filter(|_| report_capability("own_metrics", capabilities, ReportsOwnMetrics)),
            s.own_traces
                .filter(|_| report_capability("own_traces", capabilities, ReportsOwnTraces)),
            s.own_logs
                .filter(|_| report_capability("own_logs", capabilities, ReportsOwnLogs)),
            Some(s.other_connections).filter(|_| {
                report_capability(
                    "other_connections",
                    capabilities,
                    AcceptsOtherConnectionSettings,
                )
            }),
        )
    } else {
        (None, None, None, None)
    }
}

#[async_trait]
impl<C, T> TransportRunner for HttpTransport<C, T>
where
    C: Callbacks + Send + Sync,
    T: Transport + Send + Sync,
{
    type State = Arc<ClientSyncedState>;
    async fn run(&mut self, _state: Self::State) -> Result<(), TransportError> {
        let send_handler = |result: Result<ServerToAgent, HttpError>| {
            match result {
                Ok(response) => {
                    debug!("Response: {:?}", response);
                    let s = self.receive(_state.clone(), capabilities, response).await;
                    if s.is_some() {
                        let msg = self.next_message.lock().unwrap().pop();
                        let _ = self.send_message(msg).await;
                    }
                }
                Err(e) => {
                    // what happens with the typed error? Callbacks< With error + From:: Http)
                    // self.callbacks.on_connect_failed(e);
                    error!("Error sending message: {}", e);
                }
            };
        };

        while let Some(_) = self.next_send().await {
            let msg = self.next_message.lock().unwrap().pop();
            send_handler(self.send_message(msg).await);
        }

        info!("HTTP Forwarder Receving channel closed! Exiting");
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod test {
    use async_trait::async_trait;
    use http::response::Builder;
    use http::StatusCode;
    use mockall::mock;
    use prost::Message;
    use tokio::{spawn, sync::mpsc::channel};

    use crate::capabilities;
    use crate::common::{clientstate::ClientSyncedState, transport::TransportRunner};
    use crate::opamp::proto::{
        AgentConfigMap, AgentDescription, AgentHealth, AgentIdentification, AgentRemoteConfig,
        EffectiveConfig, KeyValue, OpAmpConnectionSettings, PackageStatus, PackageStatuses,
        RemoteConfigStatus, ServerToAgentCommand,
    };
    use crate::operation::callbacks::test::{CallbacksMockError, MockCallbacksMockall};

    use super::*;

    #[tokio::test]
    async fn test_http_client_run() {
        let (sender, receiver) = channel(10);
        // we'll send X messages and assert they are sent and the sequence numbers are consecutive
        let messages_to_send = 4;

        let mut transport = MockTransportMockall::new();
        // we will send X messages, so we expect transport to be called 3 times
        // and we can validate the input message's sequence number
        for i in 1..messages_to_send {
            transport
                .expect_send()
                .once()
                .withf(move |req_builder: &RequestBuilder| {
                    let agent_to_server = agent_to_server_from_req(req_builder);
                    agent_to_server.sequence_num == i
                })
                .returning(|_| {
                    Ok(reqwest_response_from_server_to_agent(
                        &ServerToAgent::default(),
                        ResponseParts::default(),
                    ))
                });
        }

        // on_connect callback is called every time a message is received
        let mut callbacks_mock = MockCallbacksMockall::new();
        for _ in 1..messages_to_send {
            callbacks_mock.should_on_connect();
        }

        for _ in 1..messages_to_send {
            callbacks_mock.should_on_message(MessageData::default());
        }

        // create the http transport with the mocked transport,receiver and callbacks
        let mut runner = create_runner(receiver, transport, callbacks_mock);

        let handle = spawn(async move {
            let state = Arc::new(ClientSyncedState::default());
            let capabilities = capabilities!();
            runner.run(state, capabilities).await.unwrap();
            // drop runner to decrease arc references
            drop(runner);
        });

        // send X messages
        for _ in 1..messages_to_send {
            sender.send(()).await.unwrap();
        }
        // cancel the runner
        drop(sender);

        assert!(handle.await.is_ok());
    }

    #[tokio::test]
    async fn test_http_client_run_receive_message_on_command() {
        let (sender, receiver) = channel(10);
        // prepare the message form the server to the agent. It will just be a command
        let server_to_agent = ServerToAgent {
            command: Some(ServerToAgentCommand::default()),
            ..ServerToAgent::default()
        };

        // the http transport will return the message with a command, we don't care about validating
        // the arguments
        let mut transport = MockTransportMockall::new();
        transport.expect_send().once().returning(move |_| {
            Ok(reqwest_response_from_server_to_agent(
                &server_to_agent,
                ResponseParts::default(),
            ))
        });

        // expect on_command callback to be called
        let mut callbacks_mock = MockCallbacksMockall::new();
        callbacks_mock.should_on_connect();
        callbacks_mock
            .expect_on_command()
            .once()
            .withf(|x| x == &ServerToAgentCommand::default())
            .returning(|_| Ok(()));

        // create the http transport with the mocked transport,receiver and callbacks
        let mut runner = create_runner(receiver, transport, callbacks_mock);

        // run the http transport
        let handle = spawn(async move {
            let state = Arc::new(ClientSyncedState::default());
            let capabilities = capabilities!(AgentCapabilities::AcceptsRestartCommand);
            runner.run(state, capabilities).await.unwrap();
            // drop runner to decrease arc references
            drop(runner);
        });

        sender.send(()).await.unwrap();

        // cancel the runner
        drop(sender);

        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_http_client_opamp() {
        let (sender, receiver) = channel(10);
        // prepare the message form the server to the agent. It will just be a command
        let server_to_agent = ServerToAgent {
            connection_settings: Some(ConnectionSettingsOffers {
                opamp: Some(OpAmpConnectionSettings::default()),
                ..Default::default()
            }),
            ..Default::default()
        };

        // the http transport will return the message with a command, we don't care about validating
        // the arguments
        let mut transport = MockTransportMockall::new();
        transport.expect_send().once().returning(move |_| {
            Ok(reqwest_response_from_server_to_agent(
                &server_to_agent,
                ResponseParts::default(),
            ))
        });

        // expect on_command callback to be called
        let mut callbacks_mock = MockCallbacksMockall::new();
        callbacks_mock.should_on_connect();
        callbacks_mock.should_on_message(MessageData::default());
        callbacks_mock
            .expect_on_opamp_connection_settings()
            .once()
            .withf(|x| x == &OpAmpConnectionSettings::default())
            .returning(|_| Ok(()));
        callbacks_mock
            .expect_on_opamp_connection_settings_accepted()
            .once()
            .return_const(());

        // create the http transport with the mocked transport,receiver and callbacks
        let mut runner = create_runner(receiver, transport, callbacks_mock);

        // run the http transport
        let handle = spawn(async move {
            let state = Arc::new(ClientSyncedState::default());
            let capabilities = capabilities!(AgentCapabilities::AcceptsOpAmpConnectionSettings);
            runner.run(state, capabilities).await.unwrap();
            // drop runner to decrease arc references
            drop(runner);
        });

        sender.send(()).await.unwrap();

        // cancel the runner
        drop(sender);

        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_http_client_opamp_failed() {
        let (sender, receiver) = channel(10);
        // prepare the message form the server to the agent. It will just be a command
        let server_to_agent = ServerToAgent {
            connection_settings: Some(ConnectionSettingsOffers {
                opamp: Some(OpAmpConnectionSettings::default()),
                ..Default::default()
            }),
            ..Default::default()
        };

        // the http transport will return the message with a command, we don't care about validating
        // the arguments
        let mut transport = MockTransportMockall::new();
        transport.expect_send().once().returning(move |_| {
            Ok(reqwest_response_from_server_to_agent(
                &server_to_agent,
                ResponseParts::default(),
            ))
        });

        // expect on_command callback to be called
        let mut callbacks_mock = MockCallbacksMockall::new();
        callbacks_mock.should_on_connect();
        callbacks_mock.should_on_message(MessageData::default());
        callbacks_mock
            .expect_on_opamp_connection_settings()
            .once()
            .withf(|x| x == &OpAmpConnectionSettings::default())
            .returning(|_| Err(CallbacksMockError));
        callbacks_mock
            .expect_on_opamp_connection_settings_accepted()
            .never();

        // create the http transport with the mocked transport,receiver and callbacks
        let mut runner = create_runner(receiver, transport, callbacks_mock);

        // run the http transport
        let handle = spawn(async move {
            let state = Arc::new(ClientSyncedState::default());
            let capabilities = capabilities!(AgentCapabilities::AcceptsOpAmpConnectionSettings);
            runner.run(state, capabilities).await.unwrap();
            // drop runner to decrease arc references
            drop(runner);
        });

        sender.send(()).await.unwrap();

        // cancel the runner
        drop(sender);

        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_rcv_flags_no_schedule() {
        let (_, receiver) = channel(1);
        let callbacks_mock = MockCallbacksMockall::new();
        let transport = MockTransportMockall::new();
        let state = Arc::new(ClientSyncedState::default());

        // create the http transport with the mocked transport,receiver and callbacks
        let runner = create_runner(receiver, transport, callbacks_mock);

        let flags = 0;
        // Test when can_report_full_state is false
        let result = runner.rcv_flags(state.clone(), flags).await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_rcv_flags_schedule_msg() {
        let (_, receiver) = channel(1);

        let test_agent_description = AgentDescription {
            identifying_attributes: vec![KeyValue {
                key: "test".to_string(),
                value: None,
            }],
            ..Default::default()
        };
        let test_health = AgentHealth {
            healthy: true,
            ..Default::default()
        };
        let test_remote_config_status = RemoteConfigStatus {
            last_remote_config_hash: vec![0, 1, 2, 3, 4, 5],
            ..Default::default()
        };
        let test_package_statuses = PackageStatuses {
            packages: [("random_package".to_string(), PackageStatus::default())]
                .iter()
                .cloned()
                .collect(),
            ..Default::default()
        };

        let mut callbacks_mock = MockCallbacksMockall::new();
        callbacks_mock
            .expect_get_effective_config()
            .once()
            .returning(|| Ok(EffectiveConfig::default()));
        let transport = MockTransportMockall::new();
        let state = Arc::new(ClientSyncedState::default());

        let _ = state.set_agent_description(test_agent_description.clone());
        let _ = state.set_health(test_health.clone());
        let _ = state.set_remote_config_status(test_remote_config_status.clone());
        let _ = state.set_package_statuses(test_package_statuses.clone());

        // create the http transport with the mocked transport,receiver and callbacks
        let runner = create_runner(receiver, transport, callbacks_mock);
        let flags = ServerToAgentFlags::ReportFullState as u64;
        // Test when can_report_full_state is true
        let result = runner.rcv_flags(state.clone(), flags).await;
        assert_eq!(result, Some(ScheduleMessage));

        let actual_next_msg = runner.next_message.lock().unwrap().pop();
        assert_eq!(
            actual_next_msg.agent_description,
            Some(test_agent_description)
        );
        assert_eq!(actual_next_msg.health, Some(test_health));
        assert_eq!(
            actual_next_msg.remote_config_status,
            Some(test_remote_config_status)
        );
        assert_eq!(
            actual_next_msg.package_statuses,
            Some(test_package_statuses)
        );
    }

    #[tokio::test]
    async fn test_http_client_agent_identification() {
        let (sender, receiver) = channel(10);
        // prepare the message form the server to the agent. It will just be a command
        let server_to_agent = ServerToAgent {
            agent_identification: Some(AgentIdentification {
                new_instance_uid: "test_instance_uid".to_string(),
            }),
            ..Default::default()
        };

        // the http transport will return the message with a command, we don't care about validating
        // the arguments
        let mut transport = MockTransportMockall::new();
        transport.expect_send().once().returning(move |_| {
            Ok(reqwest_response_from_server_to_agent(
                &server_to_agent,
                ResponseParts::default(),
            ))
        });

        // expect on_command callback to be called
        let mut callbacks_mock = MockCallbacksMockall::new();
        callbacks_mock.should_on_connect();
        callbacks_mock.should_on_message(MessageData::default());

        // create the http transport with the mocked transport,receiver and callbacks
        let mut runner = create_runner(receiver, transport, callbacks_mock);

        // run the http transport
        let handle = spawn(async move {
            let state = Arc::new(ClientSyncedState::default());
            let capabilities = capabilities!();
            runner.run(state, capabilities).await.unwrap();
            assert_eq!(
                runner.next_message.lock().unwrap().pop().instance_uid,
                "test_instance_uid".to_string()
            );
            // drop runner to decrease arc references
            drop(runner);
        });

        sender.send(()).await.unwrap();

        // cancel the runner
        drop(sender);

        handle.await.unwrap();
    }

    #[test]
    fn test_message_data() {
        let msg = ServerToAgent::default();

        let capabilities = Capabilities::default();

        let message_data = message_data(&msg, capabilities);

        assert_eq!(message_data.remote_config, None);
        assert_eq!(message_data.own_metrics, None);
        assert_eq!(message_data.own_traces, None);
        assert_eq!(message_data.own_logs, None);
        assert_eq!(message_data.other_connection_settings, None);
        assert_eq!(message_data.agent_identification, None);
    }

    #[test]
    fn test_message_data_with_remote_config() {
        let remote_config = AgentRemoteConfig {
            config: Some(AgentConfigMap {
                config_map: Default::default(),
            }),
            config_hash: Default::default(),
        };

        let msg = ServerToAgent {
            remote_config: Some(remote_config.clone()),
            ..Default::default()
        };

        let capabilities = capabilities!(AgentCapabilities::AcceptsRemoteConfig);

        let message_data = message_data(&msg, capabilities);

        assert_eq!(message_data.remote_config, Some(remote_config));
        assert_eq!(message_data.own_metrics, None);
        assert_eq!(message_data.own_traces, None);
        assert_eq!(message_data.own_logs, None);
        assert_eq!(message_data.other_connection_settings, None);
        assert_eq!(message_data.agent_identification, None);
    }

    #[test]
    fn test_message_data_with_agent_identification() {
        let agent_identification = AgentIdentification {
            new_instance_uid: "test-instance-uid".to_string(),
        };

        let msg = ServerToAgent {
            agent_identification: Some(agent_identification),
            ..Default::default()
        };

        let capabilities = capabilities!();

        let message_data = message_data(&msg, capabilities);

        assert_eq!(message_data.remote_config, None);
        assert_eq!(message_data.own_metrics, None);
        assert_eq!(message_data.own_traces, None);
        assert_eq!(message_data.own_logs, None);
        assert_eq!(message_data.other_connection_settings, None);
        assert_eq!(
            message_data.agent_identification,
            Some(AgentIdentification {
                new_instance_uid: "test-instance-uid".to_string()
            })
        );
    }

    #[test]
    fn test_message_data_with_agent_identification_and_empty_instance_uid() {
        let agent_identification = AgentIdentification {
            new_instance_uid: "".to_string(),
        };

        let msg = ServerToAgent {
            agent_identification: Some(agent_identification),
            ..Default::default()
        };

        let capabilities = capabilities!();

        let message_data = message_data(&msg, capabilities);

        assert_eq!(message_data.remote_config, None);
        assert_eq!(message_data.own_metrics, None);
        assert_eq!(message_data.own_traces, None);
        assert_eq!(message_data.own_logs, None);
        assert_eq!(message_data.other_connection_settings, None);
        assert_eq!(message_data.agent_identification, None);
    }

    #[test]
    fn test_get_telemetry_connection_settings_metrics() {
        // Test with reporting metrics
        let capabilities = capabilities!(ReportsOwnMetrics);
        let settings = Some(ConnectionSettingsOffers {
            own_metrics: Some(TelemetryConnectionSettings::default()),
            own_traces: Some(TelemetryConnectionSettings::default()),
            own_logs: Some(TelemetryConnectionSettings::default()),
            other_connections: HashMap::default(),
            ..Default::default()
        });
        let expected = (
            Some(TelemetryConnectionSettings::default()),
            None,
            None,
            None,
        );
        assert_eq!(
            get_telemetry_connection_settings(settings, capabilities),
            expected
        );
    }

    #[test]
    fn test_get_telemetry_connection_settings_traces() {
        // Test with reporting traces
        let capabilities = capabilities!(ReportsOwnTraces);
        let settings = Some(ConnectionSettingsOffers {
            own_metrics: Some(TelemetryConnectionSettings::default()),
            own_traces: Some(TelemetryConnectionSettings::default()),
            own_logs: Some(TelemetryConnectionSettings::default()),
            other_connections: HashMap::default(),
            ..Default::default()
        });
        let expected = (
            None,
            Some(TelemetryConnectionSettings::default()),
            None,
            None,
        );
        assert_eq!(
            get_telemetry_connection_settings(settings, capabilities),
            expected
        );
    }

    #[test]
    fn test_get_telemetry_connection_settings_logs() {
        // Test with reporting logs
        let capabilities = capabilities!(ReportsOwnLogs);
        let settings = Some(ConnectionSettingsOffers {
            own_metrics: Some(TelemetryConnectionSettings::default()),
            own_traces: Some(TelemetryConnectionSettings::default()),
            own_logs: Some(TelemetryConnectionSettings::default()),
            other_connections: HashMap::default(),
            ..Default::default()
        });
        let expected = (
            None,
            None,
            Some(TelemetryConnectionSettings::default()),
            None,
        );
        assert_eq!(
            get_telemetry_connection_settings(settings, capabilities),
            expected
        );
    }

    #[test]
    fn test_get_telemetry_connection_settings_other_conns() {
        // Test with reporting other connections
        let other_conn: HashMap<String, OtherConnectionSettings> =
            [("example".to_string(), OtherConnectionSettings::default())]
                .iter()
                .cloned()
                .collect();
        let capabilities = capabilities!(AcceptsOtherConnectionSettings);
        let settings = Some(ConnectionSettingsOffers {
            own_metrics: Some(TelemetryConnectionSettings::default()),
            own_traces: Some(TelemetryConnectionSettings::default()),
            own_logs: Some(TelemetryConnectionSettings::default()),
            other_connections: other_conn.clone(),
            ..Default::default()
        });
        let expected = (None, None, None, Some(other_conn));
        assert_eq!(
            get_telemetry_connection_settings(settings, capabilities),
            expected
        );
    }

    #[test]
    fn test_get_telemetry_connection_settings_no_capabilities() {
        // Test with no capabilities
        let capabilities = capabilities!();
        let settings = Some(ConnectionSettingsOffers {
            own_metrics: Some(TelemetryConnectionSettings::default()),
            own_traces: Some(TelemetryConnectionSettings::default()),
            own_logs: Some(TelemetryConnectionSettings::default()),
            other_connections: HashMap::default(),
            ..Default::default()
        });
        let expected = (None, None, None, None);
        assert_eq!(
            get_telemetry_connection_settings(settings, capabilities),
            expected
        );

        // Test with no settings
        let capabilities = capabilities!();
        let settings = None;
        let expected = (None, None, None, None);
        assert_eq!(
            get_telemetry_connection_settings(settings, capabilities),
            expected
        );
    }

    #[test]
    fn test_get_telemetry_connection_settings_several_capabilities() {
        let capabilities = capabilities!(ReportsOwnMetrics, ReportsOwnTraces, ReportsOwnLogs);
        let settings = Some(ConnectionSettingsOffers {
            own_metrics: Some(TelemetryConnectionSettings::default()),
            own_traces: Some(TelemetryConnectionSettings::default()),
            own_logs: Some(TelemetryConnectionSettings::default()),
            other_connections: HashMap::default(),
            ..Default::default()
        });
        let expected = (
            Some(TelemetryConnectionSettings::default()),
            Some(TelemetryConnectionSettings::default()),
            Some(TelemetryConnectionSettings::default()),
            None,
        );
        assert_eq!(
            get_telemetry_connection_settings(settings, capabilities),
            expected
        );
    }

    #[tokio::test]
    async fn client_gzip_headers() {
        let http_config =
            HttpConfig::new(url::Url::parse("http://example.com").unwrap()).with_gzip_compression();

        assert_eq!(
            http_config.headers.get("Content-Encoding"),
            Some(&HeaderValue::from_static("gzip"))
        );
        assert_eq!(
            http_config.headers.get("Accept-Encoding"),
            Some(&HeaderValue::from_static("gzip"))
        )
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn send_error_should_be_logged_and_not_stop_runner() {
        let (sender, receiver) = channel(10);

        let mut transport = MockTransportMockall::new();
        let agent_to_server = AgentToServer {
            sequence_num: 1,
            ..Default::default()
        };

        //we force a transport error
        transport.should_not_send(agent_to_server, TransportError::Invalid);

        // create the http transport with the mocked transport,receiver and callbacks
        let callbacks_mock = MockCallbacksMockall::new();
        let mut runner = create_runner(receiver, transport, callbacks_mock);

        let handle = spawn(async move {
            let state = Arc::new(ClientSyncedState::default());
            let capabilities = capabilities!();

            runner.run(state, capabilities).await.unwrap();

            // drop runner to decrease arc references
            drop(runner);
        });

        // send X messages
        sender.send(()).await.unwrap();

        // cancel the runner
        drop(sender);

        assert!(handle.await.is_ok());

        //assert on logs
        assert!(logs_contain("Error sending message: `some error`"));
        assert!(logs_contain(
            "HTTP Forwarder Receiving channel closed! Exiting"
        ));
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn unsupported_compression_should_be_logged_and_not_stop_the_runner() {
        let (sender, receiver) = channel(10);

        let agent_to_server = AgentToServer {
            sequence_num: 1,
            ..Default::default()
        };
        let response = reqwest_response_from_server_to_agent(
            &ServerToAgent::default(),
            ResponseParts {
                headers: HashMap::from([(
                    "Content-Encoding".to_string(),
                    "unsupported".to_string(),
                )]),
                ..Default::default()
            },
        );

        let mut transport = MockTransportMockall::new();
        transport.should_send(agent_to_server, response);

        // create the http transport with the mocked transport,receiver and callbacks
        let callbacks_mock = MockCallbacksMockall::new();
        let mut runner = create_runner(receiver, transport, callbacks_mock);

        let handle = spawn(async move {
            let state = Arc::new(ClientSyncedState::default());
            let capabilities = capabilities!();
            runner.run(state, capabilities).await.unwrap();
            // drop runner to decrease arc references
            drop(runner);
        });

        // send X messages
        sender.send(()).await.unwrap();

        // cancel the runner
        drop(sender);

        assert!(handle.await.is_ok());

        //assert on logs
        assert!(logs_contain(
            "Error sending message: `encoding format not supported: `unsupported``"
        ));
        assert!(logs_contain(
            "HTTP Forwarder Receiving channel closed! Exiting"
        ));
    }

    /////////////////////////////////////////////
    // Test helpers & mocks
    /////////////////////////////////////////////

    // Define a struct to represent the mock client
    mock! {
      pub(crate) TransportMockall {}

        #[async_trait]
        impl Transport for TransportMockall {
             async fn send(&self,request: reqwest::RequestBuilder) -> Result<reqwest::Response, TransportError>;
        }
    }

    impl MockTransportMockall {
        fn should_send(&mut self, agent_to_server: AgentToServer, reqwest_response: Response) {
            self.expect_send()
                .once()
                .withf(move |req_builder: &RequestBuilder| {
                    let expected_agent_to_server = agent_to_server_from_req(req_builder);
                    expected_agent_to_server.eq(&agent_to_server)
                })
                .return_once(move |_| Ok(reqwest_response));
        }

        fn should_not_send(&mut self, agent_to_server: AgentToServer, error: TransportError) {
            self.expect_send()
                .once()
                .withf(move |req_builder: &RequestBuilder| {
                    let expected_agent_to_server = agent_to_server_from_req(req_builder);
                    expected_agent_to_server.eq(&agent_to_server)
                })
                .return_once(move |_| Err(error));
        }
    }

    struct ResponseParts {
        status: StatusCode,
        headers: HashMap<String, String>,
    }

    impl Default for ResponseParts {
        fn default() -> Self {
            ResponseParts {
                status: StatusCode::OK,
                headers: HashMap::new(),
            }
        }
    }

    // Create a reqwest response from a ServerToAgent
    fn reqwest_response_from_server_to_agent(
        server_to_agent: &ServerToAgent,
        response_parts: ResponseParts,
    ) -> Response {
        let mut buf = vec![];
        let _ = &server_to_agent.encode(&mut buf);

        let mut response_builder = Builder::new();
        for (k, v) in response_parts.headers {
            response_builder = response_builder.header(k, v);
        }

        let response = response_builder
            .status(response_parts.status)
            .body(buf)
            .unwrap();

        Response::from(response)
    }

    // Create a ServerToAgent struct from a request body
    fn agent_to_server_from_req(req_builder: &RequestBuilder) -> AgentToServer {
        let request = req_builder.try_clone().unwrap().build().unwrap();
        let body = request.body().unwrap().as_bytes().unwrap();
        AgentToServer::decode(body).unwrap()
    }

    // Create runner
    fn create_runner<C, T>(
        receiver: Receiver<()>,
        transport: T,
        callbacks: C,
    ) -> HttpTransport<C, T>
    where
        C: Callbacks,
        T: Transport,
    {
        let next_message = Arc::new(Mutex::new(NextMessage::new()));
        HttpTransport::with_transport(
            HttpConfig::new(url::Url::parse("http://example.com").unwrap()),
            Duration::from_secs(100),
            transport,
            receiver,
            next_message,
            callbacks,
        )
        .unwrap()
    }
}
