/// RecordingDriver — transparent proxy that logs device actions for macro replay.
use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{Action, AppInfo, DeviceDriver};
use crate::error::Result;

/// A recorded action entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action_type")]
pub enum RecordedAction {
    #[serde(rename = "tap")]
    Tap { x: i32, y: i32 },

    #[serde(rename = "swipe")]
    Swipe {
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
        duration_ms: u32,
    },

    #[serde(rename = "input_text")]
    InputText { text: String, clear: bool },

    #[serde(rename = "key_press")]
    KeyPress { keycode: i32 },

    #[serde(rename = "start_app")]
    StartApp {
        package: String,
        activity: Option<String>,
    },

    #[serde(rename = "drag")]
    Drag {
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
        duration_ms: u32,
    },
}

/// Proxy driver that records all mutating actions.
///
/// Read-only methods (screenshot, get_ui_tree, etc.) pass through
/// without recording.
pub struct RecordingDriver<D: DeviceDriver> {
    inner: D,
    log: Mutex<Vec<RecordedAction>>,
}

impl<D: DeviceDriver> RecordingDriver<D> {
    pub fn new(inner: D) -> Self {
        Self {
            inner,
            log: Mutex::new(Vec::new()),
        }
    }

    /// Get all recorded actions.
    pub fn recorded_actions(&self) -> Vec<RecordedAction> {
        self.log.lock().unwrap().clone()
    }

    /// Clear the recording log.
    pub fn clear_log(&self) {
        self.log.lock().unwrap().clear();
    }

    /// Serialize the log to JSON.
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(&self.recorded_actions())
    }

    fn record(&self, action: RecordedAction) {
        self.log.lock().unwrap().push(action);
    }
}

#[async_trait]
impl<D: DeviceDriver> DeviceDriver for RecordingDriver<D> {
    async fn connect(&mut self) -> Result<()> {
        self.inner.connect().await
    }

    async fn ensure_connected(&mut self) -> Result<()> {
        self.inner.ensure_connected().await
    }

    async fn tap(&self, x: i32, y: i32) -> Result<()> {
        self.inner.tap(x, y).await?;
        self.record(RecordedAction::Tap { x, y });
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
        self.inner.swipe(x1, y1, x2, y2, duration_ms).await?;
        self.record(RecordedAction::Swipe {
            start_x: x1,
            start_y: y1,
            end_x: x2,
            end_y: y2,
            duration_ms,
        });
        Ok(())
    }

    async fn input_text(&self, text: &str, clear: bool) -> Result<bool> {
        let result = self.inner.input_text(text, clear).await?;
        self.record(RecordedAction::InputText {
            text: text.to_string(),
            clear,
        });
        Ok(result)
    }

    async fn press_key(&self, keycode: i32) -> Result<()> {
        self.inner.press_key(keycode).await?;
        self.record(RecordedAction::KeyPress { keycode });
        Ok(())
    }

    async fn drag(
        &self,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        duration_ms: u32,
    ) -> Result<()> {
        self.inner.drag(x1, y1, x2, y2, duration_ms).await?;
        self.record(RecordedAction::Drag {
            start_x: x1,
            start_y: y1,
            end_x: x2,
            end_y: y2,
            duration_ms,
        });
        Ok(())
    }

    async fn start_app(&self, package: &str, activity: Option<&str>) -> Result<String> {
        let result = self.inner.start_app(package, activity).await?;
        self.record(RecordedAction::StartApp {
            package: package.to_string(),
            activity: activity.map(|a| a.to_string()),
        });
        Ok(result)
    }

    // ── Pass-through (not recorded) ────────────────────────────

    async fn install_app(&self, path: &Path) -> Result<String> {
        self.inner.install_app(path).await
    }

    async fn get_apps(&self, include_system: bool) -> Result<Vec<AppInfo>> {
        self.inner.get_apps(include_system).await
    }

    async fn list_packages(&self, include_system: bool) -> Result<Vec<String>> {
        self.inner.list_packages(include_system).await
    }

    async fn screenshot(&self, hide_overlay: bool) -> Result<Vec<u8>> {
        self.inner.screenshot(hide_overlay).await
    }

    async fn get_ui_tree(&self) -> Result<serde_json::Value> {
        self.inner.get_ui_tree().await
    }

    async fn get_date(&self) -> Result<String> {
        self.inner.get_date().await
    }

    fn supported_actions(&self) -> &HashSet<Action> {
        self.inner.supported_actions()
    }
}
