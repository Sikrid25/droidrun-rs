//! # droidrun-core
//!
//! Android device automation core library.
//!
//! Provides device drivers, Portal APK management, and UI state processing
//! for controlling Android devices programmatically.
//!
//! ## Usage
//!
//! ```no_run
//! use droidrun_core::driver::android::AndroidDriver;
//! use droidrun_core::driver::DeviceDriver;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut driver = AndroidDriver::new(None, true);
//!     driver.connect().await?;
//!
//!     // Take screenshot
//!     let png = driver.screenshot(true).await?;
//!     std::fs::write("screen.png", &png)?;
//!
//!     // Tap
//!     driver.tap(540, 1200).await?;
//!
//!     // Type text
//!     driver.input_text("hello", false).await?;
//!
//!     Ok(())
//! }
//! ```

pub mod driver;
pub mod error;
pub mod helpers;
pub mod portal;
pub mod ui;

// Re-exports for convenience
pub use driver::android::AndroidDriver;
pub use driver::recording::RecordingDriver;
pub use driver::{Action, AppInfo, DeviceDriver, Point};
pub use error::{DroidrunError, Result};
pub use portal::client::PortalClient;
pub use portal::manager::PortalManager;
pub use ui::filter::{ConciseFilter, TreeFilter};
pub use ui::formatter::{IndexedFormatter, TreeFormatter};
pub use ui::provider::AndroidStateProvider;
pub use ui::state::{Element, PhoneState, ScreenDimensions, UIState};
