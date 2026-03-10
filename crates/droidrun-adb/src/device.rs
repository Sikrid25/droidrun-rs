/// ADB device — async operations against a single Android device.
use std::path::Path;

use regex::Regex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, trace, warn};

use crate::connection::AdbConnection;
use crate::error::{AdbError, Result};
use crate::models::{
    AppDetail, CurrentApp, DeviceState, FileStat, ForwardEntry, RebootMode, ReverseEntry,
    ScreenSize, ShellOutput, SyncDirEntry,
};

/// Represents a connection to a specific Android device via the ADB server.
///
/// Each operation opens a fresh TCP connection to the ADB server, selects
/// the transport for this device, then performs the command. This matches
/// how the ADB protocol works — there is no persistent session.
#[derive(Debug, Clone)]
pub struct AdbDevice {
    host: String,
    port: u16,
    pub serial: String,
}

impl AdbDevice {
    /// Create a new device handle.
    pub fn new(serial: impl Into<String>, host: impl Into<String>, port: u16) -> Self {
        Self {
            serial: serial.into(),
            host: host.into(),
            port,
        }
    }

    /// Create a device handle using default ADB server address.
    pub fn with_serial(serial: impl Into<String>) -> Self {
        Self::new(serial, "127.0.0.1", 5037)
    }

    // ══════════════════════════════════════════════════════════════
    //  Connection helpers
    // ══════════════════════════════════════════════════════════════

    /// Open a connection to the ADB server.
    async fn connect_server(&self) -> Result<AdbConnection> {
        AdbConnection::connect(&self.host, self.port).await
    }

    /// Open a connection and select this device's transport.
    async fn connect_transport(&self) -> Result<AdbConnection> {
        let mut conn = self.connect_server().await?;
        conn.send_and_okay(&format!("host:transport:{}", self.serial))
            .await?;
        Ok(conn)
    }

    /// Open a sync protocol session (transport + sync:).
    async fn connect_sync(&self) -> Result<AdbConnection> {
        let mut conn = self.connect_transport().await?;
        conn.send_and_okay("sync:").await?;
        Ok(conn)
    }

    // ══════════════════════════════════════════════════════════════
    //  Device state & info
    // ══════════════════════════════════════════════════════════════

    /// Get the device state (device, offline, unauthorized, etc.).
    pub async fn get_state(&self) -> Result<DeviceState> {
        let mut conn = self.connect_server().await?;
        conn.send_command(&format!("host-serial:{}:get-state", self.serial))
            .await?;
        conn.read_status().await?;
        let state_str = conn.read_length_prefixed_string().await?;
        Ok(DeviceState::from(state_str.as_str()))
    }

    /// Get the real serial number of the device.
    pub async fn get_serialno(&self) -> Result<String> {
        let mut conn = self.connect_server().await?;
        conn.send_and_okay(&format!("host-serial:{}:get-serialno", self.serial))
            .await?;
        let serial = conn.read_length_prefixed_string().await?;
        Ok(serial.trim().to_string())
    }

    /// Get the device feature list (e.g., shell_v2, cmd, stat_v2).
    pub async fn get_features(&self) -> Result<Vec<String>> {
        let mut conn = self.connect_server().await?;
        conn.send_and_okay(&format!("host-serial:{}:features", self.serial))
            .await?;
        let features_str = conn.read_length_prefixed_string().await?;
        Ok(features_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect())
    }

    // ══════════════════════════════════════════════════════════════
    //  Shell
    // ══════════════════════════════════════════════════════════════

    /// Run a shell command and return stdout as a String.
    pub async fn shell(&self, cmd: &str) -> Result<String> {
        debug!("adb shell: {cmd}");
        let mut conn = self.connect_transport().await?;
        conn.send_and_okay(&format!("shell:{cmd}")).await?;
        let output = conn.read_until_close_string().await?;
        trace!(
            "shell output ({} bytes): {}",
            output.len(),
            &output[..output.len().min(200)]
        );
        Ok(output)
    }

    /// Run a shell command and return stdout as raw bytes.
    pub async fn shell_bytes(&self, cmd: &str) -> Result<Vec<u8>> {
        debug!("adb shell (bytes): {cmd}");
        let mut conn = self.connect_transport().await?;
        conn.send_and_okay(&format!("shell:{cmd}")).await?;
        conn.read_until_close_bytes().await
    }

    /// Run a shell command and return both stdout and exit code.
    ///
    /// Wraps the command in a subshell `(cmd)` so that even `exit N` won't
    /// prevent the sentinel from being printed, then appends
    /// `; echo DROIDRUN_EXIT:$?` to capture the exit code.
    pub async fn shell2(&self, cmd: &str) -> Result<ShellOutput> {
        let sentinel = "DROIDRUN_EXIT:";
        let full_cmd = format!("({cmd}); echo {sentinel}$?");
        let raw = self.shell(&full_cmd).await?;

        let (stdout, exit_code) = if let Some(pos) = raw.rfind(sentinel) {
            let code_str = raw[pos + sentinel.len()..].trim();
            let code = code_str.parse::<i32>().unwrap_or(-1);
            let stdout = raw[..pos].to_string();
            (stdout, code)
        } else {
            (raw, -1)
        };

        Ok(ShellOutput { stdout, exit_code })
    }

