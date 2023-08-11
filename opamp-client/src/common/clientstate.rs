use std::sync::{Arc, RwLock};

use crate::{
    opamp::proto::{AgentDescription, AgentHealth, PackageStatuses, RemoteConfigStatus},
    operation::syncedstate::{SyncedState, SyncedStateError},
};

// ClientSyncedState stores the state of the Agent messages that the OpAMP Client needs to
// have access to synchronize to the Server. 4 messages can be stored in this store:
// AgentDescription, AgentHealth, RemoteConfigStatus and PackageStatuses.
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
    health: AgentHealth,
    remote_config_status: RemoteConfigStatus,
    package_statuses: PackageStatuses,
}

impl SyncedState for Arc<ClientSyncedState> {
    fn agent_description(&self) -> Result<AgentDescription, SyncedStateError> {
        Ok(self.data.read()?.agent_description.clone())
    }

    fn set_agent_description(&self, description: AgentDescription) -> Result<(), SyncedStateError> {
        if description.identifying_attributes.is_empty()
            && description.non_identifying_attributes.is_empty()
        {
            return Err(SyncedStateError::AgentDescriptionNoAttributes);
        }

        self.data.write()?.agent_description = description;

        Ok(())
    }

    fn set_health(&self, health: AgentHealth) -> Result<(), SyncedStateError> {
        self.data.write()?.health = health;
        Ok(())
    }

    fn health(&self) -> Result<AgentHealth, SyncedStateError> {
        Ok(self.data.read()?.health.clone())
    }

    fn set_remote_config_status(&self, status: RemoteConfigStatus) -> Result<(), SyncedStateError> {
        self.data.write()?.remote_config_status = status;
        Ok(())
    }

    fn remote_config_status(&self) -> Result<RemoteConfigStatus, SyncedStateError> {
        Ok(self.data.read()?.remote_config_status.clone())
    }

    fn set_package_statuses(&self, status: PackageStatuses) -> Result<(), SyncedStateError> {
        self.data.write()?.package_statuses = status;
        Ok(())
    }

    fn package_statuses(&self) -> Result<PackageStatuses, SyncedStateError> {
        Ok(self.data.read()?.package_statuses.clone())
    }
}
