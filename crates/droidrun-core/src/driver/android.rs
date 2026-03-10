/// AndroidDriver — ADB + Portal based device driver.
use std::collections::HashSet;
use std::path::Path;

use async_trait::async_trait;
use tracing::{debug, info};

use droidrun_adb::AdbDevice;

use crate::error::{DroidrunError, Result};
use crate::portal::client::PortalClient;
use crate::portal::keyboard;

use super::{Action, AppInfo, DeviceDriver};

const PORTAL_DEFAULT_TCP_PORT: u16 = 8080;

/// Android device driver using ADB + DroidRun Portal.
pub struct AndroidDriver {
    serial: Option<String>,
    use_tcp: bool,
    remote_tcp_port: u16,
    device: Option<AdbDevice>,
    portal: Option<PortalClient>,
    connected: bool,
    supported: HashSet<Action>,
}

impl AndroidDriver {
    /// Create a new Android driver.
    pub fn new(serial: Option<&str>, use_tcp: bool) -> Self {
        let supported = HashSet::from([
            Action::Tap,
            Action::Swipe,
            Action::InputText,
            Action::PressKey,
            Action::StartApp,
            Action::InstallApp,
            Action::Screenshot,
            Action::GetUiTree,
            Action::GetDate,
            Action::GetApps,
            Action::ListPackages,
            Action::Drag,
        ]);

        Self {
            serial: serial.map(|s| s.to_string()),
            use_tcp,
            remote_tcp_port: PORTAL_DEFAULT_TCP_PORT,
            device: None,
            portal: None,
            connected: false,
            supported,
        }
    }

    /// Get a reference to the underlying ADB device.
    pub fn adb_device(&self) -> Result<&AdbDevice> {
        self.device.as_ref().ok_or(DroidrunError::NotConnected)
    }

    /// Get a reference to the Portal client.
    pub fn portal_client(&self) -> Result<&PortalClient> {
        self.portal.as_ref().ok_or(DroidrunError::NotConnected)
    }
}

#[async_trait]
impl DeviceDriver for AndroidDriver {
    // ── Lifecycle ──────────────────────────────────────────────

    async fn connect(&mut self) -> Result<()> {
        if self.connected {
            return Ok(());
        }

        // Resolve device
        let server = droidrun_adb::AdbServer::default();
        let device = server.resolve_device(self.serial.as_deref()).await?;

        // Verify device is online
        let state = device.get_state().await?;
        if !state.is_online() {
            return Err(DroidrunError::Adb(droidrun_adb::AdbError::DeviceNotOnline(
                state.to_string(),
            )));
        }

        info!("connected to device: {}", device.serial);

        // Create Portal client
        let mut portal = PortalClient::new(device.clone(), self.use_tcp, self.remote_tcp_port);
        portal.connect().await?;

        // Setup keyboard
        keyboard::setup_keyboard(&device).await?;

        self.device = Some(device);
        self.portal = Some(portal);
        self.connected = true;

        Ok(())
    }

    async fn ensure_connected(&mut self) -> Result<()> {
        if !self.connected {
            self.connect().await?;
        }
        Ok(())
    }

    // ── Input actions ──────────────────────────────────────────

    async fn tap(&self, x: i32, y: i32) -> Result<()> {
        let device = self.adb_device()?;
        device.tap(x, y).await?;
        Ok(())
    }

    async fn swipe(
        &self,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        duration_ms: u32,
    ) -> Result<()> {
        let device = self.adb_device()?;
        device.swipe(x1, y1, x2, y2, duration_ms).await?;
        tokio::time::sleep(std::time::Duration::from_millis(duration_ms as u64)).await;
        Ok(())
    }

    async fn input_text(&self, text: &str, clear: bool) -> Result<bool> {
        let portal = self.portal_client()?;
        portal.input_text(text, clear).await
    }

    async fn press_key(&self, keycode: i32) -> Result<()> {
        let device = self.adb_device()?;
        device.keyevent(keycode).await?;
        Ok(())
    }

    async fn drag(
        &self,
        _x1: i32,
        _y1: i32,
        _x2: i32,
        _y2: i32,
        _duration_ms: u32,
    ) -> Result<()> {
        Err(DroidrunError::NotSupported("drag is not implemented yet".into()))
    }

    // ── App management ─────────────────────────────────────────

    async fn start_app(&self, package: &str, activity: Option<&str>) -> Result<String> {
        let device = self.adb_device()?;
        debug!("starting app {package} with activity {activity:?}");
        match device.app_start(package, activity).await {
            Ok(result) => Ok(result),
            Err(e) => Ok(format!("Failed to start app {package}: {e}")),
        }
    }

    async fn install_app(&self, path: &Path) -> Result<String> {
        let device = self.adb_device()?;
        let result = device.install(path, &["-g"]).await?;
        Ok(result)
    }

    async fn get_apps(&self, include_system: bool) -> Result<Vec<AppInfo>> {
        let portal = self.portal_client()?;
        portal.get_apps(include_system).await
    }

    async fn list_packages(&self, include_system: bool) -> Result<Vec<String>> {
        let device = self.adb_device()?;
        let flags = if include_system {
            vec![]
        } else {
            vec!["-3"]
        };
        let pkgs = device.list_packages(&flags).await?;
        Ok(pkgs)
    }

    // ── State / observation ────────────────────────────────────

    async fn screenshot(&self, hide_overlay: bool) -> Result<Vec<u8>> {
        let portal = self.portal_client()?;
        portal.take_screenshot(hide_overlay).await
    }

    async fn get_ui_tree(&self) -> Result<serde_json::Value> {
        let portal = self.portal_client()?;
        portal.get_state().await
    }

    async fn get_date(&self) -> Result<String> {
        let device = self.adb_device()?;
        device.get_date().await.map_err(|e| e.into())
    }

    // ── Capabilities ───────────────────────────────────────────

    fn supported_actions(&self) -> &HashSet<Action> {
        &self.supported
    }
}
