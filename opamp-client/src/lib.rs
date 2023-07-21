// public exported traits
pub(crate) mod common;
pub mod operation;

pub mod opamp {
    //! The opamp module contains all those entities defined by the
    //! [Opamp specification](https://github.com/open-telemetry/opamp-spec/blob/main/specification.md)
    pub mod proto {
        //! The proto module contains the protobuffers structures defined by the
        //! [Opamp specification](https://github.com/open-telemetry/opamp-spec/blob/main/specification.md)
        include!(concat!(env!("OUT_DIR"), "/opamp.proto.rs"));
    }
}

pub mod error;
pub mod httpclient;

use async_trait::async_trait;
use opamp::proto::{AgentDescription, AgentHealth, PackageStatuses, RemoteConfigStatus};

#[async_trait]
pub trait OpAMPClient {
    type Handle: OpAMPClientHandle;
    type Error: std::error::Error + Send + Sync;

    /// start the client and begin attempts to connect to the Server. Once connection
    /// is established the client will attempt to maintain it by reconnecting if
    /// the connection is lost. All failed connection attempts will be reported via
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
    async fn start(self) -> Result<Self::Handle, Self::Error>;
}

#[async_trait]
pub trait OpAMPClientHandle {
    type Error: std::error::Error + Send + Sync;

    /// After this call returns successfully it is guaranteed that no
    /// callbacks will be called. stop() will cancel context of any in-fly
    /// callbacks, but will wait until such in-fly callbacks are returned before
    /// Stop returns, so make sure the callbacks don't block infinitely and react
    /// promptly to context cancellations.
    /// Once stopped OpAMPClient cannot be started again.
    async fn stop(self) -> Result<(), Self::Error>;

    /// set_agent_description sets attributes of the Agent. The attributes will be included
    /// in the next status report sent to the Server.
    async fn set_agent_description(
        &mut self,
        description: &AgentDescription,
    ) -> Result<(), Self::Error>;

    // /// agent_description returns the last value successfully set by set_agent_description().
    // fn agent_description(&self) -> &AgentDescription;
    //
    /// set_health sets the health status of the Agent. The AgentHealth will be included
    async fn set_health(&mut self, health: &AgentHealth) -> Result<(), Self::Error>;

    // update_effective_config fetches the current local effective config using
    // get_effective_config callback and sends it to the Server.
    async fn update_effective_config(&mut self) -> Result<(), Self::Error>;

    // /// set_remote_config_status sets the current RemoteConfigStatus.
    // async fn set_remote_config_status(
    //     &mut self,
    //     status: &RemoteConfigStatus,
    // ) -> Result<(), Self::Error>;

    // /// set_package_statuses sets the current PackageStatuses.
    // async fn set_package_statuses(&mut self, statuses: PackageStatuses) -> Result<(), Self::Error>;
}
