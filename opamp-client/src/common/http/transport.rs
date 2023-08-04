use std::{
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{
    common::{
        clientstate::ClientSyncedState,
        http::compression::{decode_message, encode_message},
        nextmessage::NextMessage,
        transport::{TransportError, TransportRunner},
    },
    opamp::proto::{AgentToServer, ServerToAgent},
    operation::callbacks::Callbacks,
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
}

#[async_trait]
impl<C: Callbacks + Send + Sync, T: Transport + Send + Sync> TransportRunner
    for HttpTransport<C, T>
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

    use crate::{
        common::{clientstate::ClientSyncedState, transport::TransportRunner},
        operation::callbacks::test::CallbacksMock,
    };

    use super::*;

    // Define a struct to represent the mock client
    pub struct TransportMock {
        messages: Arc<Mutex<Vec<AgentToServer>>>,
    }

    #[async_trait]
    impl Transport for TransportMock {
        type Error = reqwest::Error;

        async fn send(
            &self,
            request: reqwest::RequestBuilder,
        ) -> Result<reqwest::Response, Self::Error> {
            // get acutal message
            let binding = request.build()?;
            let msg_bytes = binding.body().unwrap().as_bytes().unwrap();
            let agent_to_server_msg = AgentToServer::decode(msg_bytes).unwrap();

            self.messages.lock().unwrap().push(agent_to_server_msg);
            Ok(http::Response::new(ServerToAgent::default().encode_to_vec()).into())
        }
    }

    #[tokio::test]
    async fn test_http_client_run() {
        let next_message = Arc::new(Mutex::new(NextMessage::new()));
        let (sender, receiver) = channel(10);

        let counter = Arc::new(Mutex::new(vec![]));
        let transport = TransportMock {
            messages: counter.clone(),
        };
        let mut runner = HttpTransport::with_transport(
            HttpConfig::new(url::Url::parse("http://example.com").unwrap()),
            Duration::from_secs(100),
            transport,
            receiver,
            next_message,
            CallbacksMock {},
        )
        .unwrap();

        let handle = spawn(async move {
            let state = Arc::new(ClientSyncedState::default());
            runner.run(state).await.unwrap();
            // drop runner to decrease arc references
            drop(runner);
            Arc::into_inner(counter)
        });

        // send 3 messages
        sender.send(()).await.unwrap();
        sender.send(()).await.unwrap();
        sender.send(()).await.unwrap();
        // cancel the runner
        drop(sender);

        let send_messages = handle.await.unwrap().unwrap();

        // 3 messages where send
        assert_eq!(send_messages.lock().unwrap().len(), 3);

        // consecutive sequence numbers
        assert_eq!(
            send_messages
                .lock()
                .unwrap()
                .iter()
                .fold(vec![], |mut acc, msg| {
                    acc.push(msg.sequence_num);
                    acc
                }),
            vec![1, 2, 3],
        );
    }
}
