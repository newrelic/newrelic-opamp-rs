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
use tracing::{debug, error, info};
use url::Url;

use crate::{
    common::{
        clientstate::ClientSyncedState,
        http::compression::{decode_message, encode_message},
        nextmessage::NextMessage,
        transport::{TransportError, TransportRunner},
    },
    opamp::proto::{AgentToServer, ServerToAgent, ServerToAgentFlags},
    operation::callbacks::Callbacks,
};
use crate::operation::syncedstate::SyncedState;

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
            I: IntoIterator<Item=(&'a str, &'a str)>,
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
            Some(algorithm) => Compressor::try_from(algorithm.as_bytes())?,
            None => Compressor::Plain,
        };

        let response = decode_message::<ServerToAgent>(&compression, &response.bytes().await?)?;

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

    async fn receive(&self, state: Arc<ClientSyncedState>, msg: ServerToAgent) {
        if let Some(server_to_agent_command) = msg.command {
            // TODO : Should on_command return the error? The error in on_commands belogs
            // to the library user implementation and IMHO should not be used here (like the Go one)
            _ = self.callbacks.on_command(&server_to_agent_command);
            return;
        }

        let scheduled = self.rcv_flags(state, msg.flags);
        println!("{:?}", scheduled.await);

    }

    async fn rcv_flags(&self, state: Arc<ClientSyncedState>, flags: u64) -> Option<bool> {
        // if flags & ServerToAgentFlags::ReportFullState != 0 {
        // if true {
        //     self.next_message.lock()?.update(|msg: &mut AgentToServer| {
        //         msg.AgentDescription = state.agent_description();
        //         msg.Health = state.health();
        //         // TODO...
        //     });


            // Go code...
            // cfg, err := r.callbacks.GetEffectiveConfig(ctx)
            // if err != nil {
            //     r.logger.Errorf("Cannot GetEffectiveConfig: %v", err)
            //     cfg = nil
            // }
            //
            // r.sender.NextMessage().Update(
            //     func(msg *protobufs.AgentToServer) {
            //         msg.AgentDescription = r.clientSyncedState.AgentDescription()
            //         msg.Health = r.clientSyncedState.Health()
            //         msg.RemoteConfigStatus = r.clientSyncedState.RemoteConfigStatus()
            //         msg.PackageStatuses = r.clientSyncedState.PackageStatuses()
            //
            //         // The logic for EffectiveConfig is similar to the previous 4 sub-messages however
            //         // the EffectiveConfig is fetched using GetEffectiveConfig instead of
            //         // from clientSyncedState. We do this to avoid keeping EffectiveConfig in-memory.
            //         msg.EffectiveConfig = cfg
            //     },
            // )
            // scheduleSend = true
        // }
        return Some(true);
    }
}

#[async_trait]
impl<C: Callbacks + Send + Sync, T: Transport + Send + Sync> TransportRunner
for HttpTransport<C, T>
{
    type State = Arc<ClientSyncedState>;
    async fn run(&mut self, _state: Self::State) -> Result<(), TransportError> {
        while let Some(_) = self.next_send().await {
            let msg = self.next_message.lock().unwrap().pop();
            let result = self.send_message(msg).await;
            match result {
                Ok(response) => {
                    debug!("Response: {:?}", response);
                    self.callbacks.on_connect();
                    self.receive(_state.clone(), response).await;
                }
                Err(e) => {
                    // what happens with the typed error? Callbacks< With error + From:: Http)
                    // self.callbacks.on_connect_failed(e);
                    error!("Error sending message: {:?}", e);
                }
            };
        }

        info!("HTTP Forwarder Receving channel closed! Exiting");
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod test {
    use async_trait::async_trait;
    use http::response::Builder;
    use mockall::mock;
    use prost::Message;
    use reqwest::{RequestBuilder, Response};
    use tokio::{spawn, sync::mpsc::channel};

    use crate::{
        common::{clientstate::ClientSyncedState, transport::TransportRunner},
    };
    use crate::opamp::proto::ServerToAgentCommand;
    use crate::operation::callbacks::test::MockCallbacksMockall;

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
            transport.expect_send()
                .times(1)
                .withf(move |req_builder: &RequestBuilder| {
                    let agent_to_server = agent_to_server_from_req(req_builder);
                    agent_to_server.sequence_num == i
                })
                .returning(|_| {
                    Ok(reqwest_response_from_server_to_agent(&ServerToAgent::default()))
                });
        }

        // on_connect callback is called every time a message is received
        let mut callbacks_mock = MockCallbacksMockall::new();
        for _ in 1..messages_to_send {
            callbacks_mock.expect_on_connect().times(1).return_const(());
        }

        // create the http transport with the mocked transport,receiver and callbacks
        let mut runner = create_runner(receiver, transport, callbacks_mock);

        let handle = spawn(async move {
            let state = Arc::new(ClientSyncedState::default());
            runner.run(state).await.unwrap();
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
        let mut server_to_agent = ServerToAgent::default();
        server_to_agent.command = Some(ServerToAgentCommand::default());

        // the http transport will return the message with a command, we don't care about validating
        // the arguments
        let mut transport = MockTransportMockall::new();
        transport.expect_send()
            .once()
            .returning(move |_| {
                Ok(reqwest_response_from_server_to_agent(&server_to_agent))
            });

        // expect on_command callback to be called
        let mut callbacks_mock = MockCallbacksMockall::new();
        callbacks_mock.expect_on_connect().times(1).return_const(());
        callbacks_mock.expect_on_command()
            .once()
            .withf(|x| { x == &ServerToAgentCommand::default() })
            .returning(|_| {
                Ok(())
            });

        // create the http transport with the mocked transport,receiver and callbacks
        let mut runner = create_runner(receiver, transport, callbacks_mock);

        // run the http transport
        let handle = spawn(async move {
            let state = Arc::new(ClientSyncedState::default());
            runner.run(state).await.unwrap();
            // drop runner to decrease arc references
            drop(runner);
        });

        sender.send(()).await.unwrap();

        // cancel the runner
        drop(sender);

        handle.await.unwrap();
    }

    /////////////////////////////////////////////
    // Test helpers
    /////////////////////////////////////////////

    // Create a reqwest response from a ServerToAgent
    fn reqwest_response_from_server_to_agent(server_to_agent: &ServerToAgent) -> Response {
        let mut buf = vec![];
        let _ = &server_to_agent.encode(&mut buf);

        let response = Builder::new()
            .status(200)
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
    fn create_runner<C, T>(receiver: Receiver<()>, transport: T, callbacks: C) -> HttpTransport<C, T> where C: Callbacks, T: Transport {
        let next_message = Arc::new(Mutex::new(NextMessage::new()));
        HttpTransport::with_transport(
            HttpConfig::new(url::Url::parse("http://example.com").unwrap()),
            Duration::from_secs(100),
            transport,
            receiver,
            next_message,
            callbacks,
        ).unwrap()
    }
}
