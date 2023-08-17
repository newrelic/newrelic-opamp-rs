use super::capabilities::Capabilities;

// StartSettings defines the parameters for starting the OpAMP Client.
#[derive(Debug)]
pub struct StartSettings {
    // Agent information.
    pub instance_id: String,

    // Defines the capabilities of the Agent. AgentCapabilities_ReportsStatus bit does not need to
    // be set in this field, it will be set automatically since it is required by OpAMP protocol.
    pub capabilities: Capabilities,
}
