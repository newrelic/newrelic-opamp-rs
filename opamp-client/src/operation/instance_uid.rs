//! Provide tools to represent and generate a valid instance_uid.
//!
use std::convert::{TryFrom, TryInto};
use std::fmt::{Display, Formatter};
use thiserror::Error;
use uuid::Uuid;

/// Holds possible errors that can occur when working with instance uid.
#[derive(Error, Debug)]
pub enum InstanceUidError {
    /// The format is invalid
    #[error("invalid instance_uid format: {0}")]
    InvalidFormat(String),
}

/// Represents an [Agent Instance UID](https://github.com/open-telemetry/opamp-spec/blob/main/specification.md#agenttoserverinstance_uid)
/// Currently, it's represented by a [UUID v7](https://www.ietf.org/archive/id/draft-ietf-uuidrev-rfc4122bis-14.html#name-uuid-version-7)
///
/// The corresponding string representation is uppercase with no hyphens.
///
/// Example:
/// ```
/// use opamp_client::operation::instance_uid::InstanceUid;
///
/// let instance_uid = InstanceUid::try_from("0190592a-8287-7fb1-a6d9-1ecaa57032bd").unwrap();
/// assert_eq!(instance_uid.to_string(), "0190592A82877FB1A6D91ECAA57032BD".to_string());
/// ```
#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub struct InstanceUid(Uuid);

impl InstanceUid {
    /// Creates a new instance with a random valid value. It uses UUID v7.
    pub fn create() -> Self {
        Self(Uuid::now_v7())
    }
}

impl TryFrom<String> for InstanceUid {
    type Error = InstanceUidError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from(value.as_str())
    }
}

impl TryFrom<Vec<u8>> for InstanceUid {
    type Error = InstanceUidError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let uuid: Uuid = value
            .try_into()
            .map_err(|e: uuid::Error| InstanceUidError::InvalidFormat(e.to_string()))?;

        Ok(Self(uuid))
    }
}

impl TryFrom<&str> for InstanceUid {
    type Error = InstanceUidError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(Self(Uuid::try_parse(value).map_err(|e| {
            InstanceUidError::InvalidFormat(e.to_string())
        })?))
    }
}

impl From<InstanceUid> for Vec<u8> {
    fn from(val: InstanceUid) -> Self {
        val.0.into()
    }
}

impl Display for InstanceUid {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.as_simple().to_string().to_uppercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;

    #[test]
    fn test_instance_id_display_uppercase_no_hyphen() {
        let instance_uid = InstanceUid::try_from("0190592a-8287-7fb1-a6d9-1ecaa57032bd").unwrap();
        assert_eq!(
            instance_uid.to_string(),
            String::from("0190592A82877FB1A6D91ECAA57032BD")
        );
    }

    #[test]
    fn test_instance_id_accepts_no_hyphen_on_build() {
        let instance_uid = InstanceUid::try_from("0190592A82877FB1A6D91ECAA57032BD").unwrap();
        assert_eq!(
            instance_uid.to_string(),
            String::from("0190592A82877FB1A6D91ECAA57032BD")
        );
    }

    #[test]
    fn test_instance_id_invalid_uuid() {
        struct TestCase {
            _name: &'static str,
            uuid: &'static str,
        }
        impl TestCase {
            fn run(self) {
                let err = InstanceUid::try_from(self.uuid).unwrap_err();
                assert_matches!(err, InstanceUidError::InvalidFormat(_))
            }
        }
        let test_cases = vec![
            TestCase {
                _name: "Contains a non-hexadecimal character 'g'",
                uuid: "g2345678-1234-7234-1234-123456789012",
            },
            TestCase {
                _name: "Hyphens in the wrong position",
                uuid: "123456781234-7234-1234-123456789012",
            },
            TestCase {
                _name: "Incorrect length, too short",
                uuid: "12345678-1234-7234-1234-12345678901",
            },
            TestCase {
                _name: "Incorrect length, too long",
                uuid: "12345678-1234-7234-1234-1234567890123",
            },
            TestCase {
                _name: "Ends with a hyphen",
                uuid: "12345678-1234-7234-1234-12345678901-",
            },
        ];

        for test_case in test_cases {
            test_case.run();
        }
    }
}
