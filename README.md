# droidrun-rs

Pure Rust implementation of Android device automation — a rewrite of the
[droidrun](https://github.com/droidrun/droidrun) Python framework's
android-driver and portal-controller layers.

Fully async with [tokio](https://tokio.rs/). Zero Python dependencies.

## Features

- **Async ADB client** — Native ADB wire protocol over TCP, no `adb` CLI dependency
- **Portal integration** — Dual-mode communication (TCP + ContentProvider fallback)
- **UI state pipeline** — Accessibility tree filtering, formatting, and element resolution
- **Recording driver** — Proxy wrapper that logs all actions as JSON
- **CLI tool** — Full-featured command-line interface for device automation
- **80+ tests** — Unit tests (60) + integration tests (18) + doc tests (2)

## Installation

### Prerequisites

- Rust 1.85.0+
- ADB server running (`adb start-server`)
- Android device/emulator with [DroidRun Portal](https://github.com/droidrun/droidrun-portal) installed

### Build from source

```bash
git clone https://github.com/user/droidrun-rs.git
cd droidrun-rs
cargo build --release
```

The CLI binary will be at `target/release/droidrun`.

## Quick Start

### CLI

```bash
# List connected devices
droidrun devices

# Check device + Portal health
droidrun doctor

# Take a screenshot
droidrun screenshot screen.png

# Tap at coordinates
droidrun tap 540 1200

# Type text
droidrun type "hello world" --clear

# Get UI state (formatted)
droidrun state

# Get UI state (raw JSON)
droidrun state --json

# Swipe down
droidrun swipe 540 400 540 1600 --duration 300

# Open an app
droidrun open com.example.app

# Run a shell command
droidrun shell getprop ro.build.version.sdk
```

### As a library

**Cargo.toml:**
```toml
[dependencies]
droidrun-core = { path = "crates/droidrun-core" }
tokio = { version = "1", features = ["full"] }
```

**Basic usage:**
```rust
use droidrun_core::driver::android::AndroidDriver;
use droidrun_core::driver::DeviceDriver;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to first available device (TCP mode)
    let mut driver = AndroidDriver::new(None, true);
    driver.connect().await?;

    // Take screenshot
    let png = driver.screenshot(true).await?;
    std::fs::write("screen.png", &png)?;

    // Tap
    driver.tap(540, 1200).await?;

    // Type text
    driver.input_text("hello from rust!", false).await?;

    // Get UI tree
    let state = driver.get_ui_tree().await?;
    println!("{}", serde_json::to_string_pretty(&state)?);

    Ok(())
}
```

**Using the state provider pipeline:**
```rust
use droidrun_core::{AndroidDriver, DeviceDriver};
use droidrun_core::{AndroidStateProvider, ConciseFilter, IndexedFormatter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut driver = AndroidDriver::new(None, true);
    driver.connect().await?;

    let provider = AndroidStateProvider::new(
        ConciseFilter,
        IndexedFormatter,
        false, // use absolute coordinates
    );

    let state = provider.get_state(&driver).await?;

    println!("Screen: {}x{}", state.screen.width, state.screen.height);
    println!("Elements: {}", state.elements.len());
    println!("\n{}", state.formatted_text);

    // Find element by index
    if let Some(elem) = state.get_element(1) {
        println!("Element 1: {} '{}'", elem.class_name, elem.text);
    }

    Ok(())
}
```

**Low-level ADB operations:**
```rust
use droidrun_adb::{AdbServer, AdbDevice};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = AdbServer::default();

    // List devices
    let devices = server.devices().await?;
    for d in &devices {
        println!("{} ({})", d.serial, d.state);
    }

    // Get first online device
    let device = server.device().await?;

    // Shell command
    let sdk = device.shell("getprop ro.build.version.sdk").await?;
    println!("SDK: {}", sdk.trim());

    // Screenshot
    let png = device.screencap().await?;
    std::fs::write("screen.png", &png)?;

    // Port forwarding
    let port = device.forward(0, 8080).await?;
    println!("Forwarded localhost:{port} -> device:8080");

    // List packages
    let packages = device.list_packages(&["-3"]).await?;
    for p in &packages {
        println!("  {p}");
    }

    Ok(())
}
```

**Recording driver:**
```rust
use droidrun_core::{AndroidDriver, RecordingDriver, DeviceDriver};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut inner = AndroidDriver::new(None, true);
    inner.connect().await?;

    let mut recorder = RecordingDriver::new(inner);

    // All actions are recorded
    recorder.tap(540, 1200).await?;
    recorder.input_text("hello", false).await?;
    recorder.press_key(4).await?; // Back

    // Get recorded actions as JSON
    let actions = recorder.actions();
    println!("{}", serde_json::to_string_pretty(&actions)?);

    Ok(())
}
```

## Architecture

```
┌──────────────┐
│ droidrun-cli │  CLI tool (clap subcommands)
└──────┬───────┘
       │
┌──────▼────────┐
│ droidrun-core │  DeviceDriver trait, Portal client, UI pipeline
└──────┬────────┘
       │
┌──────▼───────┐
│ droidrun-adb │  Async ADB wire protocol over TCP
└──────────────┘
```

### Crates

| Crate | Description |
|-------|-------------|
| **droidrun-adb** | Low-level async ADB client. Implements the ADB wire protocol directly over TCP using tokio. Provides device discovery, shell execution, file push, port forwarding, screenshots, and input control. |
| **droidrun-core** | High-level automation framework. Defines the `DeviceDriver` trait, Portal APK management, dual-mode Portal communication (TCP + ContentProvider), and the UI state processing pipeline (filter → format → UIState). |
| **droidrun-cli** | Command-line tool built with clap. Exposes all framework capabilities as subcommands. |

### Portal Communication

DroidRun Portal is an Android APK that provides accessibility tree access,
keyboard input, and screenshot capabilities. The framework communicates
with Portal via two transport modes:

| Mode | How | Speed | Use case |
|------|-----|-------|----------|
| **TCP** | HTTP requests to Portal's embedded server (port 8080, ADB-forwarded) | Fast | Default, preferred |
| **ContentProvider** | `adb shell content query/insert` commands | Slower | Automatic fallback |

The client automatically falls back from TCP to ContentProvider on failure.

### UI State Pipeline

```
Raw accessibility tree (JSON from Portal)
            ↓
    TreeFilter (ConciseFilter)     — removes off-screen & tiny elements
            ↓
    TreeFormatter (IndexedFormatter) — assigns indices, formats text
            ↓
    UIState {
        elements:       Vec<Element>,      // flattened with indices
        formatted_text: String,            // human-readable output
        phone_state:    PhoneState,        // current app, keyboard, focus
        screen:         ScreenDimensions,  // width x height
    }
```

Both `TreeFilter` and `TreeFormatter` are traits — implement your own for
custom processing.

## CLI Reference

```
droidrun [OPTIONS] <COMMAND>

Options:
  -s, --serial <SERIAL>  Device serial number
      --tcp              Use TCP mode (default: true)
  -v, --verbose          Enable debug logging

Commands:
  devices      List connected devices
  setup        Install & configure Portal on device
  doctor       Check device + Portal health
  screenshot   Take a screenshot [default: screenshot.png]
  tap          Tap at coordinates (x, y)
  swipe        Swipe between points (x1, y1, x2, y2) [--duration ms]
  type         Type text into focused field [--clear]
  key          Send key event (3=Home, 4=Back, 66=Enter)
  state        Get UI state [--json]
  apps         List installed apps [--system]
  open         Start an app by package name [--activity name]
  shell        Run a shell command on device
```

## Testing

```bash
# All tests (needs device connected)
cargo test

# Unit tests only (no device needed)
cargo test --lib

# Integration tests
cargo test -p droidrun-adb --test integration -- --nocapture
cargo test -p droidrun-core --test integration -- --nocapture

# Skip device tests
SKIP_DEVICE_TESTS=1 cargo test
```

### Test Requirements

- ADB server running
- Android emulator or device connected
- DroidRun Portal APK installed with accessibility service enabled

## Dependencies

| Crate | Purpose |
|-------|---------|
| [tokio](https://tokio.rs/) | Async runtime |
| [reqwest](https://docs.rs/reqwest) | HTTP client for Portal TCP |
| [serde](https://serde.rs/) | JSON serialization |
| [clap](https://docs.rs/clap) | CLI argument parsing |
| [thiserror](https://docs.rs/thiserror) | Error derive macros |
| [tracing](https://docs.rs/tracing) | Structured logging |
| [async-trait](https://docs.rs/async-trait) | Async trait support |
| [base64](https://docs.rs/base64) | Encoding for keyboard/screenshots |

## License

MIT
