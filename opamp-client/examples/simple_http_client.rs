use std::{collections::HashMap, thread::sleep, time::Duration};

use opamp_client::{
    capabilities,
    httpclient::HttpClient,
    opamp::proto::{
        AgentCapabilities, AgentHealth, EffectiveConfig, OpAmpConnectionSettings,
        ServerErrorResponse, ServerToAgentCommand,
    },
    operation::{
        agent::Agent,
        callbacks::{Callbacks, MessageData},
        settings::{AgentDescription, StartSettings},
    },
    OpAMPClient, OpAMPClientHandle,
};
use tracing::info;

use thiserror::Error;
#[derive(Error, Debug)]
pub enum AgentError {
    #[error("`{0}`")]
    Testing(String),
}
struct AgentMock;

impl Agent for AgentMock {
    type Error = AgentError;
    fn get_effective_config(
        &self,
    ) -> Result<opamp_client::opamp::proto::EffectiveConfig, Self::Error> {
        Ok(opamp_client::opamp::proto::EffectiveConfig { config_map: None })
    }
}

struct CallbacksMock;

#[derive(Error, Debug)]
pub(crate) enum CallbacksMockError {}

impl Callbacks for CallbacksMock {
    type Error = CallbacksMockError;
    fn on_error(&self, _err: ServerErrorResponse) {}
    fn on_connect(&self) {
        info!("On connect callback called!")
    }
    fn on_message(&self, _msg: MessageData) {}
    fn on_command(&self, _command: &ServerToAgentCommand) -> Result<(), Self::Error> {
        Ok(())
    }
    fn on_connect_failed(&self, _err: Self::Error) {}
    fn on_opamp_connection_settings(
        &self,
        _settings: &OpAmpConnectionSettings,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    fn on_opamp_connection_settings_accepted(&self, _settings: &OpAmpConnectionSettings) {}
    fn get_effective_config(
        &self,
    ) -> Result<opamp_client::opamp::proto::EffectiveConfig, Self::Error> {
        Ok(EffectiveConfig::default())
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let headers = [("super-key", "super-password")];

    let client = HttpClient::new(
        AgentMock {},
        "https://127.0.0.1/v1/opamp",
        headers,
        StartSettings {
            instance_id: "3Q38XWW0Q98GMAD3NHWZM2PZWZ".to_string(),
            capabilities: capabilities!(AgentCapabilities::ReportsStatus),
            agent_description: AgentDescription {
                identifying_attributes: HashMap::from([
                    ("service.name".to_string(), "com.newrelic.meta_agent".into()),
                    ("service.namespace".to_string(), "newrelic".into()),
                    ("service.version".to_string(), "0.2.0".into()),
                ]),
                non_identifying_attributes: HashMap::from([
                    ("key".to_string(), "val".into()),
                    ("int".to_string(), 5.into()),
                    ("bool".to_string(), true.into()),
                ]),
            },
        },
        CallbacksMock {},
    )
    .unwrap();

    let mut client = client.start().await.unwrap();

    client
        .set_health(&AgentHealth {
            healthy: true,
            start_time_unix_nano: 1689942447,
            last_error: "".to_string(),
        })
        .await
        .unwrap();

    sleep(Duration::from_secs(30));

    client.stop().await.unwrap()
}
