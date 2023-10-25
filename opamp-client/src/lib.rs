//! OpAMP client library.

#![warn(missing_docs)]

// public exported traits
pub(crate) mod common;
pub mod operation;

// OpAMP protobuffers module files will be excluded from documentation.
#[doc(hidden)]
pub mod opamp {
    //! The opamp module contains all those entities defined by the
    //! [Opamp specification](https://github.com/open-telemetry/opamp-spec/blob/main/specification.md)
    pub mod proto {
        //! The proto module contains the protobuffers structures defined by the
        //! [Opamp specification](https://github.com/open-telemetry/opamp-spec/blob/main/specification.md)
        include!(concat!("../../proto", "/opamp.proto.rs"));
    }
}

pub mod error;

#[cfg(feature = "async-http")]
pub mod http;

use crate::opamp::proto::RemoteConfigStatus;
use async_trait::async_trait;
use error::{ClientResult, NotStartedClientResult, StartedClientResult};
use opamp::proto::{AgentDescription, AgentHealth};

#[async_trait]
/// Client defines the communication methods with the Opamp server.
/// It must be shared among threads safely.
pub trait Client: Send + Sync {
    /// set_agent_description sets attributes of the Agent. The attributes will be included
    /// in the next status report sent to the Server.
    async fn set_agent_description(&self, description: AgentDescription) -> ClientResult<()>;

    /// set_health sets the health status of the Agent. The AgentHealth will be included
    async fn set_health(&self, health: AgentHealth) -> ClientResult<()>;

    /// update_effective_config fetches the current local effective config using
    /// get_effective_config callback and sends it to the Server.
    async fn update_effective_config(&self) -> ClientResult<()>;

    /// set_remote_config_status sets the current RemoteConfigStatus.
    async fn set_remote_config_status(&self, status: RemoteConfigStatus) -> ClientResult<()>;

    // /// set_package_statuses sets the current PackageStatuses.
    // async fn set_package_statuses(&mut self, statuses: PackageStatuses) -> Result<(), Self::Error>;
}

/// A trait defining the methods necessary for managing a client in the OpAMP library.
#[async_trait]
pub trait NotStartedClient {
    /// Associated type for this UnstartedClient which implements the StartedClient trait.
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
    async fn start(self) -> NotStartedClientResult<Self::StartedClient>;
}

/// A trait defining the `stop()` method for stopping a client in the OpAMP library. Implements the `Client` trait.
#[async_trait]
pub trait StartedClient: Client {
    /// After this call returns successfully it is guaranteed that no
    /// callbacks will be called. stop() will cancel context of any in-fly
    /// callbacks, but will wait until such in-fly callbacks are returned before
    /// Stop returns, so make sure the callbacks don't block infinitely and react
    /// promptly to context cancellations.
    /// Once stopped OpAMPClient cannot be started again.
    async fn stop(self) -> StartedClientResult<()>;
}
