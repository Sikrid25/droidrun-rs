/// StateProvider — orchestrates fetching and parsing device state.
use tracing::{debug, warn};

use crate::driver::DeviceDriver;
use crate::error::{DroidrunError, Result};
use crate::ui::filter::TreeFilter;
use crate::ui::formatter::TreeFormatter;
use crate::ui::state::{ScreenDimensions, UIState};

/// Fetches state from an Android device, applies filters and formatters.
pub struct AndroidStateProvider<F: TreeFilter, M: TreeFormatter> {
    filter: F,
    formatter: M,
    use_normalized: bool,
}

impl<F: TreeFilter, M: TreeFormatter> AndroidStateProvider<F, M> {
    pub fn new(filter: F, formatter: M, use_normalized: bool) -> Self {
        Self {
            filter,
            formatter,
            use_normalized,
        }
    }

    /// Fetch and process the current UI state.
    ///
    /// Includes retry logic (3 attempts).
    pub async fn get_state(&self, driver: &dyn DeviceDriver) -> Result<UIState> {
        let max_retries = 3;
        let mut last_error = None;

        for attempt in 1..=max_retries {
            debug!("getting state (attempt {attempt}/{max_retries})");

            match self.get_state_inner(driver).await {
                Ok(state) => return Ok(state),
                Err(e) => {
                    warn!("get_state attempt {attempt} failed: {e}");
                    last_error = Some(e);
                    if attempt < max_retries {
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            DroidrunError::PortalCommError("get_state failed after retries".into())
        }))
    }

    async fn get_state_inner(&self, driver: &dyn DeviceDriver) -> Result<UIState> {
        let combined = driver.get_ui_tree().await?;

        // Check for error response
        if combined.get("error").is_some() {
            let msg = combined
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            return Err(DroidrunError::PortalCommError(format!(
                "Portal returned error: {msg}"
            )));
        }

        // Validate required keys
        for key in &["a11y_tree", "phone_state", "device_context"] {
            if combined.get(*key).is_none() {
                return Err(DroidrunError::Parse(format!("Missing data in state: {key}")));
            }
        }

        let device_context = &combined["device_context"];
        let screen_bounds = device_context
            .get("screen_bounds")
            .cloned()
            .unwrap_or_default();
        let screen_width = screen_bounds
            .get("width")
            .and_then(|v| v.as_i64())
            .unwrap_or(1080) as i32;
        let screen_height = screen_bounds
            .get("height")
            .and_then(|v| v.as_i64())
            .unwrap_or(2400) as i32;

        // Filter tree
        let filtered = self
            .filter
            .filter(&combined["a11y_tree"], device_context);

        // Format
        let (formatted_text, focused_text, elements, phone_state) = self.formatter.format(
            filtered.as_ref(),
            &combined["phone_state"],
            screen_width,
            screen_height,
            self.use_normalized,
        );

        Ok(UIState::new(
            elements,
            formatted_text,
            focused_text,
            phone_state,
            ScreenDimensions {
                width: screen_width,
                height: screen_height,
            },
            self.use_normalized,
        ))
    }
}
