/// Accessibility service management for DroidRun Portal.
use droidrun_adb::AdbDevice;
use tracing::debug;

use super::A11Y_SERVICE;
use crate::error::{DroidrunError, Result};

/// Enable the Portal accessibility service.
pub async fn enable(device: &AdbDevice) -> Result<()> {
    device
        .shell(&format!(
            "settings put secure enabled_accessibility_services {A11Y_SERVICE}"
        ))
        .await
        .map_err(DroidrunError::Adb)?;

    device
        .shell("settings put secure accessibility_enabled 1")
        .await
        .map_err(DroidrunError::Adb)?;

    debug!("accessibility service enabled");
    Ok(())
}

/// Check if the Portal accessibility service is enabled.
pub async fn check(device: &AdbDevice) -> Result<bool> {
    let services = device
        .shell("settings get secure enabled_accessibility_services")
        .await
        .map_err(DroidrunError::Adb)?;

    if !services.contains(A11Y_SERVICE) {
        return Ok(false);
    }

    let enabled = device
        .shell("settings get secure accessibility_enabled")
        .await
        .map_err(DroidrunError::Adb)?;

    Ok(enabled.trim() == "1")
}

/// Open the accessibility settings screen on the device.
pub async fn open_settings(device: &AdbDevice) -> Result<()> {
    device
        .shell("am start -a android.settings.ACCESSIBILITY_SETTINGS")
        .await
        .map_err(DroidrunError::Adb)?;
    Ok(())
}

/// Set the overlay offset.
pub async fn set_overlay_offset(device: &AdbDevice, offset: i32) -> Result<()> {
    device
        .shell(&format!(
            r#"content insert --uri "content://com.droidrun.portal/overlay_offset" --bind offset:i:{offset}"#
        ))
        .await
        .map_err(DroidrunError::Adb)?;
    Ok(())
}

/// Toggle the overlay visibility.
pub async fn toggle_overlay(device: &AdbDevice, visible: bool) -> Result<()> {
    let visible_str = if visible { "true" } else { "false" };
    device
        .shell(&format!(
            r#"content insert --uri "content://com.droidrun.portal/overlay_visible" --bind visible:b:{visible_str}"#
        ))
        .await
        .map_err(DroidrunError::Adb)?;
    Ok(())
}
