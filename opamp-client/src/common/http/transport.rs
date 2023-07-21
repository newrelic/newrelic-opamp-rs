use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{
    common::{
        asyncsender::{TransportError, TransportRunner},
        nextmessage::NextMessage,
    },
    opamp::proto::{AgentToServer, ServerToAgent},
};
use async_trait::async_trait;
use http::HeaderMap;
use log::{debug, error, info};
use prost::DecodeError;
use prost::Message;
use thiserror::Error;
use tokio::{select, sync::mpsc::Receiver, time::interval};

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

fn opamp_headers(custom_headers: Option<HeaderMap>) -> HeaderMap {
    let mut headers = custom_headers.unwrap_or(HeaderMap::new());

    headers.insert("Content-Type", "application/x-protobuf".parse().unwrap());
    headers
}

#[derive(Debug)]
pub struct HttpTransport<T: Transport = ReqwestSender> {
    pub(crate) pending_messages: Receiver<()>,
    http_client: reqwest::Client,
    url: url::Url,
    // key-value vector of headers
    headers: HeaderMap,
    sender: T,

    // polling interval has passed. Force a status update.
    polling: Duration,
    // next message structure for forced status updates
    next_message: Arc<Mutex<NextMessage>>,
}

#[derive(Error, Debug)]
pub(crate) enum HttpError {
    #[error("protobuf decode error: `{0}`")]
    ProtobufDecoding(DecodeError),
    #[error("reqwest error: `{0}`")]
    ReqwestError(#[from] reqwest::Error),
}

impl HttpTransport<ReqwestSender> {
    pub(crate) fn new(
        url: url::Url,
        headers: Option<HeaderMap>,
        polling: Duration,
        pending_messages: Receiver<()>,
        next_message: Arc<Mutex<NextMessage>>,
    ) -> HttpTransport {
        HttpTransport {
            http_client: reqwest::Client::new(),
            sender: ReqwestSender {},
            pending_messages,
            url,
            headers: opamp_headers(headers),
            polling,
            next_message,
        }
    }
}

impl<T: Transport> HttpTransport<T> {
    pub(crate) fn with_transport(
        url: url::Url,
        headers: Option<HeaderMap>,
        polling: Duration,
        transport: T,
        pending_messages: Receiver<()>,
        next_message: Arc<Mutex<NextMessage>>,
    ) -> HttpTransport<T> {
        HttpTransport {
            http_client: reqwest::Client::new(),
            sender: transport,
            pending_messages,
            url,
            headers: opamp_headers(headers),
            polling,
            next_message,
        }
    }

    async fn send_message(&self, msg: AgentToServer) -> Result<ServerToAgent, HttpError> {
        // Serialize the message to bytes
        let bytes = msg.encode_to_vec();

        let response = self
            .sender
            .send(
                self.http_client
                    .post(self.url.clone())
                    .headers(self.headers.clone())
                    .body(reqwest::Body::from(bytes)),
            )
            .await?;

        // decode response
        let response_bytes = response.bytes().await?;

        // TODO
        // if let Some(encoding) = headers.get("Content-Encoding") {
        //     if encoding == "gzip" {
        //         println!("gzip encoding")
        //     }
        //     println!("Response is encoded!")
        // }

        ServerToAgent::decode(response_bytes).map_err(|err| HttpError::ProtobufDecoding(err))
    }
}

#[async_trait]
impl<T: Transport + Send + Sync> TransportRunner for HttpTransport<T> {
    async fn run(&mut self) -> Result<(), TransportError> {
        let mut polling_ticker = interval(self.polling);

        let send_handler = |result: Result<ServerToAgent, HttpError>| {
            match result {
                Ok(response) => debug!("Response: {:?}", response),
                Err(e) => error!("Error sending message: {:?}", e),
            };
        };

        loop {
            select! {
                _ = polling_ticker.tick() => {
                    debug!("Polling ticker triggered, forcing status update");
                    let msg = self.next_message.lock().unwrap().pop();
                    send_handler(self.send_message(msg).await);
                }
                recv_result = self.pending_messages.recv() => match recv_result {
                        Some(()) => {
                            debug!("Pending message notification, sending message");
                            let msg = self.next_message.lock().unwrap().pop();
                            send_handler(self.send_message(msg).await);
                        }
                        None => {
                            info!("HTTP Forwarder Receving channel closed! Exiting");
                            break;
                        }
                    },
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod test {

    use std::sync::atomic::AtomicUsize;

    use async_trait::async_trait;
    use tokio::{spawn, sync::mpsc::channel};

    use crate::common::asyncsender::TransportRunner;

    use super::*;

    // Define a struct to represent the mock client
    pub struct TransportMock {
        counter: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Transport for TransportMock {
        type Error = reqwest::Error;

        async fn send(
            &self,
            _request: reqwest::RequestBuilder,
        ) -> Result<reqwest::Response, Self::Error> {
            self.counter
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(http::Response::new(ServerToAgent::default().encode_to_vec()).into())
        }
    }

    #[tokio::test]
    async fn test_http_client_run() {
        let next_message = Arc::new(Mutex::new(NextMessage::new()));
        let (sender, receiver) = channel(10);

        let counter = Arc::new(AtomicUsize::new(0));
        let transport = TransportMock {
            counter: counter.clone(),
        };
        let mut runner = HttpTransport::with_transport(
            url::Url::parse("http://example.com").unwrap(),
            None,
            Duration::from_secs(100),
            transport,
            receiver,
            next_message,
        );

        let handle = spawn(async move {
            runner.run().await.unwrap();
            counter.load(std::sync::atomic::Ordering::Relaxed)
        });

        // send 3 messages
        sender.send(()).await.unwrap();
        sender.send(()).await.unwrap();
        sender.send(()).await.unwrap();
        // cancel the runner
        drop(sender);

        assert_eq!(handle.await.unwrap(), 3);
    }
}
