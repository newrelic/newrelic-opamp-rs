use std::sync::PoisonError;

use crate::{
    opamp::proto::AgentToServer,
    operation::{capabilities::Capabilities, syncedstate::SyncedState},
};
use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;

// Sender is an interface of the sending portion of OpAMP protocol that stores
// the NextMessage to be sent and can be ordered to send the message.
#[async_trait]
pub(crate) trait Sender {
    type Controller: TransportController;
    type Runner: TransportRunner + Send + 'static;
    type Error: std::error::Error + Send + Sync;

    fn transport(self) -> Result<(Self::Controller, Self::Runner), Self::Error>;
}

#[derive(Error, Debug)]
pub enum TransportError {
    #[error("some error")]
    Invalid,
    #[error("cannot set instance uid to empty value")]
    EmptyUlid,
    #[error("ulid could not be deserialized: `{0}`")]
    InvalidUlid(ulid::DecodeError),
    #[error("`{0}`")]
    SendError(#[from] SendError<()>),
    #[error("poison error, a thread panicked while holding a lock")]
    PoisonError,
}

impl<T> From<PoisonError<T>> for TransportError {
    fn from(_value: PoisonError<T>) -> Self {
        TransportError::PoisonError
    }
}

// Sender is an interface of the sending portion of OpAMP protocol that stores
// the NextMessage to be sent and can be ordered to send the message.
#[async_trait]
pub trait TransportController {
    fn update<F>(&mut self, modifier: F) -> Result<(), TransportError>
    where
        F: Fn(&mut AgentToServer);

    // schedule_send signals to Sender that the message in NextMessage struct
    // is now ready to be sent.  The Sender should send the NextMessage as soon as possible.
    // If there is no pending message (e.g. the NextMessage was already sent and
    // "pending" flag is reset) then no message will be sent.
    async fn schedule_send(&mut self) -> Result<(), TransportError>;

    // set_instance_uid sets a new instanceUid to be used for all subsequent messages to be sent.
    fn set_instance_uid(&mut self, instance_uid: String) -> Result<(), TransportError>;

    // stop cancels the transport runner
    fn stop(self);
}

// TODO: Change to Sender?
#[async_trait]
pub trait TransportRunner {
    type State: SyncedState;
    // run internal networking transport until canceled.
    async fn run(
        &mut self,
        state: Self::State,
        capabilities: Capabilities,
    ) -> Result<(), TransportError>;
}
