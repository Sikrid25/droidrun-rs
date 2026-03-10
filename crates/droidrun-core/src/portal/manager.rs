/// Portal APK lifecycle management — download, install, version checks.
use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use tracing::{debug, info, warn};

use droidrun_adb::AdbDevice;

use super::{a11y, keyboard, PORTAL_PACKAGE, VERSION_MAP_URL};
use crate::error::{DroidrunError, Result};
use crate::portal::client::parse_content_provider_output;

const ASSET_NAME: &str = "droidrun-portal";

/// Version mapping from server.
#[derive(Debug, Deserialize)]
struct VersionMap {
    mappings: HashMap<String, String>,
    #[serde(default = "default_download_base")]
    download_base: String,
}

fn default_download_base() -> String {
    "https://github.com/droidrun/droidrun-portal/releases/download".into()
}

/// Manages Portal APK lifecycle on a device.
pub struct PortalManager {
    device: AdbDevice,
    http: reqwest::Client,
}

impl PortalManager {
    pub fn new(device: AdbDevice) -> Self {
        Self {
            device,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Full setup: download → install → enable accessibility → setup keyboard.
    pub async fn setup(&self, sdk_version: &str, debug_mode: bool) -> Result<()> {
        let (portal_version, download_base) =
            self.get_compatible_version(sdk_version, debug_mode).await?;

        let apk_path = self.download_apk(&portal_version, &download_base).await?;
        info!("installing Portal APK v{portal_version}...");

        self.device
            .install(Path::new(&apk_path), &["-g"])
            .await
            .map_err(DroidrunError::Adb)?;

        // Cleanup temp file
        let _ = tokio::fs::remove_file(&apk_path).await;

        info!("Portal APK installed");

        // Enable accessibility service
        a11y::enable(&self.device).await?;
        self.wait_for_service(std::time::Duration::from_secs(10))
            .await?;
        info!("accessibility service enabled");

        // Setup keyboard
        keyboard::setup_keyboard(&self.device).await?;

        Ok(())
    }

    /// Check if Portal is ready, auto-fix if not.
    pub async fn ensure_ready(&self, sdk_version: &str, debug_mode: bool) -> Result<()> {
        // Parallel health checks
        let (packages_result, version_result, a11y_result) = tokio::join!(
            self.device.list_packages(&[]),
            self.device
                .shell("content query --uri content://com.droidrun.portal/version"),
            self.device
                .shell("settings get secure enabled_accessibility_services"),
        );

        // If all checks failed, device is likely unreachable
        if packages_result.is_err() && version_result.is_err() && a11y_result.is_err() {
            debug!("portal health check skipped (device unreachable)");
            return Ok(());
        }

        let is_installed = packages_result
            .as_ref()
            .map(|pkgs| pkgs.iter().any(|p| p == PORTAL_PACKAGE))
            .unwrap_or(false);

        let installed_version = version_result
            .as_ref()
            .ok()
            .and_then(|raw| parse_portal_version(raw));

        let a11y_enabled = a11y_result
            .as_ref()
            .map(|s| s.contains(super::A11Y_SERVICE))
            .unwrap_or(false);

        // Check version compatibility
        let mut needs_upgrade = false;
        if is_installed {
            if let Some(ref installed_ver) = installed_version {
                if let Ok((expected, _)) =
                    self.get_compatible_version(sdk_version, debug_mode).await
                {
                    let expected_clean = expected.trim_start_matches('v');
                    if installed_ver != expected_clean {
                        info!(
                            "portal version mismatch: installed={installed_ver}, expected={expected}"
                        );
                        needs_upgrade = true;
                    }
                }
            }
        }

        // Fix if needed
        if !is_installed || needs_upgrade {
            let reason = if !is_installed {
                "not installed"
            } else {
                "outdated"
            };
            info!("portal {reason}, running auto-setup...");
            self.setup(sdk_version, debug_mode).await?;
            return Ok(());
        }

        if !a11y_enabled {
            info!("portal accessibility service not enabled, enabling...");
            a11y::enable(&self.device).await?;
            if !a11y::check(&self.device).await? {
                return Err(DroidrunError::PortalAccessibilityDisabled);
            }
            self.wait_for_service(std::time::Duration::from_secs(10))
                .await?;
            info!("accessibility service enabled");
        }

        Ok(())
    }

    /// Get compatible Portal version for a given SDK version.
    async fn get_compatible_version(
        &self,
        sdk_version: &str,
        debug_mode: bool,
    ) -> Result<(String, String)> {
        let version_map = self.fetch_version_map(debug_mode).await?;

        // Exact match first
        if let Some(portal_ver) = version_map.mappings.get(sdk_version) {
            return Ok((portal_ver.clone(), version_map.download_base));
        }

        // Range match (e.g., "0.4.0-0.4.14": "1.0.0")
        for (key, portal_ver) in &version_map.mappings {
            if version_in_range(sdk_version, key) {
                return Ok((portal_ver.clone(), version_map.download_base.clone()));
            }
        }

        // Fallback: use latest from mappings
        if let Some((_, portal_ver)) = version_map.mappings.iter().last() {
            warn!("no exact match for SDK {sdk_version}, using latest portal: {portal_ver}");
            return Ok((portal_ver.clone(), version_map.download_base));
        }

        Err(DroidrunError::PortalSetupFailed(
            "cannot determine compatible portal version".into(),
        ))
    }

    async fn fetch_version_map(&self, _debug: bool) -> Result<VersionMap> {
        let resp = self
            .http
            .get(VERSION_MAP_URL)
            .send()
            .await
            .map_err(DroidrunError::Http)?;

        resp.json::<VersionMap>()
            .await
            .map_err(|e| DroidrunError::PortalSetupFailed(format!("failed to parse version map: {e}")))
    }

    async fn download_apk(&self, version: &str, download_base: &str) -> Result<String> {
        let url = format!("{download_base}/{version}/{ASSET_NAME}-{version}.apk");
        info!("downloading Portal APK v{version}");
        debug!("URL: {url}");

        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(DroidrunError::Http)?;

        if !resp.status().is_success() {
            return Err(DroidrunError::PortalSetupFailed(format!(
                "APK download failed: HTTP {}",
                resp.status()
            )));
        }

        let bytes = resp.bytes().await.map_err(DroidrunError::Http)?;

        let tmp = tempfile::Builder::new()
            .suffix(".apk")
            .tempfile()
            .map_err(DroidrunError::Io)?;
        let path = tmp.path().to_string_lossy().to_string();
        tokio::fs::write(&path, &bytes)
            .await
            .map_err(DroidrunError::Io)?;
        // Keep the file alive (don't let tmp drop delete it)
        tmp.into_temp_path();

        debug!("downloaded {} bytes to {path}", bytes.len());
        Ok(path)
    }

    async fn wait_for_service(&self, timeout: std::time::Duration) -> Result<()> {
        let start = tokio::time::Instant::now();
        let interval = std::time::Duration::from_secs(1);

        while start.elapsed() < timeout {
            if let Ok(output) = self
                .device
                .shell("content query --uri content://com.droidrun.portal/state")
                .await
            {
                if output.contains(r#""status":"success""#) {
                    return Ok(());
                }
            }
            tokio::time::sleep(interval).await;
        }

        warn!("portal service did not become responsive within timeout");
        Ok(())
    }
}

/// Check if version falls within a range like "0.4.0-0.4.14".
fn version_in_range(version: &str, range: &str) -> bool {
    let Some((start, end)) = range.split_once('-') else {
        return false;
    };

    let parse = |s: &str| -> Option<Vec<u32>> {
        s.split('.').map(|p| p.parse().ok()).collect()
    };

    let Some(v) = parse(version) else {
        return false;
    };
    let Some(s) = parse(start) else {
        return false;
    };
    let Some(e) = parse(end) else {
        return false;
    };

    v >= s && v <= e
}

/// Extract portal version string from content provider output.
fn parse_portal_version(raw: &str) -> Option<String> {
    let data = parse_content_provider_output(raw)?;
    data.as_str().map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_in_range_exact() {
        assert!(version_in_range("0.4.5", "0.4.0-0.4.14"));
    }

    #[test]
    fn test_version_in_range_start() {
        assert!(version_in_range("0.4.0", "0.4.0-0.4.14"));
    }

    #[test]
    fn test_version_in_range_end() {
        assert!(version_in_range("0.4.14", "0.4.0-0.4.14"));
    }

    #[test]
    fn test_version_out_of_range() {
        assert!(!version_in_range("0.5.0", "0.4.0-0.4.14"));
        assert!(!version_in_range("0.3.9", "0.4.0-0.4.14"));
    }

    #[test]
    fn test_version_in_range_no_dash() {
        assert!(!version_in_range("0.4.0", "0.4.0"));
    }
}
