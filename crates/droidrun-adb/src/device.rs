/// ADB device — async operations against a single Android device.
use std::path::Path;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, trace, warn};

use crate::connection::AdbConnection;
use crate::error::{AdbError, Result};
use crate::models::{DeviceState, ForwardEntry};

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

    // ── Connection helpers ──────────────────────────────────────

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

    // ── Device state ────────────────────────────────────────────

    /// Get the device state (device, offline, unauthorized, etc.).
    pub async fn get_state(&self) -> Result<DeviceState> {
        let mut conn = self.connect_server().await?;
        conn.send_command(&format!("host-serial:{}:get-state", self.serial))
            .await?;
        conn.read_status().await?;
        let state_str = conn.read_length_prefixed_string().await?;
        Ok(DeviceState::from(state_str.as_str()))
    }

    // ── Shell ───────────────────────────────────────────────────

    /// Run a shell command and return stdout as a String.
    pub async fn shell(&self, cmd: &str) -> Result<String> {
        debug!("adb shell: {cmd}");
        let mut conn = self.connect_transport().await?;
        conn.send_and_okay(&format!("shell:{cmd}")).await?;
        let output = conn.read_until_close_string().await?;
        trace!("shell output ({} bytes): {}", output.len(), &output[..output.len().min(200)]);
        Ok(output)
    }

    /// Run a shell command and return stdout as raw bytes.
    pub async fn shell_bytes(&self, cmd: &str) -> Result<Vec<u8>> {
        debug!("adb shell (bytes): {cmd}");
        let mut conn = self.connect_transport().await?;
        conn.send_and_okay(&format!("shell:{cmd}")).await?;
        conn.read_until_close_bytes().await
    }

    // ── Input actions ───────────────────────────────────────────

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

    // ── Screenshots ─────────────────────────────────────────────

    /// Take a screenshot and return PNG bytes.
    pub async fn screencap(&self) -> Result<Vec<u8>> {
        debug!("taking screenshot via screencap");
        let data = self.shell_bytes("screencap -p").await?;
        if data.is_empty() {
            return Err(AdbError::ShellError("screencap returned empty data".into()));
        }
        // ADB shell may convert \n to \r\n on some devices
        // For binary data this can corrupt PNG — but screencap -p handles it
        Ok(data)
    }

    // ── App management ──────────────────────────────────────────

    /// Start an app with optional activity name.
    pub async fn app_start(&self, package: &str, activity: Option<&str>) -> Result<String> {
        let activity = match activity {
            Some(a) => a.to_string(),
            None => {
                // Resolve the main activity
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

        // Push file using exec-out (simpler than sync protocol for single files)
        self.push_file(apk_path, remote_path).await?;

        // Install
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

    /// Push a local file to the device using the sync protocol.
    async fn push_file(&self, local: &Path, remote: &str) -> Result<()> {
        debug!("pushing {} -> {remote}", local.display());

        let data = tokio::fs::read(local).await?;
        let size = data.len();

        let mut conn = self.connect_transport().await?;
        conn.send_and_okay("sync:").await?;

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
        let chunk_size = 64 * 1024; // 64KB chunks
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
                // Send QUIT to close sync session
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

    // ── Port forwarding ─────────────────────────────────────────

    /// Set up port forwarding. Returns the local port.
    ///
    /// If `local_port` is 0, the ADB server assigns a free port.
    pub async fn forward(&self, local_port: u16, remote_port: u16) -> Result<u16> {
        let mut conn = self.connect_server().await?;

        if local_port == 0 {
            // Ask ADB to allocate a port.
            // ADB protocol for forward:tcp:0 sends two responses:
            //   1st OKAY = command accepted
            //   2nd OKAY + port = allocation result
            let cmd = format!(
                "host-serial:{}:forward:tcp:0;tcp:{}",
                self.serial, remote_port
            );
            conn.send_and_okay(&cmd).await?;
            // Read the second OKAY status
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

    // ── Device info ─────────────────────────────────────────────

    /// Get the device date/time.
    pub async fn get_date(&self) -> Result<String> {
        let result = self.shell("date").await?;
        Ok(result.trim().to_string())
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
}
