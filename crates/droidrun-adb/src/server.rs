/// ADB server — discovery, device enumeration, and server lifecycle management.
use tracing::debug;

use crate::connection::AdbConnection;
use crate::device::AdbDevice;
use crate::error::{AdbError, Result};
use crate::models::{DeviceEvent, DeviceInfo, DeviceState, ForwardEntry};

/// ADB server connection for device discovery and management.
#[derive(Debug, Clone)]
pub struct AdbServer {
    host: String,
    port: u16,
}

impl Default for AdbServer {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 5037,
        }
    }
}

impl AdbServer {
    /// Create a server connection with custom address.
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
        }
    }

    // ── Device discovery ──────────────────────────────────────────

    /// List all connected devices.
    pub async fn devices(&self) -> Result<Vec<DeviceInfo>> {
        let mut conn = AdbConnection::connect(&self.host, self.port).await?;
        conn.send_and_okay("host:devices").await?;
        let data = conn.read_length_prefixed_string().await?;

        let devices: Vec<DeviceInfo> = data
            .lines()
            .filter(|l| !l.is_empty())
            .filter_map(|line| {
                let mut parts = line.split_whitespace();
                let serial = parts.next()?.to_string();
                let state_str = parts.next().unwrap_or("unknown");
                Some(DeviceInfo {
                    serial,
                    state: DeviceState::from(state_str),
                })
            })
            .collect();

        debug!("found {} device(s)", devices.len());
        Ok(devices)
    }

    /// Get a handle to the first connected device.
    pub async fn device(&self) -> Result<AdbDevice> {
        let devices = self.devices().await?;
        let info = devices
            .into_iter()
            .find(|d| d.state.is_online())
            .ok_or(AdbError::NoDevice)?;
        Ok(AdbDevice::new(info.serial, &self.host, self.port))
    }

    /// Get a handle to a specific device by serial.
    pub async fn device_by_serial(&self, serial: &str) -> Result<AdbDevice> {
        let devices = self.devices().await?;
        let found = devices.iter().any(|d| d.serial == serial);
        if found {
            Ok(AdbDevice::new(serial, &self.host, self.port))
        } else {
            Err(AdbError::DeviceNotFound(serial.to_string()))
        }
    }

    /// Get a device — by serial if provided, otherwise the first available.
    pub async fn resolve_device(&self, serial: Option<&str>) -> Result<AdbDevice> {
        match serial {
            Some(s) => self.device_by_serial(s).await,
            None => self.device().await,
        }
    }

    // ── Server lifecycle ──────────────────────────────────────────

    /// Get the ADB server version.
    pub async fn version(&self) -> Result<u32> {
        let mut conn = AdbConnection::connect(&self.host, self.port).await?;
        conn.send_and_okay("host:version").await?;
        let version_str = conn.read_length_prefixed_string().await?;
        let version = u32::from_str_radix(version_str.trim(), 16)
            .map_err(|_| AdbError::Parse(format!("cannot parse version: {version_str}")))?;
        Ok(version)
    }

    /// Kill the ADB server.
    pub async fn server_kill(&self) -> Result<()> {
        let mut conn = AdbConnection::connect(&self.host, self.port).await?;
        conn.send_and_okay("host:kill").await?;
        debug!("ADB server killed");
        Ok(())
    }

    // ── Remote device management ─────────────────────────────────

    /// Connect to a remote device via TCP/IP (adb connect host:port).
    pub async fn connect_device(&self, addr: &str) -> Result<String> {
        let mut conn = AdbConnection::connect(&self.host, self.port).await?;
        conn.send_and_okay(&format!("host:connect:{addr}")).await?;
        let response = conn.read_length_prefixed_string().await?;
        debug!("connect {addr}: {response}");
        Ok(response)
    }

    /// Disconnect from a remote device (adb disconnect host:port).
    pub async fn disconnect_device(&self, addr: &str) -> Result<String> {
        let mut conn = AdbConnection::connect(&self.host, self.port).await?;
        conn.send_and_okay(&format!("host:disconnect:{addr}")).await?;
        let response = conn.read_length_prefixed_string().await?;
        debug!("disconnect {addr}: {response}");
        Ok(response)
    }

    // ── Wait & Track ──────────────────────────────────────────────

    /// Block until a device reaches the specified state, or time out.
    ///
    /// `state` can be "device", "recovery", "bootloader", etc.
    pub async fn wait_for(
        &self,
        serial: Option<&str>,
        state: &str,
        timeout: std::time::Duration,
    ) -> Result<()> {
        let cmd = match serial {
            Some(s) => format!("host-serial:{s}:wait-for-any-{state}"),
            None => format!("host:wait-for-any-{state}"),
        };
        let host = self.host.clone();
        let port = self.port;

        let fut = async move {
            let mut conn = AdbConnection::connect(&host, port).await?;
            conn.send_and_okay(&cmd).await?;
            // The server blocks this connection until the state is reached
            Ok::<(), AdbError>(())
        };

        tokio::time::timeout(timeout, fut)
            .await
            .map_err(|_| AdbError::Timeout(format!("wait_for {state} timed out")))?
    }

    /// Track device connect/disconnect events as an async stream.
    ///
    /// Returns an mpsc receiver that yields device state updates. The
    /// background task runs until the receiver is dropped.
    pub async fn track_devices(
        &self,
    ) -> Result<tokio::sync::mpsc::Receiver<Vec<DeviceEvent>>> {
        let mut conn = AdbConnection::connect(&self.host, self.port).await?;
        conn.send_and_okay("host:track-devices").await?;

        let (tx, rx) = tokio::sync::mpsc::channel(16);

        tokio::spawn(async move {
            loop {
                match conn.read_length_prefixed_string().await {
                    Ok(data) => {
                        let events: Vec<DeviceEvent> = data
                            .lines()
                            .filter(|l| !l.is_empty())
                            .filter_map(|line| {
                                let mut parts = line.split_whitespace();
                                let serial = parts.next()?.to_string();
                                let state =
                                    DeviceState::from(parts.next().unwrap_or("unknown"));
                                Some(DeviceEvent { serial, state })
                            })
                            .collect();
                        if tx.send(events).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        debug!("tracking device events");
        Ok(rx)
    }

    // ── Server-level forward list ────────────────────────────────

    /// List port forwards for ALL devices (server-level).
    ///
    /// Unlike `AdbDevice::forward_list()` which filters by serial,
    /// this returns forwards across all connected devices.
    pub async fn forward_list_all(&self) -> Result<Vec<ForwardEntry>> {
        let mut conn = AdbConnection::connect(&self.host, self.port).await?;
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
                    None
                }
            })
            .collect();

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_server() {
        let server = AdbServer::default();
        assert_eq!(server.host, "127.0.0.1");
        assert_eq!(server.port, 5037);
    }

    #[test]
    fn test_parse_devices_output() {
        let data = "emulator-5554\tdevice\n192.168.1.5:5555\toffline\n";
        let devices: Vec<DeviceInfo> = data
            .lines()
            .filter(|l| !l.is_empty())
            .filter_map(|line| {
                let mut parts = line.split_whitespace();
                let serial = parts.next()?.to_string();
                let state_str = parts.next().unwrap_or("unknown");
                Some(DeviceInfo {
                    serial,
                    state: DeviceState::from(state_str),
                })
            })
            .collect();

        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].serial, "emulator-5554");
        assert!(devices[0].state.is_online());
        assert_eq!(devices[1].serial, "192.168.1.5:5555");
        assert!(!devices[1].state.is_online());
    }

    #[test]
    fn test_parse_track_devices_output() {
        let data = "emulator-5554\tdevice\n192.168.1.5:5555\toffline\n";
        let events: Vec<DeviceEvent> = data
            .lines()
            .filter(|l| !l.is_empty())
            .filter_map(|line| {
                let mut parts = line.split_whitespace();
                let serial = parts.next()?.to_string();
                let state = DeviceState::from(parts.next().unwrap_or("unknown"));
                Some(DeviceEvent { serial, state })
            })
            .collect();

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].serial, "emulator-5554");
        assert!(events[0].state.is_online());
        assert!(!events[1].state.is_online());
    }
}
