//! OpAMP client trait and error.

use crate::common::clientstate::SyncedStateError;
use crate::common::message_processor::ProcessError;
use crate::http::HttpClientError;
use crate::opamp::proto::{
    AgentDescription, ComponentHealth, CustomCapabilities, RemoteConfigStatus,
};
use crate::NotStartedClientResult;
use thiserror::Error;

/// Represents various errors that can occur during OpAMP connections.
#[derive(Error, Debug)]
pub enum ClientError {
    /// Indicates a poison error, where a thread panicked while holding a lock.
    #[error("poison error, a thread panicked while holding a lock")]
    PoisonError,
    /// Error to use when the `on_connect_failed` callback has been called with this error type, which would consume its value.
    #[error("connect failed: `{0}`")]
    ConnectFailedCallback(String),
    /// Represents a process message error.
    #[error("`{0}`")]
    ProcessMessageError(#[from] ProcessError),
    /// Represents an HTTP client error.
    #[error("`{0}`")]
    SenderError(#[from] HttpClientError),
    /// Represents a synchronized state error.
    #[error("`{0}`")]
    SyncedStateError(#[from] SyncedStateError),
    /// Indicates that the report effective configuration capability is not set.
    #[error("report effective configuration capability is not set")]
    UnsetEffectConfigCapability,
    /// Indicates that the report remote config status capability is not set.
    #[error("report remote configuration status capability is not set")]
    UnsetRemoteConfigStatusCapability,
    /// Indicates that the report health capability are not set.
    #[error("report health capability is not set")]
    UnsetHealthCapability,
    /// Indicates an error while fetching effective configuration.
    #[error("error while fetching effective config")]
    EffectiveConfigError,
}

/// Represents errors related to the OpAMP started client.
#[derive(Error, Debug)]
pub enum StartedClientError {
    /// Represents a join error.
    #[error("error while joining internal thread")]
    JoinError,
    /// Represents a synchronized state error.
    #[error("`{0}`")]
    ClientError(#[from] ClientError),
}

/// A type alias for results from OpAMP operations.
pub type ClientResult<T> = Result<T, ClientError>;

/// A type alias for results from OpAMP Sync sender operations.
pub type OpampSenderResult<T> = Result<T, HttpClientError>;

/// A type alias for results from the OpAMP started client.
pub type StartedClientResult<T> = Result<T, StartedClientError>;

/// Client defines the communication methods with the Opamp server.
/// It must be shared among threads safely.
pub trait Client: Send + Sync {
    /// set_agent_description sets attributes of the Agent. The attributes will be included
    /// in the next status report sent to the Server.
    fn set_agent_description(&self, description: AgentDescription) -> ClientResult<()>;
    /// get_agent_description returns attributes of the Agent.
    fn get_agent_description(&self) -> ClientResult<AgentDescription>;

    /// set_health sets the health status of the Agent. The ComponentHealth will be included
    fn set_health(&self, health: ComponentHealth) -> ClientResult<()>;

    /// update_effective_config fetches the current local effective config using
    /// get_effective_config callback and sends it to the Server.
    /// The reason why there is a callback to fetch the EffectiveConfig from the Agent and it is not
    /// sent by the Agent like health, is to allow the compression mechanism without storing it.
    fn update_effective_config(&self) -> ClientResult<()>;

    /// set_remote_config_status sets the current RemoteConfigStatus.
    fn set_remote_config_status(&self, status: RemoteConfigStatus) -> ClientResult<()>;

    /// set_custom_capabilities sets the custom capabilities of the Agent.
    fn set_custom_capabilities(&self, custom_capabilities: CustomCapabilities) -> ClientResult<()>;
}

/// A trait defining the methods necessary for managing a client in the OpAMP library.
pub trait NotStartedClient {
    /// The type of the client that is started.
    type StartedClient: StartedClient;
    /// start the client and begin attempts to connect to the Server. Once a connection
    /// is established the client will attempt to maintain it by reconnecting if
    /// the connection is lost. All failed connections attempts will be reported via
    /// OnConnectFailed callback.
    ///
    /// Start may immediately return an error if the settings are incorrect (e.g. the
    /// serverURL is not a valid URL).
    ///
    /// Start does not wait until the connection to the Server is established and will
    /// likely return before the connection attempts are even made.
    ///
    /// It is guaranteed that after the Start() call returns without error one of the
    /// following callbacks will be called eventually (unless Stop() is called earlier):
    ///  - OnConnectFailed
    ///  - OnError
    ///  - OnRemoteConfig
    ///
    ///  Starts should periodically poll status updates from the remote server and apply
    ///  the corresponding updates.
    fn start(self) -> NotStartedClientResult<Self::StartedClient>;
}

/// A trait defining the `stop()` method for stopping a client in the OpAMP library. Implements the `Client` trait.
pub trait StartedClient: Client {
    /// After this call returns successfully it is guaranteed that no
    /// callbacks will be called. stop() will cancel context of any in-fly
    /// callbacks, but will wait until such in-fly callbacks are returned before
    /// Stop returns, so make sure the callbacks don't block infinitely and react
    /// promptly to context cancellations.
    /// Once stopped OpAMPClient cannot be started again.
    fn stop(self) -> StartedClientResult<()>;
}
