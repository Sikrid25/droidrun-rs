/// ADB server — discovery and device enumeration.
use tracing::debug;

use crate::connection::AdbConnection;
use crate::device::AdbDevice;
use crate::error::{AdbError, Result};
use crate::models::{DeviceInfo, DeviceState};

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

    /// Get the ADB server version.
    pub async fn version(&self) -> Result<u32> {
        let mut conn = AdbConnection::connect(&self.host, self.port).await?;
        conn.send_and_okay("host:version").await?;
        let version_str = conn.read_length_prefixed_string().await?;
        let version = u32::from_str_radix(version_str.trim(), 16)
            .map_err(|_| AdbError::Parse(format!("cannot parse version: {version_str}")))?;
        Ok(version)
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
}