    // ══════════════════════════════════════════════════════════════
    //  System properties
    // ══════════════════════════════════════════════════════════════

    /// Get a system property by name.
    pub async fn getprop(&self, name: &str) -> Result<String> {
        let output = self.shell(&format!("getprop {name}")).await?;
        Ok(output.trim().to_string())
    }

    /// Get the device model (ro.product.model).
    pub async fn prop_model(&self) -> Result<String> {
        self.getprop("ro.product.model").await
    }

    /// Get the device name (ro.product.name).
    pub async fn prop_name(&self) -> Result<String> {
        self.getprop("ro.product.name").await
    }

    /// Get the device codename (ro.product.device).
    pub async fn prop_device(&self) -> Result<String> {
        self.getprop("ro.product.device").await
    }

    // ══════════════════════════════════════════════════════════════
    //  Input actions
    // ══════════════════════════════════════════════════════════════

    /// Tap at screen coordinates.
    pub async fn tap(&self, x: i32, y: i32) -> Result<()> {
        self.shell(&format!("input tap {x} {y}")).await?;
        Ok(())
    }

    /// Swipe from (x1, y1) to (x2, y2) over duration_ms milliseconds.
    pub async fn swipe(
        &self,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        duration_ms: u32,
    ) -> Result<()> {
        self.shell(&format!("input swipe {x1} {y1} {x2} {y2} {duration_ms}"))
            .await?;
        Ok(())
    }

    /// Send a key event.
    pub async fn keyevent(&self, keycode: i32) -> Result<()> {
        self.shell(&format!("input keyevent {keycode}")).await?;
        Ok(())
    }

    /// Drag from (sx, sy) to (ex, ey) over duration_ms milliseconds.
    pub async fn drag(
        &self,
        sx: i32,
        sy: i32,
        ex: i32,
        ey: i32,
        duration_ms: u32,
    ) -> Result<()> {
        self.shell(&format!("input draganddrop {sx} {sy} {ex} {ey} {duration_ms}"))
            .await?;
        Ok(())
    }

    /// Type text using ADB input method.
    ///
    /// Note: For reliable Unicode text input, use droidrun-core's Portal keyboard.
    /// This method escapes special shell characters but cannot handle all Unicode.
    pub async fn input_text(&self, text: &str) -> Result<()> {
        let escaped = text
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace(' ', "%s")
            .replace('&', "\\&")
            .replace('<', "\\<")
            .replace('>', "\\>")
            .replace('\'', "\\'");
        self.shell(&format!("input text \"{escaped}\"")).await?;
        Ok(())
    }

    // ══════════════════════════════════════════════════════════════
    //  Screenshots
    // ══════════════════════════════════════════════════════════════

    /// Take a screenshot and return PNG bytes.
    pub async fn screencap(&self) -> Result<Vec<u8>> {
        debug!("taking screenshot via screencap");
        let data = self.shell_bytes("screencap -p").await?;
        if data.is_empty() {
            return Err(AdbError::ShellError(
                "screencap returned empty data".into(),
            ));
        }
        Ok(data)
    }

    // ══════════════════════════════════════════════════════════════
    //  App management
    // ══════════════════════════════════════════════════════════════

    /// Start an app with optional activity name.
    pub async fn app_start(&self, package: &str, activity: Option<&str>) -> Result<String> {
        let activity = match activity {
            Some(a) => a.to_string(),
            None => {
                let output = self
                    .shell(&format!("cmd package resolve-activity --brief {package}"))
                    .await?;
                let lines: Vec<&str> = output.lines().collect();
                if lines.len() < 2 {
                    return Err(AdbError::ShellError(format!(
                        "cannot resolve main activity for {package}"
                    )));
                }
                let full = lines[1].trim();
                match full.split_once('/') {
                    Some((_, act)) => act.to_string(),
                    None => full.to_string(),
                }
            }
        };

        debug!("starting {package}/{activity}");
        let result = self
            .shell(&format!("am start -n {package}/{activity}"))
            .await?;
        Ok(result)
    }

    /// Force stop an app.
    pub async fn app_stop(&self, package: &str) -> Result<()> {
        debug!("stopping {package}");
        self.shell(&format!("am force-stop {package}")).await?;
        Ok(())
    }

    /// Clear app data and cache.
    pub async fn app_clear(&self, package: &str) -> Result<String> {
        debug!("clearing {package}");
        let output = self.shell(&format!("pm clear {package}")).await?;
        Ok(output.trim().to_string())
    }

