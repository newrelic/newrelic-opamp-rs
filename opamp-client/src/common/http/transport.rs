use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{
    common::{
        nextmessage::NextMessage,
        transport::{TransportError, TransportRunner},
    },
    opamp::proto::{AgentToServer, ServerToAgent},
    operation::callbacks::Callbacks,
};
use async_trait::async_trait;
use http::{header::InvalidHeaderValue, HeaderMap};
use log::{debug, error, info};
use prost::DecodeError;
use prost::Message;
use reqwest::Client;
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

fn opamp_headers(custom_headers: Option<HeaderMap>) -> Result<HeaderMap, HttpError> {
    let mut headers = custom_headers.unwrap_or(HeaderMap::new());

    headers.insert("Content-Type", "application/x-protobuf".parse()?);
    Ok(headers)
}

#[derive(Debug)]
pub struct HttpTransport<C, T: Transport = ReqwestSender>
where
    C: Callbacks,
{
    pub(crate) pending_messages: Receiver<()>,
    http_client: reqwest::Client,
    url: url::Url,
    sender: T,

    // callbacks function when a new message is received
    callbacks: C,

    // polling interval has passed. Force a status update.
    polling: Duration,
    // next message structure for forced status updates
    next_message: Arc<Mutex<NextMessage>>,
}

#[derive(Error, Debug)]
pub enum HttpError {
    #[error("`{0}`")]
    ProtobufDecoding(#[from] DecodeError),
    #[error("`{0}`")]
    ReqwestError(#[from] reqwest::Error),
    #[error("`{0}`")]
    InvalidHeader(#[from] InvalidHeaderValue),
}

impl<C> HttpTransport<C, ReqwestSender>
where
    C: Callbacks,
{
    pub(crate) fn new(
        url: url::Url,
        headers: Option<HeaderMap>,
        polling: Duration,
        pending_messages: Receiver<()>,
        next_message: Arc<Mutex<NextMessage>>,
        callbacks: C,
    ) -> Result<Self, HttpError> {
        let http_client = Client::builder()
            .default_headers(opamp_headers(headers)?)
            .build()?;

        Ok(HttpTransport {
            http_client,
            sender: ReqwestSender {},
            pending_messages,
            url,
            polling,
            next_message,
            callbacks,
        })
    }
}

impl<C: Callbacks, T: Transport> HttpTransport<C, T> {
    pub(crate) fn with_transport(
        url: url::Url,
        headers: Option<HeaderMap>,
        polling: Duration,
        transport: T,
        pending_messages: Receiver<()>,
        next_message: Arc<Mutex<NextMessage>>,
        callbacks: C,
    ) -> Result<HttpTransport<C, T>, HttpError> {
        let http_client = Client::builder()
            .default_headers(opamp_headers(headers)?)
            .build()?;
        Ok(HttpTransport {
            http_client,
            sender: transport,
            pending_messages,
            url,
            polling,
            next_message,
            callbacks,
        })
    }

    async fn send_message(&self, msg: AgentToServer) -> Result<ServerToAgent, HttpError> {
        // Serialize the message to bytes
        let bytes = msg.encode_to_vec();

        let response = self
            .sender
            .send(
                self.http_client
                    .post(self.url.clone())
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

        let response = ServerToAgent::decode(response_bytes)?;

        self.callbacks.on_connect();

        Ok(response)
    }
}

#[async_trait]
impl<C: Callbacks + Send + Sync, T: Transport + Send + Sync> TransportRunner
    for HttpTransport<C, T>
{
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

    use async_trait::async_trait;
    use tokio::{spawn, sync::mpsc::channel};

    use crate::{common::transport::TransportRunner, operation::callbacks::test::CallbacksMock};

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
            url::Url::parse("http://example.com").unwrap(),
            None,
            Duration::from_secs(100),
            transport,
            receiver,
            next_message,
            CallbacksMock {},
        )
        .unwrap();

        let handle = spawn(async move {
            runner.run().await.unwrap();
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
