use tokio::{
    spawn,
    task::{JoinError, JoinHandle},
};

use crate::{
    opamp::proto::{AgentCapabilities, AgentDescription, AgentHealth},
    operation::{
        agent::Agent,
        settings::StartSettings,
        syncedstate::{SyncedState, SyncedStateError},
    },
};

use super::transport::{TransportController, TransportError, TransportRunner};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CommonClientError {
    #[error("`{0}`")]
    Transport(#[from] TransportError),

    #[error("`{0}`")]
    SyncState(#[from] SyncedStateError),

    #[error("`{0}`")]
    Join(#[from] JoinError),

    #[error("report remote configuration capabilities is not set")]
    UnsetRemoteCapabilities,

    #[error("get effective config error: `{0}`")]
    GetConfig(String),
}

// State machine client
// Unstarted only has preparation functions (TODO: change to unstared/started)
pub struct Unstarted;

// StartedClient contains start and modification functions
pub struct Started {
    handles: Vec<JoinHandle<Result<(), TransportError>>>,
}

// Client contains the OpAMP logic that is common between WebSocket and
// plain HTTP transports.
#[derive(Debug)]
pub(crate) struct CommonClient<A, C, R, S, Stage = Unstarted>
where
    A: Agent,
    C: TransportController,
    R: TransportRunner + Send + 'static,
    S: SyncedState,
{
    stage: Stage,

    agent: A,

    // Client state storage. This is needed if the Server asks to report the state.
    client_synced_state: S,

    // Agent's capabilities defined at Start() time.
    capabilities: AgentCapabilities,

    // The transport-specific sender.
    sender: C,

    // The transport-specific runner.
    runner: Option<R>,
}

impl<A, C, R, S> CommonClient<A, C, R, S, Unstarted>
where
    A: Agent,
    C: TransportController,
    R: TransportRunner<State = S> + Send + 'static,
    S: SyncedState + Clone + Send + 'static,
{
    // TODO: align with upstream
    // See: https://github.com/open-telemetry/opamp-go/blob/main/client/internal/clientcommon.go#L70
    pub(crate) fn new(
        agent: A,
        client_synced_state: S,
        start_settings: StartSettings,
        sender: C,
        runner: R,
    ) -> Result<Self, CommonClientError> {
        let mut client = Self {
            stage: Unstarted,
            agent,
            client_synced_state,
            capabilities: start_settings.capabilities,
            sender,
            runner: Some(runner),
        };
        client.sender.set_instance_uid(start_settings.instance_id)?;
        Ok(client)
    }

    pub(crate) fn start_connect_and_run(mut self) -> CommonClient<A, C, R, S, Started> {
        let mut runner = self.runner.take().unwrap();
        let state = self.client_synced_state.clone();
        // TODO: Do a sanity check to runner (head request?)
        let handle = spawn(async move { runner.run(state).await });

        CommonClient {
            stage: Started {
                handles: vec![handle],
            },
            agent: self.agent,
            client_synced_state: self.client_synced_state,
            capabilities: self.capabilities,
            sender: self.sender,
            runner: None,
        }
    }
}

impl<A, C, R, S> CommonClient<A, C, R, S, Started>
where
    A: Agent,
    C: TransportController,
    R: TransportRunner + Send + 'static,
    S: SyncedState,
{
    pub(crate) async fn stop(self) -> Result<(), CommonClientError> {
        // TODO: handle Option unwrap
        self.sender.stop();
        for handle in self.stage.handles {
            handle.await??;
        }
        Ok(())
    }

    // agent_description returns the current state of the AgentDescription.
    pub(crate) fn agent_description(&self) -> Result<AgentDescription, SyncedStateError> {
        self.client_synced_state.agent_description()
    }

    // set_agent_description sends a status update to the Server with the new AgentDescription
    // and remembers the AgentDescription in the client state so that it can be sent
    // to the Server when the Server asks for it.
    pub(crate) async fn set_agent_description(
        &mut self,
        description: &AgentDescription,
    ) -> Result<(), CommonClientError> {
        // update next message with provided description
        self.sender.update(|msg| {
            msg.agent_description = Some(description.clone());
        })?;

        Ok(self.sender.schedule_send().await?)
    }

    // set_health sends a status update to the Server with the new AgentHealth
    // and remembers the AgentHealth in the client state so that it can be sent
    // to the Server when the Server asks for it.
    pub(crate) async fn set_health(
        &mut self,
        health: &AgentHealth,
    ) -> Result<(), CommonClientError> {
        // store the AgentHealth to send on reconnect
        self.client_synced_state.set_health(health.clone())?;

        // update next message with provided description
        self.sender.update(|msg| {
            msg.health = Some(health.clone());
        })?;

        Ok(self.sender.schedule_send().await?)
    }

    // update_effective_config fetches the current local effective config using
    // get_effective_config callback and sends it to the Server using provided Sender.
    pub(crate) async fn update_effective_config(&mut self) -> Result<(), CommonClientError> {
        if self.capabilities as u64 & AgentCapabilities::ReportsRemoteConfig as u64 == 0 {
            return Err(CommonClientError::UnsetRemoteCapabilities);
        }

        let config = self
            .agent
            .get_effective_config()
            .map_err(|err| CommonClientError::GetConfig(err.to_string()))?;

        // update next message with effective config
        self.sender.update(|msg| {
            msg.effective_config = Some(config.clone());
        })?;

        Ok(self.sender.schedule_send().await?)
    }
}

#[cfg(test)]
mod test {

    use std::sync::{atomic::AtomicU8, Arc};

    use async_trait::async_trait;
    use tokio::{
        select,
        sync::mpsc::{channel, Receiver, Sender},
    };

    use super::*;

    use crate::{
        common::clientstate::ClientSyncedState,
        opamp::proto::{AgentToServer, KeyValue},
        operation::agent::test::AgentMock,
    };

    struct TControllerMock {
        notifier: Sender<()>,
        counter: Arc<AtomicU8>,
    }

    #[async_trait]
    impl TransportController for TControllerMock {
        fn update<F>(&mut self, _modifier: F) -> Result<(), TransportError>
        where
            F: Fn(&mut AgentToServer),
        {
            Ok(())
        }

        async fn schedule_send(&mut self) -> Result<(), TransportError> {
            self.counter
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        }

        fn set_instance_uid(&mut self, _instance_uid: String) -> Result<(), TransportError> {
            Ok(())
        }

        fn stop(self) {
            drop(self.notifier)
        }
    }

    struct TRunnerMock {
        cancel: Receiver<()>,
    }

    #[async_trait]
    impl TransportRunner for TRunnerMock {
        type State = Arc<ClientSyncedState>;
        async fn run(&mut self, _state: Self::State) -> Result<(), TransportError> {
            loop {
                select! {
                    result = self.cancel.recv() => match result {
                        Some(()) => (),
                        None => break,
                    }
                }
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn start_stop() {
        let (sender, receiver) = channel(1);
        let (controller, runner) = (
            TControllerMock {
                notifier: sender,
                counter: Arc::new(0.into()),
            },
            TRunnerMock { cancel: receiver },
        );

        let client = CommonClient::new(
            AgentMock,
            Arc::new(ClientSyncedState::default()),
            StartSettings {
                instance_id: "3Q38XWW0Q98GMAD3NHWZM2PZWZ".to_string(),
                capabilities: AgentCapabilities::ReportsStatus,
            },
            controller,
            runner,
        )
        .unwrap();

        assert!(client.start_connect_and_run().stop().await.is_ok())
    }

    #[tokio::test]
    async fn modify_client_state() {
        let (sender, receiver) = channel(1);
        let (controller, runner) = (
            TControllerMock {
                notifier: sender,
                counter: Arc::new(0.into()),
            },
            TRunnerMock { cancel: receiver },
        );

        let shared_state = Arc::new(ClientSyncedState::default());

        let client = CommonClient::new(
            AgentMock,
            shared_state.clone(),
            StartSettings {
                instance_id: "3Q38XWW0Q98GMAD3NHWZM2PZWZ".to_string(),
                capabilities: AgentCapabilities::ReportsStatus,
            },
            controller,
            runner,
        )
        .unwrap();

        let client = client.start_connect_and_run();

        let new_description = AgentDescription {
            identifying_attributes: vec![KeyValue {
                key: "test".to_string(),
                value: None,
            }],
            non_identifying_attributes: vec![],
        };

        // set custom description using shared state reference
        shared_state
            .set_agent_description(new_description.clone())
            .unwrap();

        // retrive current agent description
        assert_eq!(client.agent_description().unwrap(), new_description);
        assert!(client.stop().await.is_ok())
    }

    #[tokio::test]
    async fn update_effective_config() {
        let (sender, receiver) = channel(1);
        let counter = Arc::new(AtomicU8::new(0));

        let (controller, runner) = (
            TControllerMock {
                notifier: sender,
                counter: counter.clone(),
            },
            TRunnerMock { cancel: receiver },
        );

        let shared_state = Arc::new(ClientSyncedState::default());

        let client = CommonClient::new(
            AgentMock,
            shared_state.clone(),
            StartSettings {
                instance_id: "3Q38XWW0Q98GMAD3NHWZM2PZWZ".to_string(),
                capabilities: AgentCapabilities::ReportsStatus,
            },
            controller,
            runner,
        )
        .unwrap();

        let mut client = client.start_connect_and_run();

        assert!(client.update_effective_config().await.is_err());
        assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), 0);

        // set needed capabilities
        client.capabilities = AgentCapabilities::ReportsRemoteConfig;

        assert!(client.update_effective_config().await.is_ok());
        assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), 1);
    }
}
