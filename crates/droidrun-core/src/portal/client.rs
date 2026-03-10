/// PortalClient — unified communication with DroidRun Portal app.
///
/// Supports two transport modes:
/// - **TCP**: HTTP requests to Portal's embedded server (fast, needs port forward)
/// - **Content Provider**: ADB shell `content query/insert` commands (fallback)
use base64::Engine;
use serde_json::Value;
use tracing::{debug, warn};

use droidrun_adb::AdbDevice;

use crate::driver::AppInfo;
use crate::error::{DroidrunError, Result};

/// Portal client with automatic TCP/ContentProvider fallback.
pub struct PortalClient {
    device: AdbDevice,
    prefer_tcp: bool,
    remote_port: u16,
    tcp_available: bool,
    tcp_base_url: Option<String>,
    local_tcp_port: Option<u16>,
    http: reqwest::Client,
    connected: bool,
}

impl PortalClient {
    pub fn new(device: AdbDevice, prefer_tcp: bool, remote_port: u16) -> Self {
        Self {
            device,
            prefer_tcp,
            remote_port,
            tcp_available: false,
            tcp_base_url: None,
            local_tcp_port: None,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            connected: false,
        }
    }

    /// Establish connection, trying TCP first if preferred.
    pub async fn connect(&mut self) -> Result<()> {
        if self.connected {
            return Ok(());
        }
        if self.prefer_tcp {
            self.try_enable_tcp().await;
        }
        self.connected = true;
        Ok(())
    }

    // ── Public API ──────────────────────────────────────────────

    /// Get device state (accessibility tree + phone state).
    pub async fn get_state(&self) -> Result<Value> {
        if self.tcp_available {
            match self.get_state_tcp().await {
                Ok(state) => return Ok(state),
                Err(e) => debug!("TCP get_state failed: {e}, using fallback"),
            }
        }
        self.get_state_content_provider().await
    }

    /// Input text via keyboard.
    pub async fn input_text(&self, text: &str, clear: bool) -> Result<bool> {
        if self.tcp_available {
            match self.input_text_tcp(text, clear).await {
                Ok(result) => return Ok(result),
                Err(e) => debug!("TCP input_text failed: {e}, using fallback"),
            }
        }
        self.input_text_content_provider(text, clear).await
    }

    /// Take a screenshot.
    pub async fn take_screenshot(&self, hide_overlay: bool) -> Result<Vec<u8>> {
        if self.tcp_available {
            match self.screenshot_tcp(hide_overlay).await {
                Ok(bytes) => return Ok(bytes),
                Err(e) => debug!("TCP screenshot failed: {e}, using fallback"),
            }
        }
        self.screenshot_adb().await
    }

    /// Get installed apps with labels.
    pub async fn get_apps(&self, include_system: bool) -> Result<Vec<AppInfo>> {
        let output = self
            .device
            .shell("content query --uri content://com.droidrun.portal/packages")
            .await
            .map_err(DroidrunError::Adb)?;

        let data = parse_content_provider_output(&output)
            .ok_or_else(|| DroidrunError::Parse("cannot parse packages response".into()))?;

        // Handle various response formats
        let packages_list = extract_packages_list(&data);

        match packages_list {
            Some(list) => {
                let apps: Vec<AppInfo> = list
                    .iter()
                    .filter_map(|item| {
                        let obj = item.as_object()?;
                        if !include_system && obj.get("isSystemApp")?.as_bool().unwrap_or(false) {
                            return None;
                        }
                        Some(AppInfo {
                            package: obj
                                .get("packageName")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            label: obj
                                .get("label")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                        })
                    })
                    .collect();
                debug!("found {} apps", apps.len());
                Ok(apps)
            }
            None => {
                warn!("could not extract packages list from response");
                Ok(vec![])
            }
        }
    }

