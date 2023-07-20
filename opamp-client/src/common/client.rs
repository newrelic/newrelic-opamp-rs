use tokio::{
    spawn,
    task::{JoinError, JoinHandle},
};
use tokio_util::sync::CancellationToken;

use crate::{
    opamp::proto::{AgentCapabilities, PackageStatuses},
    operation::agent::Agent,
};

use super::{
    asyncsender::{Sender, TransportError, TransportRunner},
    clientstate::ClientSyncedState,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub(crate) enum ClientError {
    #[error("`{0}`")]
    Transport(#[from] TransportError),

    #[error("`{0}`")]
    Join(#[from] JoinError),
}

// State machine client
// Unstarted only has preparation functions (TODO: change to unstared/started)
pub(crate) struct Unstarted;

// StartedClient contains start and modification functions
pub(crate) struct Started {
    handles: Vec<JoinHandle<Result<(), TransportError>>>,
}

// Client contains the OpAMP logic that is common between WebSocket and
// plain HTTP transports.
#[derive(Debug)]
pub(crate) struct CommonClient<A, S, Stage = Unstarted>
where
    A: Agent,
    S: Sender,
{
    stage: Stage,

    agent: A,

    // Client state storage. This is needed if the Server asks to report the state.
    client_synced_state: ClientSyncedState,

    // Agent's capabilities defined at Start() time.
    capabilities: AgentCapabilities,

    // The transport-specific sender.
    sender: S,

    // Cancellation token
    cancel: CancellationToken,
}

impl<A, S> CommonClient<A, S, Unstarted>
where
    A: Agent,
    S: Sender,
{
    fn new(
        agent: A,
        sender: S,
        client_synced_state: ClientSyncedState,
        capabilities: AgentCapabilities,
    ) -> Self {
        Self {
            stage: Unstarted,
            agent,
            client_synced_state,
            capabilities,
            sender,
            cancel: CancellationToken::new(),
        }
    }

    fn prepare_start(&mut self, _last_statuses: Option<PackageStatuses>) {
        // See: https://github.com/open-telemetry/opamp-go/blob/main/client/internal/clientcommon.go#L70
    }

    fn start_connect_and_run<T>(self, mut runner: T) -> CommonClient<A, S, Started>
    where
        T: TransportRunner + Send + 'static,
    {
        // TODO: Do a sanity check to runner (head request?)
        let handle = spawn({
            let cancel = self.cancel.clone();
            async move { runner.run(cancel).await }
        });

        CommonClient {
            stage: Started {
                handles: vec![handle],
            },
            agent: self.agent,
            client_synced_state: self.client_synced_state,
            capabilities: self.capabilities,
            sender: self.sender,
            cancel: self.cancel,
        }
    }
}

impl<A, S> CommonClient<A, S, Started>
where
    A: Agent,
    S: Sender,
{
    async fn stop(self) -> Result<(), ClientError> {
        self.cancel.cancel();
        for handle in self.stage.handles {
            handle.await??;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{common::asyncsender::test::new_sender_mocks, operation::agent::test::AgentMock};

    #[tokio::test]
    async fn start_stop() {
        let (transport, sender) = new_sender_mocks();

        let client = CommonClient::new(
            AgentMock,
            sender,
            ClientSyncedState::default(),
            AgentCapabilities::ReportsStatus,
        );
        assert!(client.start_connect_and_run(transport).stop().await.is_ok())
    }
}
