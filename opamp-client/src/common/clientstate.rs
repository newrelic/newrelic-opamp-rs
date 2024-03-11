use std::sync::RwLock;

use crate::opamp::proto::{AgentDescription, ComponentHealth, PackageStatuses, RemoteConfigStatus};

use std::sync::PoisonError;

use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
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

// ClientSyncedState stores the state of the Agent messages that the OpAMP Client needs to
// have access to synchronize to the Server. 4 messages can be stored in this store:
// AgentDescription, ComponentHealth, RemoteConfigStatus and PackageStatuses.
//
// See OpAMP spec for more details on how state synchronization works:
// https://github.com/open-telemetry/opamp-spec/blob/main/specification.md#Agent-to-Server-state-synchronization
//
// Note that the EffectiveConfig is subject to the same synchronization logic, however
// it is not stored in this struct since it can be large, and we do not want to always
// keep it in memory. To avoid storing it in memory the EffectiveConfig is supposed to be
// stored by the Agent implementation (e.g. it can be stored on disk) and is fetched
// via GetEffectiveConfig callback when it is needed by OpAMP client and then it is
// discarded from memory. See implementation of UpdateEffectiveConfig().
//
// It is safe to call methods of this struct concurrently.
#[derive(Debug, Default)]
pub struct ClientSyncedState {
    data: RwLock<Data>,
}

#[derive(Debug, Default)]
struct Data {
    agent_description: AgentDescription,
    health: ComponentHealth,
    remote_config_status: RemoteConfigStatus,
    package_statuses: PackageStatuses,
}

impl ClientSyncedState {
    pub(crate) fn agent_description(&self) -> Result<AgentDescription, SyncedStateError> {
        Ok(self.data.read()?.agent_description.clone())
    }

    pub(crate) fn set_agent_description(
        &self,
        description: AgentDescription,
    ) -> Result<(), SyncedStateError> {
        if description.identifying_attributes.is_empty()
            && description.non_identifying_attributes.is_empty()
        {
            return Err(SyncedStateError::AgentDescriptionNoAttributes);
        }

        self.data.write()?.agent_description = description;

        Ok(())
    }

    pub(crate) fn set_health(&self, health: ComponentHealth) -> Result<(), SyncedStateError> {
        self.data.write()?.health = health;
        Ok(())
    }

    pub(crate) fn health(&self) -> Result<ComponentHealth, SyncedStateError> {
        Ok(self.data.read()?.health.clone())
    }

    #[allow(dead_code)]
    pub(crate) fn set_remote_config_status(
        &self,
        status: RemoteConfigStatus,
    ) -> Result<(), SyncedStateError> {
        self.data.write()?.remote_config_status = status;
        Ok(())
    }

    pub(crate) fn remote_config_status(&self) -> Result<RemoteConfigStatus, SyncedStateError> {
        Ok(self.data.read()?.remote_config_status.clone())
    }

    #[allow(dead_code)]
    pub(crate) fn set_package_statuses(
        &self,
        status: PackageStatuses,
    ) -> Result<(), SyncedStateError> {
        self.data.write()?.package_statuses = status;
        Ok(())
    }

    pub(crate) fn package_statuses(&self) -> Result<PackageStatuses, SyncedStateError> {
        Ok(self.data.read()?.package_statuses.clone())
    }
}
