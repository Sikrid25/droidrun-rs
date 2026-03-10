/// Errors from droidrun-core operations.
#[derive(Debug, thiserror::Error)]
pub enum DroidrunError {
    #[error("ADB error: {0}")]
    Adb(#[from] droidrun_adb::AdbError),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Device not connected")]
    NotConnected,

    #[error("Portal not installed on device")]
    PortalNotInstalled,

    #[error("Portal accessibility service not enabled")]
    PortalAccessibilityDisabled,

    #[error("Portal setup failed: {0}")]
    PortalSetupFailed(String),

    #[error("Portal communication error: {0}")]
    PortalCommError(String),

    #[error("Element not found: index {0}")]
    ElementNotFound(usize),

    #[error("Element {0} has no bounds")]
    ElementNoBounds(usize),

    #[error("Element {0} is fully obscured")]
    ElementObscured(usize),

    #[error("Invalid bounds format: {0}")]
    InvalidBounds(String),

    #[error("Screen dimensions not available")]
    NoDimensions,

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Not supported: {0}")]
    NotSupported(String),

    #[error("Timeout: {0}")]
    Timeout(String),
}

pub type Result<T> = std::result::Result<T, DroidrunError>;
