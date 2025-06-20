use std::sync::RwLock;

use crate::opamp::proto::{
    AgentDescription, ComponentHealth, CustomCapabilities, PackageStatuses, RemoteConfigStatus,
};

use std::sync::PoisonError;

use thiserror::Error;

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
// via get_effective_config callback when it is needed by OpAMP client, and then it is
// discarded from memory. See implementation of update_effective_config().
//
// It is safe to call methods of this struct concurrently.
#[derive(Debug, Default)]
pub struct ClientSyncedState {
    data: RwLock<Data>,
}

#[derive(Debug, Default)]
struct Data {
    agent_description: Option<AgentDescription>,
    custom_capabilities: Option<CustomCapabilities>,
    health: Option<ComponentHealth>,
    remote_config_status: Option<RemoteConfigStatus>,
    package_statuses: Option<PackageStatuses>,
}

impl ClientSyncedState {
    pub(crate) fn agent_description(&self) -> Result<Option<AgentDescription>, SyncedStateError> {
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

        self.data.write()?.agent_description = Some(description);
        Ok(())
    }

    pub(crate) fn agent_description_unchanged(
        &self,
        description: &AgentDescription,
    ) -> Result<bool, SyncedStateError> {
        if let Some(synced_description) = self.agent_description()? {
            return Ok(synced_description.eq(description));
        }
        Ok(false)
    }

    pub(crate) fn set_health(&self, health: ComponentHealth) -> Result<(), SyncedStateError> {
        self.data.write()?.health = Some(health);
        Ok(())
    }

    pub(crate) fn health(&self) -> Result<Option<ComponentHealth>, SyncedStateError> {
        Ok(self.data.read()?.health.clone())
    }

    pub(crate) fn health_unchanged(
        &self,
        health: &ComponentHealth,
    ) -> Result<bool, SyncedStateError> {
        if let Some(synced_health) = self.health()? {
            return Ok(synced_health.is_same_as(health));
        }
        Ok(false)
    }

    #[allow(dead_code)]
    pub(crate) fn set_remote_config_status(
        &self,
        status: RemoteConfigStatus,
    ) -> Result<(), SyncedStateError> {
        self.data.write()?.remote_config_status = Some(status);
        Ok(())
    }

    pub(crate) fn remote_config_status(
        &self,
    ) -> Result<Option<RemoteConfigStatus>, SyncedStateError> {
        Ok(self.data.read()?.remote_config_status.clone())
    }

    pub(crate) fn remote_config_status_unchanged(
        &self,
        status: &RemoteConfigStatus,
    ) -> Result<bool, SyncedStateError> {
        if let Some(synced_remote_config_status) = self.remote_config_status()? {
            return Ok(synced_remote_config_status.eq(status));
        }
        Ok(false)
    }

    #[allow(dead_code)]
    pub(crate) fn set_package_statuses(
        &self,
        status: PackageStatuses,
    ) -> Result<(), SyncedStateError> {
        self.data.write()?.package_statuses = Some(status);
        Ok(())
    }

    pub(crate) fn package_statuses(&self) -> Result<Option<PackageStatuses>, SyncedStateError> {
        Ok(self.data.read()?.package_statuses.clone())
    }

    pub(crate) fn custom_capabilities(
        &self,
    ) -> Result<Option<CustomCapabilities>, SyncedStateError> {
        Ok(self.data.read()?.custom_capabilities.clone())
    }
    pub(crate) fn custom_capabilities_unchanged(
        &self,
        custom_capabilities: &CustomCapabilities,
    ) -> Result<bool, SyncedStateError> {
        if let Some(synced_custom_capabilities) = self.custom_capabilities()? {
            return Ok(synced_custom_capabilities.eq(custom_capabilities));
        }
        Ok(false)
    }
    pub(crate) fn set_custom_capabilities(
        &self,
        custom_capabilities: CustomCapabilities,
    ) -> Result<(), SyncedStateError> {
        self.data.write()?.custom_capabilities = Some(custom_capabilities);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opamp::proto::any_value::Value;
    use crate::opamp::proto::{AnyValue, KeyValue};

    #[test]
    fn agent_description_unchanged() {
        let expected_agent_description = AgentDescription {
            identifying_attributes: vec![KeyValue {
                key: "thing".to_string(),
                value: Some(AnyValue {
                    value: Some(Value::StringValue("thing_value".to_string())),
                }),
            }],
            non_identifying_attributes: vec![],
        };

        let synced_state = ClientSyncedState::default();
        assert!(
            synced_state
                .set_agent_description(expected_agent_description.clone())
                .is_ok()
        );
        assert!(
            synced_state
                .agent_description_unchanged(&expected_agent_description)
                .unwrap()
        );
    }

    #[test]
    fn health_unchanged() {
        let expected_health = ComponentHealth {
            healthy: true,
            last_error: String::new(),
            ..Default::default()
        };

        let synced_state = ClientSyncedState::default();
        assert!(synced_state.set_health(expected_health.clone()).is_ok());
        assert!(synced_state.health_unchanged(&expected_health).unwrap());
    }

    #[test]
    fn remote_config_status_unchanged() {
        let expected_remote_config_status = RemoteConfigStatus {
            last_remote_config_hash: vec![],
            status: 2,
            error_message: String::new(),
        };

        let synced_state = ClientSyncedState::default();
        assert!(
            synced_state
                .set_remote_config_status(expected_remote_config_status.clone())
                .is_ok()
        );
        assert!(
            synced_state
                .remote_config_status_unchanged(&expected_remote_config_status)
                .unwrap()
        );
    }

    #[test]
    fn compare_health() {
        let health1 = ComponentHealth {
            healthy: true,
            status_time_unix_nano: 1,
            last_error: String::new(),
            ..Default::default()
        };

        let health2 = ComponentHealth {
            healthy: true,
            status_time_unix_nano: 2,
            last_error: String::new(),
            ..Default::default()
        };

        assert!(health1.is_same_as(&health2));
    }

    #[test]
    fn compare_health_with_inner_component_health() {
        let health1 = ComponentHealth {
            healthy: true,
            status_time_unix_nano: 1,
            last_error: String::new(),
            component_health_map: vec![(
                "inner".to_string(),
                ComponentHealth {
                    healthy: true,
                    status_time_unix_nano: 1,
                    last_error: String::new(),
                    ..Default::default()
                },
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        };

        let health2 = ComponentHealth {
            healthy: true,
            status_time_unix_nano: 2,
            last_error: String::new(),
            component_health_map: vec![(
                "inner".to_string(),
                ComponentHealth {
                    healthy: true,
                    status_time_unix_nano: 2,
                    last_error: String::new(),
                    ..Default::default()
                },
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        };

        assert!(health1.is_same_as(&health2));
    }

    #[test]
    fn custom_capabilities() {
        let custom_capabilities = CustomCapabilities {
            capabilities: vec!["foo".to_string(), "bar".to_string()],
        };

        let synced_state = ClientSyncedState::default();

        assert_eq!(synced_state.custom_capabilities().unwrap(), None);

        synced_state
            .set_custom_capabilities(custom_capabilities.clone())
            .unwrap();

        assert!(
            synced_state
                .custom_capabilities_unchanged(&custom_capabilities)
                .unwrap()
        );
    }
}
