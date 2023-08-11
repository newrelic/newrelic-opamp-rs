use crate::opamp::proto::AgentCapabilities;

pub trait Capabilities {
    fn has_capability(self, capability: AgentCapabilities) -> bool;
}

impl Capabilities for AgentCapabilities {
    fn has_capability(self, capability: AgentCapabilities) -> bool {
        self as u64 & capability as u64 != 0
    }
}
