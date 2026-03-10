/// Errors from ADB operations.
#[derive(Debug, thiserror::Error)]
pub enum AdbError {
    #[error("ADB I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("ADB server returned FAIL: {0}")]
    ServerFailed(String),

    #[error("ADB protocol error: {0}")]
    Protocol(String),

    #[error("No device connected")]
    NoDevice,

    #[error("Device not online (state: {0})")]
    DeviceNotOnline(String),

    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("Shell command failed: {0}")]
    ShellError(String),

    #[error("Install failed: {0}")]
    InstallFailed(String),

    #[error("UTF-8 decode error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Connection refused — is ADB server running? (adb start-server)")]
    ConnectionRefused,

    #[error("Timeout: {0}")]
    Timeout(String),
}

pub type Result<T> = std::result::Result<T, AdbError>;