    /// Get the current foreground app.
    pub async fn app_current(&self) -> Result<CurrentApp> {
        let output = self
            .shell("dumpsys activity activities | grep -E 'mResumedActivity|mCurrentFocus'")
            .await?;

        // Try mResumedActivity first, then mCurrentFocus
        let re = Regex::new(r"(\S+)/(\S+)\s").unwrap();
        for line in output.lines() {
            if let Some(caps) = re.captures(line) {
                let full = caps.get(1).unwrap().as_str();
                let activity = caps.get(2).unwrap().as_str();
                // Remove trailing } or spaces
                let activity = activity.trim_end_matches('}').trim_end();
                return Ok(CurrentApp {
                    package: full.to_string(),
                    activity: activity.to_string(),
                });
            }
        }

        // Fallback: try to parse differently
        let re2 = Regex::new(r"([a-zA-Z0-9_.]+)/([a-zA-Z0-9_.]+)").unwrap();
        for line in output.lines() {
            if line.contains("mResumedActivity") || line.contains("mCurrentFocus") {
                if let Some(caps) = re2.captures(line) {
                    return Ok(CurrentApp {
                        package: caps.get(1).unwrap().as_str().to_string(),
                        activity: caps.get(2).unwrap().as_str().to_string(),
                    });
                }
            }
        }

        Err(AdbError::Parse(
            "cannot determine current foreground app".into(),
        ))
    }

    /// Get detailed info about an installed app.
    pub async fn app_info(&self, package: &str) -> Result<AppDetail> {
        let output = self
            .shell(&format!("dumpsys package {package}"))
            .await?;

        let mut detail = AppDetail {
            package: package.to_string(),
            version_name: None,
            version_code: None,
            install_path: None,
            first_install_time: None,
            last_update_time: None,
        };

        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("versionName=") {
                detail.version_name = Some(trimmed.trim_start_matches("versionName=").to_string());
            } else if trimmed.starts_with("versionCode=") {
                // Format: "versionCode=123 minSdk=..."
                let val = trimmed
                    .trim_start_matches("versionCode=")
                    .split_whitespace()
                    .next()
                    .unwrap_or("0");
                detail.version_code = val.parse().ok();
            } else if trimmed.starts_with("codePath=") {
                detail.install_path = Some(trimmed.trim_start_matches("codePath=").to_string());
            } else if trimmed.starts_with("firstInstallTime=") {
                detail.first_install_time =
                    Some(trimmed.trim_start_matches("firstInstallTime=").to_string());
            } else if trimmed.starts_with("lastUpdateTime=") {
                detail.last_update_time =
                    Some(trimmed.trim_start_matches("lastUpdateTime=").to_string());
            }
        }