    /// Get Portal version.
    pub async fn get_version(&self) -> Result<String> {
        if self.tcp_available {
            if let Ok(resp) = self
                .http
                .get(format!("{}/version", self.base_url()))
                .send()
                .await
            {
                if resp.status().is_success() {
                    if let Ok(data) = resp.json::<Value>().await {
                        if let Some(v) = extract_inner_value(&data) {
                            return Ok(v.as_str().unwrap_or("unknown").to_string());
                        }
                    }
                }
            }
        }

        // Content provider fallback
        let output = self
            .device
            .shell("content query --uri content://com.droidrun.portal/version")
            .await
            .map_err(DroidrunError::Adb)?;
        if let Some(data) = parse_content_provider_output(&output) {
            // parse_content_provider_output already unwraps portal envelope,
            // so data might be a direct string value or still an object
            if let Some(s) = data.as_str() {
                return Ok(s.to_string());
            }
            if let Some(v) = extract_inner_value(&data) {
                return Ok(v.as_str().unwrap_or("unknown").to_string());
            }
        }
        Ok("unknown".to_string())
    }

    /// Ping Portal and verify state availability.
    pub async fn ping(&self) -> Result<Value> {
        if self.tcp_available {
            let resp = self
                .http
                .get(format!("{}/ping", self.base_url()))
                .send()
                .await
                .map_err(DroidrunError::Http)?;
            if resp.status().is_success() {
                return Ok(serde_json::json!({
                    "status": "success",
                    "method": "tcp",
                    "url": self.base_url(),
                }));
            }
        }

        // Content provider fallback
        let output = self
            .device
            .shell("content query --uri content://com.droidrun.portal/state")
            .await
            .map_err(DroidrunError::Adb)?;
        if output.contains("Row: 0 result=") {
            Ok(serde_json::json!({
                "status": "success",
                "method": "content_provider",
            }))
        } else {
            Err(DroidrunError::PortalCommError(
                "Portal not reachable".into(),
            ))
        }
    }

    // ── TCP implementations ─────────────────────────────────────

    async fn try_enable_tcp(&mut self) {
        if let Err(e) = self.try_enable_tcp_inner().await {
            warn!("TCP unavailable ({e}), using content provider fallback");
            self.tcp_available = false;
        }
    }

    async fn try_enable_tcp_inner(&mut self) -> Result<()> {
        // Check for existing forward
        let local_port = match self.find_existing_forward().await? {
            Some(port) => {
                debug!("reusing existing forward: localhost:{port} -> device:{}", self.remote_port);
                port
            }
            None => {
                debug!("creating new forward for port {}", self.remote_port);
                self.device
                    .forward(0, self.remote_port)
                    .await
                    .map_err(DroidrunError::Adb)?
            }
        };

        self.local_tcp_port = Some(local_port);
        self.tcp_base_url = Some(format!("http://localhost:{local_port}"));

        // Test connection
        if self.test_tcp_connection().await {
            self.tcp_available = true;
            debug!("TCP mode enabled: {}", self.base_url());
            return Ok(());
        }

        // Try enabling the HTTP server via content provider
        debug!("TCP ping failed, trying to enable Portal HTTP server...");
        let _ = self.device.shell(
            r#"content insert --uri content://com.droidrun.portal/toggle_socket_server --bind enabled:b:true"#
        ).await;
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        if self.test_tcp_connection().await {
            self.tcp_available = true;
            debug!("TCP mode enabled after starting server: {}", self.base_url());
            Ok(())
        } else {
            Err(DroidrunError::PortalCommError(
                "TCP unavailable after enabling server".into(),
            ))
        }
    }

    async fn find_existing_forward(&self) -> Result<Option<u16>> {
        let forwards = self
            .device
            .forward_list()
            .await
            .map_err(DroidrunError::Adb)?;
        let expected_remote = format!("tcp:{}", self.remote_port);
        Ok(forwards
            .iter()
            .find(|f| f.remote == expected_remote)
            .and_then(|f| f.local_port()))
    }

