//! # droidrun-adb
//!
//! Async ADB (Android Debug Bridge) client library.
//!
//! Implements the ADB wire protocol directly over TCP using tokio,
//! providing native async support for all operations.
//!
//! ## Usage
//!
//! ```no_run
//! use droidrun_adb::{AdbServer, AdbDevice};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Connect to first available device
//!     let server = AdbServer::default();
//!     let device = server.device().await?;
//!
//!     // Run shell command
//!     let output = device.shell("getprop ro.build.version.sdk").await?;
//!     println!("SDK version: {}", output.trim());
//!
//!     // Take screenshot
//!     let png = device.screencap().await?;
//!     std::fs::write("screen.png", &png)?;
//!
//!     Ok(())
//! }
//! ```

pub mod connection;
pub mod device;
pub mod error;
pub mod models;
pub mod server;

pub use device::AdbDevice;
pub use error::{AdbError, Result};
pub use models::{
    AppDetail, CurrentApp, DeviceEvent, DeviceInfo, DeviceState, FileStat, ForwardEntry,
    RebootMode, ReverseEntry, ScreenSize, ShellOutput, SyncDirEntry,
};
pub use server::AdbServer;
