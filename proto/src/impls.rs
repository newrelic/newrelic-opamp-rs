use crate::proto::any_value::Value;
use crate::proto::{
    AgentConfigFile, AgentConfigMap, AgentRemoteConfig, AnyValue, ComponentHealth, CustomMessage,
    RemoteConfigStatus,
};
use std::fmt::Debug;

impl ComponentHealth {
    /// Compares two [`ComponentHealth`] structs disregarding the status timestamps,
    /// as we cannot reasonably expect these times to be equal. Start times are compared.
    ///
    /// This needs to be done as the component health information can be "compressed" if it hasn't
    /// changed since the last update.
    pub fn is_same_as(&self, other: &ComponentHealth) -> bool {
        self.healthy == other.healthy
          && self.start_time_unix_nano == other.start_time_unix_nano
          && self.last_error == other.last_error
          && self.status == other.status
          // Inner component health maps are also ComponentHealths, so they have timestamps...
          && (self.component_health_map.len() == other.component_health_map.len() && {
              self.component_health_map.iter().all(|(key, value)| {
                  other
                      .component_health_map
                      .get(key).is_some_and(|other_value| value.is_same_as(other_value))
              })
          })
    }
}

impl Debug for RemoteConfigStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hash = String::from_utf8(self.last_remote_config_hash.clone())
            .unwrap_or(format!("{:?}", &self.last_remote_config_hash));

        write!(
            f,
            r#"RemoteConfigStatus {{ status: {:?}, last_remote_config_hash: "{}", last_error: {:?} }}"#,
            self.status, hash, self.error_message
        )
    }
}

impl Debug for AgentRemoteConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hash = String::from_utf8(self.config_hash.clone())
            .unwrap_or(format!("{:?}", &self.config_hash));
        write!(
            f,
            r#"AgentRemoteConfig {{ config: {:?}, config_hash: "{}" }}"#,
            self.config, hash
        )
    }
}

impl Debug for AgentConfigMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AgentConfigMap {{ config_map: {:?} }}", self.config_map)
    }
}

impl Debug for AgentConfigFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let body = String::from_utf8(self.body.clone()).unwrap_or(format!("{:?}", &self.body));
        write!(
            f,
            r#"AgentConfigFile {{ body: "{}", content_type: {:?} }}"#,
            body, self.content_type
        )
    }
}

impl Debug for CustomMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let data = String::from_utf8(self.data.clone()).unwrap_or(format!("{:?}", &self.data));
        write!(
            f,
            r#"CustomMessage {{ capability: {:?}, type: {:?}, data: "{}" }}"#,
            self.capability, self.r#type, data,
        )
    }
}

impl Debug for AnyValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.value {
            Some(Value::BytesValue(value)) => {
                let data = String::from_utf8(value.clone()).unwrap_or(format!("{:?}", &value));
                write!(f, "{data:?}")
            }
            Some(Value::StringValue(value)) => write!(f, "{value:?}"),
            Some(Value::IntValue(value)) => write!(f, "{value:?}"),
            Some(Value::DoubleValue(value)) => write!(f, "{value:?}"),
            Some(Value::BoolValue(value)) => write!(f, "{value:?}"),
            Some(Value::ArrayValue(value)) => write!(f, "{value:?}"),
            Some(Value::KvlistValue(value)) => write!(f, "{value:?}"),
            None => write!(f, "None"),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::proto::{AgentDescription, AgentToServer, EffectiveConfig, KeyValue, ServerToAgent};

    use super::*;

    #[test]
    fn test_debug_server_to_agent() {
        let expected_string = "ServerToAgent { instance_uid: [0, 1, 2, 3], error_response: None, remote_config: Some(AgentRemoteConfig { config: Some(AgentConfigMap { config_map: {\"test-fleet-list-erich\": AgentConfigFile { body: \"hocus pocus\", content_type: \"text/yaml\" }} }), config_hash: \"5298c05a10e7ca5c91edab6453bb60e8f2848491c15af0f2b2fe8710ad63c34f\" }), connection_settings: None, packages_available: None, flags: 0, capabilities: 0, agent_identification: None, command: None, custom_capabilities: None, custom_message: Some(CustomMessage { capability: \"cap\", type: \"type\", data: \"data\" }) }";
        let sample_message = ServerToAgent {
            instance_uid: vec![0, 1, 2, 3],
            remote_config: Some(AgentRemoteConfig {
                config: Some(AgentConfigMap {
                    config_map: std::collections::HashMap::from([(
                        "test-fleet-list-erich".to_string(),
                        AgentConfigFile {
                            body: "hocus pocus".into(),
                            content_type: "text/yaml".to_string(),
                        },
                    )]),
                }),
                config_hash: "5298c05a10e7ca5c91edab6453bb60e8f2848491c15af0f2b2fe8710ad63c34f"
                    .into(),
            }),
            custom_message: Some(CustomMessage {
                capability: "cap".into(),
                r#type: "type".into(),
                data: "data".into(),
            }),
            ..Default::default()
        };

        eprintln!("{:?}", sample_message);

        assert_eq!(format!("{:?}", sample_message), expected_string);
    }
    #[test]
    fn test_debug_agent_to_server() {
        let expected_string = "AgentToServer { instance_uid: [0, 1, 2, 3], sequence_num: 0, agent_description: Some(AgentDescription { identifying_attributes: [KeyValue { key: \"key\", value: Some(\"value\") }], non_identifying_attributes: [] }), capabilities: 0, health: None, effective_config: Some(EffectiveConfig { config_map: Some(AgentConfigMap { config_map: {\"test-fleet-list-erich\": AgentConfigFile { body: \"hocus pocus\", content_type: \"text/yaml\" }} }) }), remote_config_status: Some(RemoteConfigStatus { status: 0, last_remote_config_hash: \"hash\", last_error: \"error\" }), package_statuses: None, agent_disconnect: None, flags: 0, connection_settings_request: None, custom_capabilities: None, custom_message: None }";
        let sample_message = AgentToServer {
            instance_uid: vec![0, 1, 2, 3],
            effective_config: Some(EffectiveConfig {
                config_map: Some(AgentConfigMap {
                    config_map: std::collections::HashMap::from([(
                        "test-fleet-list-erich".to_string(),
                        AgentConfigFile {
                            body: "hocus pocus".into(),
                            content_type: "text/yaml".to_string(),
                        },
                    )]),
                }),
            }),
            agent_description: Some(AgentDescription {
                non_identifying_attributes: vec![],
                identifying_attributes: vec![KeyValue {
                    key: "key".into(),
                    value: Some(AnyValue {
                        value: Some(Value::BytesValue("value".into())),
                    }),
                }],
            }),
            remote_config_status: Some(RemoteConfigStatus {
                status: 0,
                last_remote_config_hash: "hash".into(),
                error_message: "error".into(),
            }),
            ..Default::default()
        };

        eprintln!("{sample_message:?}");

        assert_eq!(format!("{sample_message:?}"), expected_string);
    }
}
