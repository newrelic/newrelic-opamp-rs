use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::opamp::proto::AgentToServer;

// Sender is an interface of the sending portion of OpAMP protocol that stores
// the NextMessage to be sent and can be ordered to send the message.
#[async_trait]
pub(crate) trait Sender {
    type Error: std::error::Error + Send + Sync;

    fn update<F>(&mut self, modifier: F)
    where
        F: Fn(&mut AgentToServer);

    // next_message gives access to the next message that will be sent by this Sender.
    // Can be called concurrently with any other method.
    // fn next_message(&self) -> Option<NextMessage>;

    // schedule_send signals to Sender that the message in NextMessage struct
    // is now ready to be sent.  The Sender should send the NextMessage as soon as possible.
    // If there is no pending message (e.g. the NextMessage was already sent and
    // "pending" flag is reset) then no message will be sent.
    async fn schedule_send(&mut self);

    // set_instance_uid sets a new instanceUid to be used for all subsequent messages to be sent.
    fn set_instance_uid(&mut self, instance_uid: String) -> Result<(), Self::Error>;
}

#[async_trait]
pub(crate) trait TransportRunner {
    // run internal networking transport until canceled.
    async fn run(&mut self, cancel: CancellationToken) -> Result<(), TransportError>;
}

use thiserror::Error;

#[derive(Error, Debug)]
pub(crate) enum TransportError {
    // TODO: fix
    #[error("some error")]
    Invalid,
}

#[cfg(test)]
pub(crate) mod test {

    use crate::opamp::proto;

    use super::{Sender, TransportError, TransportRunner};
    use async_trait::async_trait;
    use thiserror::Error;
    use tokio::select;
    use tokio_util::sync::CancellationToken;

    #[derive(Error, Debug)]
    pub(crate) enum SenderError {}

    pub(crate) struct SenderMock;
    pub(crate) struct TransportMock;

    pub(crate) fn new_sender_mocks() -> (TransportMock, SenderMock) {
        (TransportMock, SenderMock)
    }

    #[async_trait]
    impl Sender for SenderMock {
        type Error = SenderError;
        fn update<F>(&mut self, _modifier: F)
        where
            F: Fn(&mut proto::AgentToServer),
        {
        }

        async fn schedule_send(&mut self) {}

        fn set_instance_uid(&mut self, _instance_uid: String) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    #[async_trait]
    impl TransportRunner for TransportMock {
        async fn run(&mut self, cancel: CancellationToken) -> Result<(), TransportError> {
            select! {
                _ = cancel.cancelled() => {
                    // The token was cancelled
                    Ok(())
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(600)) => {
                    Err(TransportError::Invalid)
                }
            }
        }
    }
}
