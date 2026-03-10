/// Data models for ADB operations.
use std::fmt;

/// Device state as reported by ADB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceState {
    Device,
    Offline,
    Unauthorized,
    Authorizing,
    Connecting,
    Recovery,
    Bootloader,
    Unknown(String),
}

impl DeviceState {
    pub fn is_online(&self) -> bool {
        matches!(self, DeviceState::Device)
    }
}

impl fmt::Display for DeviceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Device => write!(f, "device"),
            Self::Offline => write!(f, "offline"),
            Self::Unauthorized => write!(f, "unauthorized"),
            Self::Authorizing => write!(f, "authorizing"),
            Self::Connecting => write!(f, "connecting"),
            Self::Recovery => write!(f, "recovery"),
            Self::Bootloader => write!(f, "bootloader"),
            Self::Unknown(s) => write!(f, "{s}"),
        }
    }
}

impl From<&str> for DeviceState {
    fn from(s: &str) -> Self {
        match s.trim() {
            "device" => Self::Device,
            "offline" => Self::Offline,
            "unauthorized" => Self::Unauthorized,
            "authorizing" => Self::Authorizing,
            "connecting" => Self::Connecting,
            "recovery" => Self::Recovery,
            "bootloader" => Self::Bootloader,
            other => Self::Unknown(other.to_string()),
        }
    }
}

/// Basic device info from `host:devices`.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub serial: String,
    pub state: DeviceState,
}

/// A port forward entry.
#[derive(Debug, Clone)]
pub struct ForwardEntry {
    pub serial: String,
    pub local: String,
    pub remote: String,
}

impl ForwardEntry {
    /// Extract port number from a "tcp:XXXXX" string.
    pub fn local_port(&self) -> Option<u16> {
        self.local
            .strip_prefix("tcp:")
            .and_then(|s| s.parse().ok())
    }

    pub fn remote_port(&self) -> Option<u16> {
        self.remote
            .strip_prefix("tcp:")
            .and_then(|s| s.parse().ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_state_from_str() {
        assert_eq!(DeviceState::from("device"), DeviceState::Device);
        assert_eq!(DeviceState::from("offline"), DeviceState::Offline);
        assert_eq!(DeviceState::from("unauthorized"), DeviceState::Unauthorized);
        assert!(DeviceState::from("device").is_online());
        assert!(!DeviceState::from("offline").is_online());
    }

    #[test]
    fn test_forward_entry_ports() {
        let entry = ForwardEntry {
            serial: "emulator-5554".into(),
            local: "tcp:27183".into(),
            remote: "tcp:8080".into(),
        };
        assert_eq!(entry.local_port(), Some(27183));
        assert_eq!(entry.remote_port(), Some(8080));
    }

    #[test]
    fn test_forward_entry_non_tcp() {
        let entry = ForwardEntry {
            serial: "device".into(),
            local: "localabstract:foo".into(),
            remote: "tcp:5000".into(),
        };
        assert_eq!(entry.local_port(), None);
        assert_eq!(entry.remote_port(), Some(5000));
    }
}