        Ok(detail)
    }

    /// Install an APK on the device.
    ///
    /// Pushes the APK to `/data/local/tmp/`, runs `pm install`, then removes it.
    pub async fn install(&self, apk_path: &Path, flags: &[&str]) -> Result<String> {
        if !apk_path.exists() {
            return Err(AdbError::InstallFailed(format!(
                "APK not found: {}",
                apk_path.display()
            )));
        }

        let remote_path = "/data/local/tmp/_droidrun_install.apk";

        self.push_file(apk_path, remote_path).await?;

        let flag_str = if flags.is_empty() {
            String::new()
        } else {
            format!(" {}", flags.join(" "))
        };
        let result = self
            .shell(&format!("pm install{flag_str} {remote_path}"))
            .await?;

        // Cleanup
        let _ = self.shell(&format!("rm -f {remote_path}")).await;

        if result.contains("Success") {
            debug!("install succeeded");
            Ok(result.trim().to_string())
        } else {
            Err(AdbError::InstallFailed(result.trim().to_string()))
        }
    }

    /// Uninstall an app.
    pub async fn uninstall(&self, package: &str) -> Result<String> {
        debug!("uninstalling {package}");
        let output = self.shell(&format!("pm uninstall {package}")).await?;
        if output.contains("Success") {
            Ok(output.trim().to_string())
        } else {
            Err(AdbError::UninstallFailed(output.trim().to_string()))
        }
    }

    /// List installed packages.
    pub async fn list_packages(&self, flags: &[&str]) -> Result<Vec<String>> {
        let flag_str = if flags.is_empty() {
            String::new()
        } else {
            format!(" {}", flags.join(" "))
        };
        let output = self
            .shell(&format!("pm list packages{flag_str}"))
            .await?;
        Ok(output
            .lines()
            .filter_map(|l| l.strip_prefix("package:"))
            .map(|s| s.trim().to_string())
            .collect())
    }

    // ══════════════════════════════════════════════════════════════
    //  Port forwarding (host-to-device)
    // ══════════════════════════════════════════════════════════════

    /// Set up port forwarding. Returns the local port.
    ///
    /// If `local_port` is 0, the ADB server assigns a free port.
    pub async fn forward(&self, local_port: u16, remote_port: u16) -> Result<u16> {
        let mut conn = self.connect_server().await?;

        if local_port == 0 {
            let cmd = format!(
                "host-serial:{}:forward:tcp:0;tcp:{}",
                self.serial, remote_port
            );
            conn.send_and_okay(&cmd).await?;
            // Read the second OKAY status (double-OKAY protocol)
            conn.read_status().await?;
            let port_str = conn.read_length_prefixed_string().await?;
            let port = port_str
                .trim()
                .parse::<u16>()
                .map_err(|_| AdbError::Parse(format!("cannot parse port: {port_str}")))?;
            debug!("forwarded tcp:{port} -> tcp:{remote_port}");
            Ok(port)
        } else {
            let cmd = format!(
                "host-serial:{}:forward:tcp:{};tcp:{}",
                self.serial, local_port, remote_port
            );
            conn.send_and_okay(&cmd).await?;
            debug!("forwarded tcp:{local_port} -> tcp:{remote_port}");
            Ok(local_port)
        }
    }

    /// List all port forwards for this device.
    pub async fn forward_list(&self) -> Result<Vec<ForwardEntry>> {
        let mut conn = self.connect_server().await?;
        conn.send_and_okay("host:list-forward").await?;
        let data = conn.read_length_prefixed_string().await?;

        let entries: Vec<ForwardEntry> = data
            .lines()
            .filter(|l| !l.is_empty())
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    Some(ForwardEntry {
                        serial: parts[0].to_string(),
                        local: parts[1].to_string(),
                        remote: parts[2].to_string(),
                    })
                } else {
                    warn!("cannot parse forward entry: {line}");
                    None
                }
            })
            .filter(|e| e.serial == self.serial)
            .collect();

        Ok(entries)
    }

    /// Remove a specific port forward.
    pub async fn forward_remove(&self, local_port: u16) -> Result<()> {
        let mut conn = self.connect_server().await?;
        let cmd = format!(
            "host-serial:{}:killforward:tcp:{}",
            self.serial, local_port
        );
        conn.send_and_okay(&cmd).await?;
        Ok(())
    }

    /// Remove all port forwards for this device.
    pub async fn forward_remove_all(&self) -> Result<()> {
        let mut conn = self.connect_server().await?;
        let cmd = format!("host-serial:{}:killforward-all", self.serial);
        conn.send_and_okay(&cmd).await?;
        Ok(())
    }

    // ══════════════════════════════════════════════════════════════
    //  Reverse port forwarding (device-to-host)
    // ══════════════════════════════════════════════════════════════

    /// Set up reverse port forwarding (device → host).
    pub async fn reverse(&self, remote_port: u16, local_port: u16) -> Result<()> {
        let mut conn = self.connect_transport().await?;
        conn.send_and_okay(&format!(
            "reverse:forward:tcp:{remote_port};tcp:{local_port}"
        ))
        .await?;
        debug!("reverse: device tcp:{remote_port} -> host tcp:{local_port}");
        Ok(())
    }

    /// List all reverse port forwards.
    pub async fn reverse_list(&self) -> Result<Vec<ReverseEntry>> {
        let mut conn = self.connect_transport().await?;
        conn.send_and_okay("reverse:list-forward").await?;
        let data = conn.read_length_prefixed_string().await?;

        let entries: Vec<ReverseEntry> = data
            .lines()
            .filter(|l| !l.is_empty())
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                // Format varies: sometimes "host remote local", sometimes just "remote local"
                if parts.len() >= 3 {
                    Some(ReverseEntry {
                        remote: parts[1].to_string(),
                        local: parts[2].to_string(),
                    })
                } else if parts.len() == 2 {
                    Some(ReverseEntry {
                        remote: parts[0].to_string(),
                        local: parts[1].to_string(),
                    })
                } else {
                    warn!("cannot parse reverse entry: {line}");
                    None
                }
            })
            .collect();

        Ok(entries)
    }

    /// Remove a specific reverse forward.
    pub async fn reverse_remove(&self, remote_port: u16) -> Result<()> {
        let mut conn = self.connect_transport().await?;
        conn.send_and_okay(&format!("reverse:killforward:tcp:{remote_port}"))
            .await?;
        Ok(())
    }

    /// Remove all reverse forwards.
    pub async fn reverse_remove_all(&self) -> Result<()> {
        let mut conn = self.connect_transport().await?;
        conn.send_and_okay("reverse:killforward-all").await?;
        Ok(())
    }

    // ══════════════════════════════════════════════════════════════
    //  System commands (ADB protocol level)
    // ══════════════════════════════════════════════════════════════

    /// Restart adbd as root. Only works on userdebug/eng builds or emulators.
    pub async fn root(&self) -> Result<String> {
        let mut conn = self.connect_transport().await?;
        conn.send_and_okay("root:").await?;
        let response = conn.read_until_close_string().await?;
        debug!("root: {}", response.trim());
        Ok(response.trim().to_string())
    }

    /// Switch adbd to TCP/IP mode on the given port.
    pub async fn tcpip(&self, port: u16) -> Result<String> {
        let mut conn = self.connect_transport().await?;
        conn.send_and_okay(&format!("tcpip:{port}")).await?;
        let response = conn.read_until_close_string().await?;
        debug!("tcpip: {}", response.trim());
        Ok(response.trim().to_string())
    }

    /// Reboot the device.
    pub async fn reboot(&self, mode: RebootMode) -> Result<()> {
        let mut conn = self.connect_transport().await?;
        let cmd = match mode {
            RebootMode::Normal => "reboot:".to_string(),
            other => format!("reboot:{}", other.as_str()),
        };
        conn.send_and_okay(&cmd).await?;
        debug!("reboot: {:?}", mode);
        Ok(())
    }

    // ══════════════════════════════════════════════════════════════
    //  File operations (sync protocol)
    // ══════════════════════════════════════════════════════════════

    /// Push a local file to the device using the sync protocol.
    pub async fn push(&self, local_path: &Path, remote_path: &str) -> Result<()> {
        self.push_file(local_path, remote_path).await
    }

    /// Internal push implementation using sync protocol.
    async fn push_file(&self, local: &Path, remote: &str) -> Result<()> {
        debug!("pushing {} -> {remote}", local.display());

        let data = tokio::fs::read(local).await?;
        let size = data.len();

        let mut conn = self.connect_sync().await?;
        let stream = conn.stream_mut();

        // SEND command: "SEND" + length-prefixed path with permissions
        let path_with_mode = format!("{remote},33188"); // 0o100644
        let path_bytes = path_with_mode.as_bytes();
        stream.write_all(b"SEND").await?;
        stream
            .write_all(&(path_bytes.len() as u32).to_le_bytes())
            .await?;
        stream.write_all(path_bytes).await?;

        // Send data in chunks
        let chunk_size = 64 * 1024;
        for chunk in data.chunks(chunk_size) {
            stream.write_all(b"DATA").await?;
            stream
                .write_all(&(chunk.len() as u32).to_le_bytes())
                .await?;
            stream.write_all(chunk).await?;
        }

        // DONE command with mtime
        let mtime = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as u32;
        stream.write_all(b"DONE").await?;
        stream.write_all(&mtime.to_le_bytes()).await?;

        // Read response
        let mut status = [0u8; 4];
        stream.read_exact(&mut status).await?;
        match &status {
            b"OKAY" => {
                debug!("pushed {size} bytes to {remote}");
                stream.write_all(b"QUIT").await?;
                stream.write_all(&0u32.to_le_bytes()).await?;
                Ok(())
            }
            b"FAIL" => {
                let mut len_buf = [0u8; 4];
                stream.read_exact(&mut len_buf).await?;
                let len = u32::from_le_bytes(len_buf) as usize;
                let mut msg_buf = vec![0u8; len];
                stream.read_exact(&mut msg_buf).await?;
                let msg = String::from_utf8_lossy(&msg_buf);
                Err(AdbError::ServerFailed(format!("push failed: {msg}")))
            }
            _ => Err(AdbError::Protocol(format!(
                "unexpected sync response: {:?}",
                status
            ))),
        }
    }

    /// Push raw bytes to a file on the device.
    pub async fn push_bytes(&self, data: &[u8], remote_path: &str) -> Result<()> {
        debug!("pushing {} bytes -> {remote_path}", data.len());

        let mut conn = self.connect_sync().await?;
        let stream = conn.stream_mut();

        let path_with_mode = format!("{remote_path},33188");
        let path_bytes = path_with_mode.as_bytes();
        stream.write_all(b"SEND").await?;
        stream
            .write_all(&(path_bytes.len() as u32).to_le_bytes())
            .await?;
        stream.write_all(path_bytes).await?;

        let chunk_size = 64 * 1024;
        for chunk in data.chunks(chunk_size) {
            stream.write_all(b"DATA").await?;
            stream
                .write_all(&(chunk.len() as u32).to_le_bytes())
                .await?;
            stream.write_all(chunk).await?;
        }

        let mtime = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as u32;
        stream.write_all(b"DONE").await?;
        stream.write_all(&mtime.to_le_bytes()).await?;

        let mut status = [0u8; 4];
        stream.read_exact(&mut status).await?;
        match &status {
            b"OKAY" => {
                stream.write_all(b"QUIT").await?;
                stream.write_all(&0u32.to_le_bytes()).await?;
                Ok(())
            }
            b"FAIL" => {
                let mut len_buf = [0u8; 4];
                stream.read_exact(&mut len_buf).await?;
                let len = u32::from_le_bytes(len_buf) as usize;
                let mut msg_buf = vec![0u8; len];
                stream.read_exact(&mut msg_buf).await?;
                Err(AdbError::SyncError(
                    String::from_utf8_lossy(&msg_buf).to_string(),
                ))
            }
            _ => Err(AdbError::Protocol(format!(
                "unexpected sync response: {:?}",
                status
            ))),
        }
    }

    /// Pull a file from the device and return its contents as bytes.
    pub async fn pull_bytes(&self, remote_path: &str) -> Result<Vec<u8>> {
        debug!("pulling {remote_path}");

        let mut conn = self.connect_sync().await?;
        let stream = conn.stream_mut();

        let path_bytes = remote_path.as_bytes();
        stream.write_all(b"RECV").await?;
        stream
            .write_all(&(path_bytes.len() as u32).to_le_bytes())
            .await?;
        stream.write_all(path_bytes).await?;

        let mut data = Vec::new();
        loop {
            let mut id = [0u8; 4];
            stream.read_exact(&mut id).await?;

            match &id {
                b"DATA" => {
                    let mut len_buf = [0u8; 4];
                    stream.read_exact(&mut len_buf).await?;
                    let chunk_len = u32::from_le_bytes(len_buf) as usize;
                    if chunk_len > 0 {
                        let mut chunk = vec![0u8; chunk_len];
                        stream.read_exact(&mut chunk).await?;
                        data.extend_from_slice(&chunk);
                    }
                }
                b"DONE" => {
                    let mut _trailing = [0u8; 4];
                    stream.read_exact(&mut _trailing).await?;
                    break;
                }
                b"FAIL" => {
                    let mut len_buf = [0u8; 4];
                    stream.read_exact(&mut len_buf).await?;
                    let msg_len = u32::from_le_bytes(len_buf) as usize;
                    let mut msg_buf = vec![0u8; msg_len];
                    stream.read_exact(&mut msg_buf).await?;
                    return Err(AdbError::SyncError(
                        String::from_utf8_lossy(&msg_buf).to_string(),
                    ));
                }
                _ => {
                    return Err(AdbError::SyncError(format!(
                        "unexpected sync response: {:?}",
                        String::from_utf8_lossy(&id)
                    )));
                }
            }
        }

        // QUIT
        stream.write_all(b"QUIT").await?;
        stream.write_all(&0u32.to_le_bytes()).await?;

        debug!("pulled {} bytes from {remote_path}", data.len());
        Ok(data)
    }

    /// Pull a file from the device to a local path.
    pub async fn pull(&self, remote_path: &str, local_path: &Path) -> Result<()> {
        let data = self.pull_bytes(remote_path).await?;
        tokio::fs::write(local_path, &data).await?;
        debug!(
            "saved {} bytes to {}",
            data.len(),
            local_path.display()
        );
        Ok(())
    }

    /// Get file metadata via sync STAT protocol.
    pub async fn stat(&self, path: &str) -> Result<FileStat> {
        let mut conn = self.connect_sync().await?;
        let stream = conn.stream_mut();

        let path_bytes = path.as_bytes();
        stream.write_all(b"STAT").await?;
        stream
            .write_all(&(path_bytes.len() as u32).to_le_bytes())
            .await?;
        stream.write_all(path_bytes).await?;

        let mut header = [0u8; 4];
        stream.read_exact(&mut header).await?;
        if &header != b"STAT" {
            return Err(AdbError::SyncError(format!(
                "expected STAT, got {:?}",
                String::from_utf8_lossy(&header)
            )));
        }

        let mut buf = [0u8; 12];
        stream.read_exact(&mut buf).await?;
        let mode = u32::from_le_bytes(buf[0..4].try_into().unwrap());
        let size = u32::from_le_bytes(buf[4..8].try_into().unwrap());
        let mtime = u32::from_le_bytes(buf[8..12].try_into().unwrap());

        // QUIT
        stream.write_all(b"QUIT").await?;
        stream.write_all(&0u32.to_le_bytes()).await?;

        Ok(FileStat { mode, size, mtime })
    }

    /// List directory contents via sync LIST protocol.
    pub async fn list_dir(&self, path: &str) -> Result<Vec<SyncDirEntry>> {
        let mut conn = self.connect_sync().await?;
        let stream = conn.stream_mut();

        let path_bytes = path.as_bytes();
        stream.write_all(b"LIST").await?;
        stream
            .write_all(&(path_bytes.len() as u32).to_le_bytes())
            .await?;
        stream.write_all(path_bytes).await?;

        let mut entries = Vec::new();
        loop {
            let mut id = [0u8; 4];
            stream.read_exact(&mut id).await?;

            if &id == b"DONE" {
                let mut _zero = [0u8; 4];
                stream.read_exact(&mut _zero).await?;
                break;
            }

            if &id != b"DENT" {
                return Err(AdbError::SyncError(format!(
                    "expected DENT/DONE, got {:?}",
                    String::from_utf8_lossy(&id)
                )));
            }

            // DENT: mode(4) + size(4) + mtime(4) + namelen(4) + name(namelen)
            let mut meta = [0u8; 16];
            stream.read_exact(&mut meta).await?;
            let mode = u32::from_le_bytes(meta[0..4].try_into().unwrap());
            let size = u32::from_le_bytes(meta[4..8].try_into().unwrap());
            let mtime = u32::from_le_bytes(meta[8..12].try_into().unwrap());
            let namelen = u32::from_le_bytes(meta[12..16].try_into().unwrap()) as usize;

            let mut name_buf = vec![0u8; namelen];
            stream.read_exact(&mut name_buf).await?;
            let name = String::from_utf8_lossy(&name_buf).to_string();

            if name != "." && name != ".." {
                entries.push(SyncDirEntry {
                    name,
                    mode,
                    size,
                    mtime,
                });
            }
        }

        // QUIT
        stream.write_all(b"QUIT").await?;
        stream.write_all(&0u32.to_le_bytes()).await?;

        debug!("listed {} entries in {path}", entries.len());
        Ok(entries)
    }

    // ══════════════════════════════════════════════════════════════
    //  File operations (shell-based)
    // ══════════════════════════════════════════════════════════════

    /// Check if a file or directory exists on the device.
    pub async fn exists(&self, path: &str) -> Result<bool> {
        let output = self
            .shell(&format!("[ -e '{path}' ] && echo 1 || echo 0"))
            .await?;
        Ok(output.trim() == "1")
    }

    /// Delete a file on the device.
    pub async fn remove(&self, path: &str) -> Result<()> {
        self.shell(&format!("rm -f '{path}'")).await?;
        Ok(())
    }

    /// Delete a directory recursively on the device.
    pub async fn rmtree(&self, path: &str) -> Result<()> {
        self.shell(&format!("rm -rf '{path}'")).await?;
        Ok(())
    }

    // ══════════════════════════════════════════════════════════════
    //  Screen & display
    // ══════════════════════════════════════════════════════════════

    /// Get screen dimensions.
    pub async fn window_size(&self) -> Result<ScreenSize> {
        let output = self.shell("wm size").await?;
        let re = Regex::new(r"(\d+)x(\d+)").unwrap();
        if let Some(caps) = re.captures(&output) {
            let width = caps[1]
                .parse()
                .map_err(|_| AdbError::Parse("width".into()))?;
            let height = caps[2]
                .parse()
                .map_err(|_| AdbError::Parse("height".into()))?;
            Ok(ScreenSize { width, height })
        } else {
            Err(AdbError::Parse(format!(
                "cannot parse wm size output: {output}"
            )))
        }
    }

    /// Get current screen rotation (0=natural, 1=left, 2=inverted, 3=right).
    pub async fn rotation(&self) -> Result<u8> {
        let output = self
            .shell("dumpsys input | grep SurfaceOrientation")
            .await?;
        if let Some(digit) = output.chars().rev().find(|c| c.is_ascii_digit()) {
            Ok(digit.to_digit(10).unwrap_or(0) as u8)
        } else {
            Ok(0)
        }
    }

    /// Check if the screen is currently on.
    pub async fn is_screen_on(&self) -> Result<bool> {
        let output = self
            .shell("dumpsys power | grep mWakefulness")
            .await?;
        Ok(output.contains("Awake"))
    }

    /// Turn screen on or off.
    pub async fn switch_screen(&self, on: bool) -> Result<()> {
        let currently_on = self.is_screen_on().await?;
        if currently_on != on {
            self.keyevent(26).await?; // KEYCODE_POWER
        }
        Ok(())
    }

    // ══════════════════════════════════════════════════════════════
    //  Network info
    // ══════════════════════════════════════════════════════════════

    /// Get the device's WLAN IP address.
    pub async fn wlan_ip(&self) -> Result<String> {
        let output = self
            .shell("ip addr show wlan0 | grep 'inet '")
            .await?;
        let re = Regex::new(r"inet (\d+\.\d+\.\d+\.\d+)").unwrap();
        if let Some(caps) = re.captures(&output) {
            Ok(caps[1].to_string())
        } else {
            Err(AdbError::ShellError("no wlan0 IP found".into()))
        }
    }

    // ══════════════════════════════════════════════════════════════
    //  Device info (misc)
    // ══════════════════════════════════════════════════════════════

    /// Get the device date/time.
    pub async fn get_date(&self) -> Result<String> {
        let result = self.shell("date").await?;
        Ok(result.trim().to_string())
    }

    // ══════════════════════════════════════════════════════════════
    //  Logcat
    // ══════════════════════════════════════════════════════════════

    /// Stream logcat output. Returns an mpsc receiver.
    ///
    /// The stream runs in a background task until the receiver is dropped.
    pub async fn logcat(
        &self,
        filter: Option<&str>,
    ) -> Result<tokio::sync::mpsc::Receiver<String>> {
        let cmd = match filter {
            Some(f) => format!("logcat {f}"),
            None => "logcat".to_string(),
        };

        let mut conn = self.connect_transport().await?;
        conn.send_and_okay(&format!("shell:{cmd}")).await?;

        let (tx, rx) = tokio::sync::mpsc::channel(256);
        let stream = conn.into_stream();

        tokio::spawn(async move {
            use tokio::io::{AsyncBufReadExt, BufReader};
            let reader = BufReader::new(stream);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if tx.send(line).await.is_err() {
                    break;
                }
            }
        });

        debug!("logcat stream started");
        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_creation() {
        let d = AdbDevice::with_serial("emulator-5554");
        assert_eq!(d.serial, "emulator-5554");
        assert_eq!(d.host, "127.0.0.1");
        assert_eq!(d.port, 5037);
    }

    #[test]
    fn test_device_custom_host() {
        let d = AdbDevice::new("device123", "192.168.1.100", 5038);
        assert_eq!(d.serial, "device123");
        assert_eq!(d.host, "192.168.1.100");
        assert_eq!(d.port, 5038);
    }

    #[test]
    fn test_parse_shell2_output() {
        let raw = "hello world\nDROIDRUN_EXIT:0\n";
        let sentinel = "DROIDRUN_EXIT:";
        let (stdout, exit_code) = if let Some(pos) = raw.rfind(sentinel) {
            let code_str = raw[pos + sentinel.len()..].trim();
            let code = code_str.parse::<i32>().unwrap_or(-1);
            let stdout = raw[..pos].to_string();
            (stdout, code)
        } else {
            (raw.to_string(), -1)
        };
        assert_eq!(stdout, "hello world\n");
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_parse_shell2_failure() {
        let raw = "error: not found\nDROIDRUN_EXIT:1\n";
        let sentinel = "DROIDRUN_EXIT:";
        let (stdout, exit_code) = if let Some(pos) = raw.rfind(sentinel) {
            let code_str = raw[pos + sentinel.len()..].trim();
            let code = code_str.parse::<i32>().unwrap_or(-1);
            let stdout = raw[..pos].to_string();
            (stdout, code)
        } else {
            (raw.to_string(), -1)
        };
        assert_eq!(stdout, "error: not found\n");
        assert_eq!(exit_code, 1);
    }

    #[test]
    fn test_parse_wm_size() {
        let output = "Physical size: 1080x1920\n";
        let re = Regex::new(r"(\d+)x(\d+)").unwrap();
        let caps = re.captures(output).unwrap();
        let width: u32 = caps[1].parse().unwrap();
        let height: u32 = caps[2].parse().unwrap();
        assert_eq!(width, 1080);
        assert_eq!(height, 1920);
    }

    #[test]
    fn test_parse_wm_size_override() {
        let output = "Physical size: 1440x2960\nOverride size: 1080x2220\n";
        let re = Regex::new(r"(\d+)x(\d+)").unwrap();
        let caps = re.captures(output).unwrap();
        let width: u32 = caps[1].parse().unwrap();
        let height: u32 = caps[2].parse().unwrap();
        assert_eq!(width, 1440);
        assert_eq!(height, 2960);
    }

    #[test]
    fn test_parse_current_app() {
        let output =
            "    mResumedActivity: ActivityRecord{abcdef u0 com.example.app/.MainActivity t1}\n";
        let re = Regex::new(r"([a-zA-Z0-9_.]+)/([a-zA-Z0-9_.]+)").unwrap();
        let caps = re.captures(output).unwrap();
        assert_eq!(&caps[1], "com.example.app");
        assert_eq!(&caps[2], ".MainActivity");
    }

    #[test]
    fn test_parse_app_info() {
        let output = "  versionName=1.2.3\n  versionCode=42 minSdk=24\n  codePath=/data/app/com.example\n  firstInstallTime=2024-01-01\n  lastUpdateTime=2024-06-15\n";
        let mut version_name = None;
        let mut version_code = None;
        let mut install_path = None;

        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("versionName=") {
                version_name = Some(trimmed.trim_start_matches("versionName=").to_string());
            } else if trimmed.starts_with("versionCode=") {
                let val = trimmed
                    .trim_start_matches("versionCode=")
                    .split_whitespace()
                    .next()
                    .unwrap_or("0");
                version_code = val.parse::<i64>().ok();
            } else if trimmed.starts_with("codePath=") {
                install_path = Some(trimmed.trim_start_matches("codePath=").to_string());
            }
        }

        assert_eq!(version_name.as_deref(), Some("1.2.3"));
        assert_eq!(version_code, Some(42));
        assert_eq!(install_path.as_deref(), Some("/data/app/com.example"));
    }

    #[test]
    fn test_parse_wlan_ip() {
        let output = "    inet 192.168.1.42/24 brd 192.168.1.255 scope global wlan0\n";
        let re = Regex::new(r"inet (\d+\.\d+\.\d+\.\d+)").unwrap();
        let caps = re.captures(output).unwrap();
        assert_eq!(&caps[1], "192.168.1.42");
    }

    #[test]
    fn test_parse_screen_on() {
        let output = "  mWakefulness=Awake\n";
        assert!(output.contains("Awake"));

        let output2 = "  mWakefulness=Asleep\n";
        assert!(!output2.contains("Awake"));
    }
}
