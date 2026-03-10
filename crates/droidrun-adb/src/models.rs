/// Data models for ADB operations.
use std::fmt;
use std::time::{Duration, UNIX_EPOCH};

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

// ── New types ───────────────────────────────────────────────────

/// Shell command result with exit code.
#[derive(Debug, Clone)]
pub struct ShellOutput {
    pub stdout: String,
    pub exit_code: i32,
}

/// A reverse port forward entry (device-to-host).
#[derive(Debug, Clone)]
pub struct ReverseEntry {
    pub remote: String,
    pub local: String,
}

impl ReverseEntry {
    /// Extract remote port number from a "tcp:XXXXX" string.
    pub fn remote_port(&self) -> Option<u16> {
        self.remote
            .strip_prefix("tcp:")
            .and_then(|s| s.parse().ok())
    }

    /// Extract local port number from a "tcp:XXXXX" string.
    pub fn local_port(&self) -> Option<u16> {
        self.local
            .strip_prefix("tcp:")
            .and_then(|s| s.parse().ok())
    }
}

/// File metadata from the sync STAT command.
#[derive(Debug, Clone)]
pub struct FileStat {
    pub mode: u32,
    pub size: u32,
    pub mtime: u32,
}

impl FileStat {
    /// Whether this is a directory.
    pub fn is_dir(&self) -> bool {
        (self.mode & 0o40000) != 0
    }

    /// Whether this is a regular file.
    pub fn is_file(&self) -> bool {
        (self.mode & 0o100000) != 0
    }

    /// Whether the file exists (mode != 0 means it was found).
    pub fn exists(&self) -> bool {
        self.mode != 0
    }

    /// Convert mtime to a `SystemTime`.
    pub fn modified_time(&self) -> std::time::SystemTime {
        UNIX_EPOCH + Duration::from_secs(self.mtime as u64)
    }
}

/// Directory entry from the sync LIST command.
#[derive(Debug, Clone)]
pub struct SyncDirEntry {
    pub name: String,
    pub mode: u32,
    pub size: u32,
    pub mtime: u32,
}

impl SyncDirEntry {
    pub fn is_dir(&self) -> bool {
        (self.mode & 0o40000) != 0
    }

    pub fn is_file(&self) -> bool {
        (self.mode & 0o100000) != 0
    }
}

/// Reboot mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebootMode {
    /// Normal reboot.
    Normal,
    /// Reboot into bootloader (fastboot).
    Bootloader,
    /// Reboot into recovery.
    Recovery,
    /// Reboot into sideload mode.
    Sideload,
}

impl RebootMode {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Normal => "",
            Self::Bootloader => "bootloader",
            Self::Recovery => "recovery",
            Self::Sideload => "sideload",
        }
    }
}

/// Current foreground app information.
#[derive(Debug, Clone)]
pub struct CurrentApp {
    pub package: String,
    pub activity: String,
}

impl fmt::Display for CurrentApp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.package, self.activity)
    }
}

/// Detailed app information from `dumpsys package`.
#[derive(Debug, Clone)]
pub struct AppDetail {
    pub package: String,
    pub version_name: Option<String>,
    pub version_code: Option<i64>,
    pub install_path: Option<String>,
    pub first_install_time: Option<String>,
    pub last_update_time: Option<String>,
}

/// Device tracking event (for `track_devices` stream).
#[derive(Debug, Clone)]
pub struct DeviceEvent {
    pub serial: String,
    pub state: DeviceState,
}

/// Screen dimensions in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenSize {
    pub width: u32,
    pub height: u32,
}

impl fmt::Display for ScreenSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
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

    // ── New type tests ──────────────────────────────────────────

    #[test]
    fn test_shell_output() {
        let output = ShellOutput {
            stdout: "hello world\n".into(),
            exit_code: 0,
        };
        assert_eq!(output.exit_code, 0);
        assert!(output.stdout.contains("hello"));
    }

    #[test]
    fn test_reverse_entry_ports() {
        let entry = ReverseEntry {
            remote: "tcp:8080".into(),
            local: "tcp:9090".into(),
        };
        assert_eq!(entry.remote_port(), Some(8080));
        assert_eq!(entry.local_port(), Some(9090));
    }

    #[test]
    fn test_reverse_entry_non_tcp() {
        let entry = ReverseEntry {
            remote: "localabstract:foo".into(),
            local: "tcp:5000".into(),
        };
        assert_eq!(entry.remote_port(), None);
        assert_eq!(entry.local_port(), Some(5000));
    }

    #[test]
    fn test_file_stat_dir() {
        let stat = FileStat {
            mode: 0o40755,
            size: 4096,
            mtime: 1700000000,
        };
        assert!(stat.is_dir());
        assert!(!stat.is_file());
        assert!(stat.exists());
    }

    #[test]
    fn test_file_stat_file() {
        let stat = FileStat {
            mode: 0o100644,
            size: 1234,
            mtime: 1700000000,
        };
        assert!(!stat.is_dir());
        assert!(stat.is_file());
        assert!(stat.exists());
    }

    #[test]
    fn test_file_stat_not_found() {
        let stat = FileStat {
            mode: 0,
            size: 0,
            mtime: 0,
        };
        assert!(!stat.is_dir());
        assert!(!stat.is_file());
        assert!(!stat.exists());
    }

    #[test]
    fn test_sync_dir_entry() {
        let entry = SyncDirEntry {
            name: "test.txt".into(),
            mode: 0o100644,
            size: 100,
            mtime: 1700000000,
        };
        assert!(entry.is_file());
        assert!(!entry.is_dir());
    }

    #[test]
    fn test_reboot_mode_str() {
        assert_eq!(RebootMode::Normal.as_str(), "");
        assert_eq!(RebootMode::Bootloader.as_str(), "bootloader");
        assert_eq!(RebootMode::Recovery.as_str(), "recovery");
        assert_eq!(RebootMode::Sideload.as_str(), "sideload");
    }

    #[test]
    fn test_current_app_display() {
        let app = CurrentApp {
            package: "com.example.app".into(),
            activity: ".MainActivity".into(),
        };
        assert_eq!(app.to_string(), "com.example.app/.MainActivity");
    }

    #[test]
    fn test_screen_size_display() {
        let size = ScreenSize {
            width: 1080,
            height: 1920,
        };
        assert_eq!(size.to_string(), "1080x1920");
    }

    #[test]
    fn test_device_event() {
        let event = DeviceEvent {
            serial: "emulator-5554".into(),
            state: DeviceState::Device,
        };
        assert!(event.state.is_online());
    }
}
