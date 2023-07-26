use tokio::{
    spawn,
    task::{JoinError, JoinHandle},
};

use crate::{
    opamp::proto::{AgentCapabilities, AgentDescription, AgentHealth},
    operation::{agent::Agent, settings::StartSettings},
};

use super::{
    clientstate::ClientSyncedState,
    transport::{TransportController, TransportError, TransportRunner},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CommonClientError {
    #[error("`{0}`")]
    Transport(#[from] TransportError),

    #[error("`{0}`")]
    Join(#[from] JoinError),

    #[error("capabilities error: `{0}`")]
    UnsetCapabilities(String),

    #[error("get effective config error: `{0}`")]
    GetConfig(String),

    #[error("error setting ulid: `{0}`")]
    InvalidUlid(String),
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
pub(crate) struct CommonClient<A, C, R, Stage = Unstarted>
where
    A: Agent,
    C: TransportController,
    R: TransportRunner + Send + 'static,
{
    stage: Stage,

    agent: A,

    // Client state storage. This is needed if the Server asks to report the state.
    client_synced_state: ClientSyncedState,

    // Agent's capabilities defined at Start() time.
    capabilities: AgentCapabilities,

    // The transport-specific sender.
    sender: C,

    // The transport-specific runner.
    runner: Option<R>,
}

impl<A, C, R> CommonClient<A, C, R, Unstarted>
where
    A: Agent,
    C: TransportController,
    R: TransportRunner + Send + 'static,
{
    // TODO: align with upstream
    // See: https://github.com/open-telemetry/opamp-go/blob/main/client/internal/clientcommon.go#L70
    pub(crate) fn new(
        agent: A,
        client_synced_state: ClientSyncedState,
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
        client
            .sender
            .set_instance_uid(start_settings.instance_id)
            .map_err(|err| CommonClientError::InvalidUlid(err.to_string()))?;
        Ok(client)
    }

    pub(crate) fn start_connect_and_run(mut self) -> CommonClient<A, C, R, Started> {
        let mut runner = self.runner.take().unwrap();
        // TODO: Do a sanity check to runner (head request?)
        let handle = spawn(async move { runner.run().await });

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

impl<A, C, R> CommonClient<A, C, R, Started>
where
    A: Agent,
    C: TransportController,
    R: TransportRunner + Send + 'static,
{
    pub(crate) async fn stop(self) -> Result<(), CommonClientError> {
        // TODO: handle Option unwrap
        self.sender.stop();
        for handle in self.stage.handles {
            handle.await??;
        }
        Ok(())
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
        });

        self.sender.schedule_send().await;
        Ok(())
    }

    pub(crate) async fn set_health(
        &mut self,
        health: &AgentHealth,
    ) -> Result<(), CommonClientError> {
        // update next message with provided description
        self.sender.update(|msg| {
            msg.health = Some(health.clone());
        });

        self.sender.schedule_send().await;
        Ok(())
    }

    // update_effective_config fetches the current local effective config using
    // get_effective_config callback and sends it to the Server using provided Sender.
    pub(crate) async fn update_effective_config(&mut self) -> Result<(), CommonClientError> {
        if self.capabilities as u64 & AgentCapabilities::ReportsRemoteConfig as u64 == 0 {
            return Err(CommonClientError::UnsetCapabilities(
                "report remote configuration capabilities is not set".into(),
            ));
        }

        let config = self
            .agent
            .get_effective_config()
            .map_err(|err| CommonClientError::GetConfig(err.to_string()))?;

        // update next message with effective config
        self.sender.update(|msg| {
            msg.effective_config = Some(config.clone());
        });

        self.sender.schedule_send().await;
        Ok(())
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::{
        common::transport::{test::SenderMock, Sender},
        operation::agent::test::AgentMock,
    };

    #[tokio::test]
    async fn start_stop() {
        let sender = SenderMock {};
        let (controller, runner) = sender.transport().unwrap();

        let client = CommonClient::new(
            AgentMock,
            ClientSyncedState::default(),
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
}
