use crate::opamp::proto::EffectiveConfig;

pub trait Agent {
    type Error: std::error::Error + Send + Sync;

    // get_effective_config returns the current effective config. Only one
    // get_effective_config  call can be active at any time. Until get_effective_config
    // returns it will not be called again.
    fn get_effective_config(&self) -> Result<EffectiveConfig, Self::Error>;
}

pub(crate) mod test {
    use crate::opamp::proto::EffectiveConfig;

    use super::Agent;

    pub(crate) struct AgentMock;

    use thiserror::Error;

    #[derive(Error, Debug)]
    pub(crate) enum AgentError {}

    impl Agent for AgentMock {
        type Error = AgentError;
        fn get_effective_config(&self) -> Result<EffectiveConfig, Self::Error> {
            Ok(EffectiveConfig::default())
        }
    }
}
