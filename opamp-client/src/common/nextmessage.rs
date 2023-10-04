use crate::opamp::proto::AgentToServer;

/// A structure that encapsulates the next message to be sent.
#[derive(Debug, Default)]
pub(crate) struct NextMessage {
    // The next message to send.
    message: AgentToServer,
}

// Implementation block for NextMessage struct.
impl NextMessage {
    /// Creates a new NextMessage with the given initial message.
    ///
    /// # Arguments
    ///
    /// * `init` - An instance of AgentToServer to be used as the initial message.
    ///
    /// # Returns
    ///
    /// A new instance of NextMessage.
    pub(crate) fn new(init: AgentToServer) -> Self {
        NextMessage { message: init }
    }

    /// Updates the current message with a modifier function.
    ///
    /// # Arguments
    ///
    /// * `modifier` - A closure that accepts a mutable reference to AgentToServer and modifies its state.
    pub(crate) fn update<F>(&mut self, modifier: F)
    where
        F: FnOnce(&mut AgentToServer),
    {
        modifier(&mut self.message);
    }

    /// Increments the sequence number and returns the current message.
    ///
    /// # Returns
    ///
    /// A clone of the current message with its sequence number incremented.
    pub(crate) fn pop(&mut self) -> AgentToServer {
        // todo clone AgentToServer and bump sequence_num
        self.message.sequence_num += 1;
        self.message.clone()
    }
}
