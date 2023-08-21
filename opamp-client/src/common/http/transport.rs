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
use tracing::{debug, error, info};
use url::Url;

use super::compression::{Compressor, CompressorError, DecoderError, EncoderError};

#[async_trait]
pub trait Transport {
    type Error: std::error::Error + Send + Sync;
    async fn send(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, reqwest::Error>;
}

pub struct ReqwestSender;

#[async_trait]
impl Transport for ReqwestSender {
    type Error = reqwest::Error;

    async fn send(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, Self::Error> {
        request.send().await
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
            _ = self.polling.tick() => {
                Some(())
            }
            recv_result = self.pending_messages.recv() => recv_result
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
                Ok(response) => debug!("Response: {:?}", response),
                Err(e) => error!("Error sending message: {:?}", e),
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

    //transport mock with mockall
    mock! {
      pub(crate) TransportMockall {}

        #[async_trait]
        impl Transport for TransportMockall {
            type Error = reqwest::Error;

             async fn send(&self,request: reqwest::RequestBuilder) -> Result<reqwest::Response, reqwest::Error>;
        }
    }

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
                    ))
                });
        }

        // on_connect callback is called every time a message is received
        let mut callbacks_mock = MockCallbacksMockall::new();
        for _ in 1..messages_to_send {
            callbacks_mock.expect_on_connect().once().return_const(());
        }

        for _ in 1..messages_to_send {
            callbacks_mock.expect_on_message().once().return_const(());
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
        let mut runner = HttpTransport::with_transport(
            HttpConfig::new(url::Url::parse("http://example.com").unwrap()),
            Duration::from_secs(100),
            transport,
            receiver,
            next_message,
            callbacks,
        )
        .unwrap()
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
}
