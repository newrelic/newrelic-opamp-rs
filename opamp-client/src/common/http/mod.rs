use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use http::HeaderMap;
use thiserror::Error;
use tokio::sync::mpsc::channel;

use crate::operation::callbacks::Callbacks;

use self::{
    sender::HttpController,
    transport::{HttpError, HttpTransport},
};

use super::{asyncsender::Sender, nextmessage::NextMessage};

pub(crate) mod sender;
pub(crate) mod transport;

pub(crate) struct HttpSender<C> {
    url: url::Url,
    headers: Option<HeaderMap>,
    polling: Duration,
    channel_buffer: usize,

    // transport callbacks
    callbacks: C,
}

impl<C: Callbacks> HttpSender<C> {
    pub(crate) fn new(
        url: url::Url,
        headers: Option<HeaderMap>,
        polling: Duration,
        channel_buffer: usize,
        callbacks: C,
    ) -> Self {
        Self {
            url,
            headers,
            polling,
            channel_buffer,
            callbacks,
        }
    }
}

#[derive(Error, Debug)]
pub enum HttpSenderError {
    #[error("transport error: `{0}`")]
    TransportError(#[from] HttpError),
}

impl<C: Callbacks + Send + Sync + 'static> Sender for HttpSender<C> {
    type Runner = HttpTransport<C>;
    type Controller = HttpController;
    type Error = HttpSenderError;

    fn transport(self) -> Result<(Self::Controller, Self::Runner), HttpSenderError> {
        let next_message = Arc::new(Mutex::new(NextMessage::new()));
        let (sender, receiver) = channel(self.channel_buffer);
        Ok((
            HttpController::new(sender, next_message.clone()),
            HttpTransport::new(
                self.url,
                self.headers,
                self.polling,
                receiver,
                next_message,
                self.callbacks,
            )?,
        ))
    }
}
