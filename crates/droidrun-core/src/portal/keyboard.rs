/// DroidRun keyboard (IME) management.
use droidrun_adb::AdbDevice;
use tracing::debug;

use super::KEYBOARD_IME;
use crate::error::{DroidrunError, Result};

/// Set up the DroidRun keyboard as the default input method.
pub async fn setup_keyboard(device: &AdbDevice) -> Result<()> {
    device
        .shell(&format!("ime enable {KEYBOARD_IME}"))
        .await
        .map_err(DroidrunError::Adb)?;

    device
        .shell(&format!("ime set {KEYBOARD_IME}"))
        .await
        .map_err(DroidrunError::Adb)?;

    debug!("DroidRun keyboard enabled");
    Ok(())
}

/// Disable the DroidRun keyboard.
pub async fn disable_keyboard(device: &AdbDevice) -> Result<()> {
    device
        .shell(&format!("ime disable {KEYBOARD_IME}"))
        .await
        .map_err(DroidrunError::Adb)?;

    debug!("DroidRun keyboard disabled");
    Ok(())
}
