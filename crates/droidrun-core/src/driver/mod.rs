//! Device driver trait and implementations.

pub mod android;
pub mod recording;

use std::collections::HashSet;
use std::path::Path;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Information about an installed app.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInfo {
    pub package: String,
    pub label: String,
}

/// A 2D point on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

/// Supported device actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Tap,
    Swipe,
    InputText,
    PressKey,
    Drag,
    StartApp,
    InstallApp,
    Screenshot,
    GetUiTree,
    GetDate,
    GetApps,
    ListPackages,
}

/// Trait for all device drivers.
///
/// Concrete drivers implement the methods they support.
/// Unsupported methods return `DroidrunError::NotSupported`.
#[async_trait]
pub trait DeviceDriver: Send + Sync {
    // ── Lifecycle ──────────────────────────────────────────────

    /// Establish connection to the device.
    async fn connect(&mut self) -> Result<()>;

    /// Connect if not already connected.
    async fn ensure_connected(&mut self) -> Result<()>;

    // ── Input actions ──────────────────────────────────────────

    /// Tap at absolute pixel coordinates.
    async fn tap(&self, x: i32, y: i32) -> Result<()>;

    /// Swipe from (x1, y1) to (x2, y2) over duration_ms.
    async fn swipe(
        &self,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        duration_ms: u32,
    ) -> Result<()>;

    /// Type text into the currently focused field.
    async fn input_text(&self, text: &str, clear: bool) -> Result<bool>;

    /// Send a single key event.
    async fn press_key(&self, keycode: i32) -> Result<()>;

    /// Drag from (x1, y1) to (x2, y2).
    async fn drag(
        &self,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        duration_ms: u32,
    ) -> Result<()>;

    // ── App management ─────────────────────────────────────────

    /// Launch an application.
    async fn start_app(&self, package: &str, activity: Option<&str>) -> Result<String>;

    /// Install an APK.
    async fn install_app(&self, path: &Path) -> Result<String>;

    /// List installed apps with labels.
    async fn get_apps(&self, include_system: bool) -> Result<Vec<AppInfo>>;

    /// List installed package names.
    async fn list_packages(&self, include_system: bool) -> Result<Vec<String>>;

    // ── State / observation ────────────────────────────────────

    /// Take a screenshot (PNG bytes).
    async fn screenshot(&self, hide_overlay: bool) -> Result<Vec<u8>>;

    /// Get the accessibility tree + phone state as JSON.
    async fn get_ui_tree(&self) -> Result<serde_json::Value>;

    /// Get the device date/time.
    async fn get_date(&self) -> Result<String>;

    // ── Capabilities ───────────────────────────────────────────

    /// Which actions this driver supports.
    fn supported_actions(&self) -> &HashSet<Action>;
}
