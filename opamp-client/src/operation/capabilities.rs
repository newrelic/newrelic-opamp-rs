use crate::opamp::proto::AgentCapabilities;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capabilities(i32);

impl Capabilities {
    pub fn new(caps: Vec<AgentCapabilities>) -> Self {
        Self(caps.into_iter().fold(0i32, |c1, c2| c1 | c2 as i32))
    }
    pub fn has_capability(self, capability: AgentCapabilities) -> bool {
        self.0 & capability as i32 != 0
    }
}

#[macro_export]
macro_rules! capabilities {
    ($($cap:expr),*) => {{
        let caps: Vec<AgentCapabilities> = vec![AgentCapabilities::Unspecified $(, $cap)*];
        Capabilities::new(caps)
    }};
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
