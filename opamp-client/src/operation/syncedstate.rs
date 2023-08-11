use std::sync::PoisonError;

use thiserror::Error;

use crate::opamp::proto::{AgentDescription, AgentHealth, PackageStatuses, RemoteConfigStatus};

#[derive(Debug, Error)]
pub enum SyncedStateError {
    #[error("agent description must contain attributes")]
    AgentDescriptionNoAttributes,

    #[error("poison error, a thread panicked while holding a lock")]
    PoisonError,
}

impl<T> From<PoisonError<T>> for SyncedStateError {
    fn from(_value: PoisonError<T>) -> Self {
        SyncedStateError::PoisonError
    }
}

pub trait SyncedState {
    fn agent_description(&self) -> Result<AgentDescription, SyncedStateError>;

    // set_agent_description sets the AgentDescription in the state.
    fn set_agent_description(&self, description: AgentDescription) -> Result<(), SyncedStateError>;

    fn set_health(&self, health: AgentHealth) -> Result<(), SyncedStateError>;

    fn health(&self) -> Result<AgentHealth, SyncedStateError>;

    fn set_remote_config_status(&self, status: RemoteConfigStatus) -> Result<(), SyncedStateError>;

    fn remote_config_status(&self) -> Result<RemoteConfigStatus, SyncedStateError>;

    fn set_package_statuses(&self, status: PackageStatuses) -> Result<(), SyncedStateError>;

    fn package_statuses(&self) -> Result<PackageStatuses, SyncedStateError>;
}
