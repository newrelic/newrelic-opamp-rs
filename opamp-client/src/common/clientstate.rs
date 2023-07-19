use crate::opamp::proto::{AgentDescription, AgentHealth, PackageStatuses, RemoteConfigStatus};

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
#[derive(Debug)]
pub(crate) struct ClientSyncedState {
    agent_description: AgentDescription,
    healt: AgentHealth,
    remote_config_status: RemoteConfigStatus,
    package_status: PackageStatuses,
}

impl Default for ClientSyncedState {
    fn default() -> Self {
        ClientSyncedState {
            agent_description: AgentDescription::default(),
            healt: AgentHealth::default(),
            remote_config_status: RemoteConfigStatus::default(),
            package_status: PackageStatuses::default(),
        }
    }
}
