/// Low-level ADB wire protocol connection.
///
/// ADB protocol format:
///   Request:  [4-char hex length][payload]
///   Response: "OKAY" | "FAIL"[4-char hex length][error message]
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::trace;

use crate::error::{AdbError, Result};

/// A single TCP connection to the ADB server.
pub struct AdbConnection {
    stream: TcpStream,
}

impl AdbConnection {
    /// Connect to the ADB server at the given address.
    pub async fn connect(host: &str, port: u16) -> Result<Self> {
        let stream = TcpStream::connect((host, port)).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::ConnectionRefused {
                AdbError::ConnectionRefused
            } else {
                AdbError::Io(e)
            }
        })?;
        Ok(Self { stream })
    }

    /// Send a command to the ADB server using the wire protocol.
    ///
    /// Format: `{length:04X}{command}`
    pub async fn send_command(&mut self, cmd: &str) -> Result<()> {
        let msg = format!("{:04X}{}", cmd.len(), cmd);
        trace!("ADB send: {msg}");
        self.stream.write_all(msg.as_bytes()).await?;
        Ok(())
    }

    /// Read the OKAY/FAIL status response.
    pub async fn read_status(&mut self) -> Result<()> {
        let mut buf = [0u8; 4];
        self.stream.read_exact(&mut buf).await?;
        match &buf {
            b"OKAY" => Ok(()),
            b"FAIL" => {
                let msg = self.read_length_prefixed_string().await?;
                Err(AdbError::ServerFailed(msg))
            }
            other => {
                let s = String::from_utf8_lossy(other).to_string();
                Err(AdbError::Protocol(format!("expected OKAY/FAIL, got: {s}")))
            }
        }
    }

    /// Send a command and expect OKAY.
    pub async fn send_and_okay(&mut self, cmd: &str) -> Result<()> {
        self.send_command(cmd).await?;
        self.read_status()
            .await
            .map_err(|e| AdbError::ServerFailed(format!("command '{cmd}' failed: {e}")))
    }

    /// Read a length-prefixed string response.
    ///
    /// Format: `[4-char hex length][data]`
    pub async fn read_length_prefixed_string(&mut self) -> Result<String> {
        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf).await?;
        let len_str = std::str::from_utf8(&len_buf)
            .map_err(|_| AdbError::Protocol("invalid length bytes".into()))?;
        let len = usize::from_str_radix(len_str, 16)
            .map_err(|_| AdbError::Protocol(format!("invalid hex length: {len_str}")))?;

        if len == 0 {
            return Ok(String::new());
        }

        let mut buf = vec![0u8; len];
        self.stream.read_exact(&mut buf).await?;
        Ok(String::from_utf8(buf)?)
    }

    /// Read all remaining data as a String until the connection closes.
    pub async fn read_until_close_string(&mut self) -> Result<String> {
        let bytes = self.read_until_close_bytes().await?;
        Ok(String::from_utf8(bytes)?)
    }

    /// Read all remaining data as bytes until the connection closes.
    pub async fn read_until_close_bytes(&mut self) -> Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(4096);
        self.stream.read_to_end(&mut buf).await?;
        Ok(buf)
    }

    /// Expose the inner stream for advanced operations (e.g., sync protocol).
    pub fn into_stream(self) -> TcpStream {
        self.stream
    }

    /// Get a mutable reference to the inner stream.
    pub fn stream_mut(&mut self) -> &mut TcpStream {
        &mut self.stream
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_command_format() {
        // Verify the format string produces correct output
        let cmd = "host:version";
        let msg = format!("{:04X}{}", cmd.len(), cmd);
        assert_eq!(msg, "000Chost:version");
    }

    #[test]
    fn test_short_command_format() {
        let cmd = "host:devices";
        let msg = format!("{:04X}{}", cmd.len(), cmd);
        assert_eq!(msg, "000Chost:devices");
    }
}