    async fn test_tcp_connection(&self) -> bool {
        match self
            .http
            .get(format!("{}/ping", self.base_url()))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
            Err(e) => {
                debug!("TCP ping failed: {e}");
                false
            }
        }
    }

    fn base_url(&self) -> &str {
        self.tcp_base_url.as_deref().unwrap_or("http://localhost:0")
    }

    async fn get_state_tcp(&self) -> Result<Value> {
        let resp = self
            .http
            .get(format!("{}/state_full", self.base_url()))
            .send()
            .await
            .map_err(DroidrunError::Http)?;

        if !resp.status().is_success() {
            return Err(DroidrunError::PortalCommError(format!(
                "HTTP {}",
                resp.status()
            )));
        }

        let data: Value = resp.json().await.map_err(DroidrunError::Http)?;
        Ok(unwrap_portal_response(data))
    }

    async fn get_state_content_provider(&self) -> Result<Value> {
        let output = self
            .device
            .shell("content query --uri content://com.droidrun.portal/state_full")
            .await
            .map_err(DroidrunError::Adb)?;

        // parse_content_provider_output already unwraps portal envelopes
        parse_content_provider_output(&output)
            .ok_or_else(|| {
                DroidrunError::Parse("failed to parse state data from ContentProvider".into())
            })
    }

    async fn input_text_tcp(&self, text: &str, clear: bool) -> Result<bool> {
        let encoded = base64::engine::general_purpose::STANDARD.encode(text);
        let payload = serde_json::json!({
            "base64_text": encoded,
            "clear": clear,
        });

        let resp = self
            .http
            .post(format!("{}/keyboard/input", self.base_url()))
            .json(&payload)
            .send()
            .await
            .map_err(DroidrunError::Http)?;

        Ok(resp.status().is_success())
    }

    async fn input_text_content_provider(&self, text: &str, clear: bool) -> Result<bool> {
        let encoded = base64::engine::general_purpose::STANDARD.encode(text);
        let clear_str = if clear { "true" } else { "false" };
        let cmd = format!(
            r#"content insert --uri "content://com.droidrun.portal/keyboard/input" --bind base64_text:s:"{encoded}" --bind clear:b:{clear_str}"#
        );
        self.device
            .shell(&cmd)
            .await
            .map_err(DroidrunError::Adb)?;
        Ok(true)
    }

    async fn screenshot_tcp(&self, hide_overlay: bool) -> Result<Vec<u8>> {
        let mut url = format!("{}/screenshot", self.base_url());
        if !hide_overlay {
            url.push_str("?hideOverlay=false");
        }

        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(DroidrunError::Http)?;

        if !resp.status().is_success() {
            return Err(DroidrunError::PortalCommError(format!(
                "screenshot HTTP {}",
                resp.status()
            )));
        }

        let data: Value = resp.json().await.map_err(DroidrunError::Http)?;
        if data.get("status").and_then(|v| v.as_str()) == Some("success") {
            let b64 = extract_inner_value(&data)
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .ok_or_else(|| DroidrunError::Parse("no screenshot data in response".into()))?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&b64)
                .map_err(|e| DroidrunError::Parse(format!("base64 decode error: {e}")))?;
            Ok(bytes)
        } else {
            Err(DroidrunError::PortalCommError(
                "screenshot response status != success".into(),
            ))
        }
    }

    async fn screenshot_adb(&self) -> Result<Vec<u8>> {
        let data = self
            .device
            .screencap()
            .await
            .map_err(DroidrunError::Adb)?;
        debug!("screenshot taken via ADB ({} bytes)", data.len());
        Ok(data)
    }
}

// ── Helper functions ────────────────────────────────────────────

/// Parse raw ADB content provider output to JSON.
///
/// The output format is: `Row: 0 result={json}`
pub fn parse_content_provider_output(raw: &str) -> Option<Value> {
    for line in raw.lines() {
        let line = line.trim();

        // "Row: N result={json}" format
        if let Some(json_start) = line.find("result=") {
            let json_str = &line[json_start + 7..];
            if let Ok(parsed) = serde_json::from_str::<Value>(json_str) {
                return Some(unwrap_portal_response(parsed));
            }
        }

        // Direct JSON
        if line.starts_with('{') || line.starts_with('[') {
            if let Ok(parsed) = serde_json::from_str::<Value>(line) {
                return Some(unwrap_portal_response(parsed));
            }
        }
    }

    // Last resort: entire output
    serde_json::from_str::<Value>(raw.trim())
        .ok()
        .map(unwrap_portal_response)
}

