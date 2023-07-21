use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use http::HeaderMap;
use tokio::sync::mpsc::channel;

use self::{sender::HttpController, transport::HttpTransport};

use super::{asyncsender::Sender, nextmessage::NextMessage};

pub(crate) mod sender;
pub(crate) mod transport;

pub(crate) struct HttpSender {
    url: url::Url,
    headers: Option<HeaderMap>,
    polling: Duration,
    channel_buffer: usize,
}

impl HttpSender {
    pub(crate) fn new(
        url: url::Url,
        headers: Option<HeaderMap>,
        polling: Duration,
        channel_buffer: usize,
    ) -> Self {
        Self {
            url,
            headers,
            polling,
            channel_buffer,
        }
    }
}

impl Sender for HttpSender {
    type Runner = HttpTransport;
    type Controller = HttpController;

    fn transport(self) -> (Self::Controller, Self::Runner) {
        let next_message = Arc::new(Mutex::new(NextMessage::new()));
        let (sender, receiver) = channel(self.channel_buffer);
        (
            HttpController::new(sender, next_message.clone()),
            HttpTransport::new(self.url, self.headers, self.polling, receiver, next_message),
        )
    }
}
