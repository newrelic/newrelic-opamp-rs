use async_trait::async_trait;

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
