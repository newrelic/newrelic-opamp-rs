use crate::opamp::proto::AgentToServer;

// NextMessage encapsulates the next message to be sent and provides a
// concurrency-safe interface to work with the message.
#[derive(Debug)]
pub(crate) struct NextMessage {
    // The next message to send.
    next_message: AgentToServer,
}

impl NextMessage {
    pub(crate) fn new() -> Self {
        NextMessage {
            next_message: AgentToServer::default(),
        }
    }

    pub(crate) fn update<F>(&mut self, modifier: F)
    where
        F: Fn(&mut AgentToServer),
    {
        modifier(&mut self.next_message);
    }

    pub(crate) fn pop(&mut self) -> AgentToServer {
        // todo clone AgentToServer and bump sequence_num
        self.next_message.sequence_num += 1;
        self.next_message.clone()
    }
}