/// Unwrap Portal's `{status, result/data}` envelope.
fn unwrap_portal_response(data: Value) -> Value {
    if let Some(obj) = data.as_object() {
        // Try "result" first (new format), then "data" (legacy)
        for key in &["result", "data"] {
            if let Some(inner) = obj.get(*key) {
                // If the inner value is a JSON string, parse it
                if let Some(s) = inner.as_str() {
                    if let Ok(parsed) = serde_json::from_str::<Value>(s) {
                        return parsed;
                    }
                    // Not JSON, return as-is
                    return inner.clone();
                }
                return inner.clone();
            }
        }
    }
    data
}

/// Extract inner value from Portal response envelope.
fn extract_inner_value(data: &Value) -> Option<&Value> {
    data.as_object().and_then(|obj| {
        obj.get("result").or_else(|| obj.get("data"))
    })
}

/// Extract packages list from various response formats.
fn extract_packages_list(data: &Value) -> Option<&Vec<Value>> {
    // Direct array
    if let Some(arr) = data.as_array() {
        return Some(arr);
    }
    // Wrapped in {"packages": [...]}
    if let Some(obj) = data.as_object() {
        if let Some(pkgs) = obj.get("packages").and_then(|v| v.as_array()) {
            return Some(pkgs);
        }
        // Wrapped in result/data
        for key in &["result", "data"] {
            if let Some(inner) = obj.get(*key) {
                if let Some(arr) = inner.as_array() {
                    return Some(arr);
                }
                if let Some(inner_obj) = inner.as_object() {
                    if let Some(pkgs) = inner_obj.get("packages").and_then(|v| v.as_array()) {
                        return Some(pkgs);
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_content_provider_result_format() {
        let raw = r#"Row: 0 result={"status":"success","result":{"a11y_tree":{},"phone_state":{}}}"#;
        let parsed = parse_content_provider_output(raw).unwrap();
        assert!(parsed.get("a11y_tree").is_some());
        assert!(parsed.get("phone_state").is_some());
    }

    #[test]
    fn test_parse_content_provider_direct_json() {
        // Direct JSON without "Row: N result=" prefix goes through
        // the fallback path which also unwraps portal response envelope
        let raw = r#"{"status":"success","result":"1.2.3"}"#;
        let parsed = parse_content_provider_output(raw).unwrap();
        // The unwrap_portal_response extracts "result" -> "1.2.3"
        assert_eq!(parsed.as_str().unwrap(), "1.2.3");
    }

    #[test]
    fn test_parse_content_provider_nested_json_string() {
        let raw = r#"Row: 0 result={"status":"success","result":"{\"key\":\"value\"}"}"#;
        let parsed = parse_content_provider_output(raw).unwrap();
        assert_eq!(parsed.get("key").unwrap().as_str().unwrap(), "value");
    }

    #[test]
    fn test_parse_content_provider_empty() {
        let parsed = parse_content_provider_output("No result found.");
        assert!(parsed.is_none());
    }

    #[test]
    fn test_unwrap_portal_response_with_result() {
        let data = serde_json::json!({"status": "success", "result": {"foo": "bar"}});
        let unwrapped = unwrap_portal_response(data);
        assert_eq!(unwrapped.get("foo").unwrap().as_str().unwrap(), "bar");
    }

    #[test]
    fn test_unwrap_portal_response_with_data() {
        let data = serde_json::json!({"status": "success", "data": [1, 2, 3]});
        let unwrapped = unwrap_portal_response(data);
        assert_eq!(unwrapped.as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_unwrap_portal_response_plain() {
        let data = serde_json::json!({"foo": "bar"});
        let unwrapped = unwrap_portal_response(data.clone());
        assert_eq!(unwrapped, data);
    }

    #[test]
    fn test_extract_packages_list_direct_array() {
        let data = serde_json::json!([
            {"packageName": "com.example", "label": "Example"}
        ]);
        let list = extract_packages_list(&data).unwrap();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn test_extract_packages_list_wrapped() {
        let data = serde_json::json!({"packages": [
            {"packageName": "com.test", "label": "Test"}
        ]});
        let list = extract_packages_list(&data).unwrap();
        assert_eq!(list.len(), 1);
    }
}
