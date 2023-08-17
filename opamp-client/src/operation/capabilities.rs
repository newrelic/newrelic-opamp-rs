use crate::opamp::proto::AgentCapabilities;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capabilities(i32);

impl Capabilities {
    pub fn new(value: i32) -> Self {
        Self(value)
    }
    pub fn has_capability(self, capability: AgentCapabilities) -> bool {
        self.0 & capability as i32 != 0
    }
}

#[macro_export]
macro_rules! capabilities {
    ($($cap:expr),*) => {
        Capabilities::new(0 $(| $cap as i32)*)
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use AgentCapabilities::*;

    #[test]
    fn test_many_capabilities() {
        let caps = capabilities!(AcceptsRestartCommand, AcceptsPackages, AcceptsRemoteConfig);
        assert!(caps.has_capability(AcceptsRestartCommand));
        assert!(caps.has_capability(AcceptsPackages));
        assert!(caps.has_capability(AcceptsRemoteConfig));

        assert!(!caps.has_capability(AcceptsOpAmpConnectionSettings));
    }

    #[test]
    fn test_single_capability() {
        let caps = capabilities!(AcceptsRestartCommand);
        assert!(caps.has_capability(AcceptsRestartCommand));
        assert!(!caps.has_capability(AcceptsPackages));
        assert!(!caps.has_capability(AcceptsRemoteConfig));
    }

    #[test]
    fn test_no_capabilities() {
        let caps = capabilities!();
        assert!(!caps.has_capability(AcceptsRestartCommand));
        assert!(!caps.has_capability(AcceptsOpAmpConnectionSettings));

        assert_eq!(caps, Capabilities::default());
        assert_eq!(caps, Capabilities(0));
    }
}
