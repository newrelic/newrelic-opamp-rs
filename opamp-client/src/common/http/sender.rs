use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::mpsc;

use crate::{
    common::{asyncsender::TransportController, nextmessage::NextMessage},
    opamp::proto::AgentToServer,
};

#[derive(Debug)]
pub struct HttpController {
    // messages to be send
    pub(crate) pending_messages: mpsc::Sender<()>,

    next_message: Arc<Mutex<NextMessage>>,
}

impl HttpController {
    pub(crate) fn new(
        sender_channel: mpsc::Sender<()>,
        next_message: Arc<Mutex<NextMessage>>,
    ) -> Self {
        HttpController {
            pending_messages: sender_channel,
            next_message,
        }
    }
}

#[derive(Error, Debug)]
pub enum HttpControllerError {
    #[error("cannot set instance uid to empty value")]
    EmptyUlid,
    #[error("ulid could not be deserialized: `{0}`")]
    InvalidUlid(ulid::DecodeError),
}

#[async_trait]
impl TransportController for HttpController {
    type Error = HttpControllerError;

    fn update<F>(&mut self, modifier: F)
    where
        F: Fn(&mut AgentToServer),
    {
        // TODO: handle unwrap
        self.next_message.lock().unwrap().update(modifier);
    }

    async fn schedule_send(&mut self) {
        // let msg_to_send = self.next_message.lock().unwrap().pop();
        self.pending_messages.send(()).await.unwrap();
    }

    fn stop(self) {}

    fn set_instance_uid(&mut self, instance_uid: String) -> Result<(), Self::Error> {
        if instance_uid == "" {
            return Err(HttpControllerError::EmptyUlid);
        }

        // just check if is a valid ulid
        let _ = ulid::Ulid::from_string(&instance_uid)
            .map_err(|err| HttpControllerError::InvalidUlid(err))?;

        self.next_message
            .lock()
            .unwrap()
            .update(|msg: &mut AgentToServer| {
                msg.instance_uid = format!("{}", instance_uid);
            });

        Ok(())
    }
}
