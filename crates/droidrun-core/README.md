# droidrun-core

[![Crates.io](https://img.shields.io/crates/v/droidrun-core.svg)](https://crates.io/crates/droidrun-core)
[![Docs.rs](https://docs.rs/droidrun-core/badge.svg)](https://docs.rs/droidrun-core)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

High-level Android device automation framework for Rust. Provides the
`DeviceDriver` trait, DroidRun Portal integration, and a UI state processing
pipeline.

Built on top of [droidrun-adb](https://crates.io/crates/droidrun-adb).
Part of the [droidrun-rs](https://github.com/Sikrid25/droidrun-rs) workspace.

## Features

- **`DeviceDriver` trait** — async interface for all device operations (tap, swipe, text, screenshot, UI tree)
- **`AndroidDriver`** — primary implementation using ADB + DroidRun Portal
- **`RecordingDriver`** — proxy wrapper that logs all actions as JSON
- **Portal integration** — dual-mode communication (TCP + ContentProvider fallback)
- **UI state pipeline** — accessibility tree filtering → formatting → structured `UIState`
- **Portal management** — automatic APK install, accessibility service setup, keyboard IME

## Quick Start

```toml
[dependencies]
droidrun-core = "0.1"
tokio = { version = "1", features = ["full"] }
```

### Basic Driver Usage

```rust
use droidrun_core::{AndroidDriver, DeviceDriver};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut driver = AndroidDriver::new(None, true); // TCP mode
    driver.connect().await?;

    // Screenshot
    let png = driver.screenshot(true).await?;
    std::fs::write("screen.png", &png)?;

    // Tap, type, navigate
    driver.tap(540, 1200).await?;
    driver.input_text("hello from rust!", false).await?;
    driver.press_key(4).await?; // Back

    // Get UI tree
    let state = driver.get_ui_tree().await?;
    println!("{}", serde_json::to_string_pretty(&state)?);

    Ok(())
}
```

### UI State Pipeline

```rust
use droidrun_core::{AndroidDriver, DeviceDriver};
use droidrun_core::{AndroidStateProvider, ConciseFilter, IndexedFormatter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut driver = AndroidDriver::new(None, true);
    driver.connect().await?;

    let provider = AndroidStateProvider::new(
        ConciseFilter,       // removes off-screen & tiny elements
        IndexedFormatter,    // assigns sequential indices
        false,               // use absolute coordinates
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

### Recording Driver

```rust
use droidrun_core::{AndroidDriver, RecordingDriver, DeviceDriver};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut inner = AndroidDriver::new(None, true);
    inner.connect().await?;

    let mut recorder = RecordingDriver::new(inner);

    recorder.tap(540, 1200).await?;
    recorder.input_text("hello", false).await?;
    recorder.press_key(4).await?;

    // Get recorded actions as JSON
    let actions = recorder.actions();
    println!("{}", serde_json::to_string_pretty(&actions)?);

    Ok(())
}
```

## Architecture

```
┌──────────────────┐
│   Your App       │
└────────┬─────────┘
         │
┌────────▼─────────┐
│  DeviceDriver    │  async trait (tap, swipe, text, screenshot, UI tree)
│  ├ AndroidDriver │  ADB + Portal
│  └ RecordingDriver  Proxy with action logging
└────────┬─────────┘
         │
┌────────▼─────────┐
│  Portal Client   │  Dual-mode: TCP (fast) → ContentProvider (fallback)
│  Portal Manager  │  APK install, a11y service, keyboard IME
└────────┬─────────┘
         │
┌────────▼─────────┐
│  UI Pipeline     │  TreeFilter → TreeFormatter → UIState
│  ├ ConciseFilter │  Remove off-screen & tiny elements
│  └ IndexedFormatter  Assign indices, format text
└──────────────────┘
```

### Portal Communication

[DroidRun Portal](https://github.com/droidrun/droidrun-portal) is an Android
APK that provides accessibility tree access, keyboard input, and screenshots.

| Mode | How | Speed |
|------|-----|-------|
| **TCP** | HTTP to Portal server (port 8080, ADB-forwarded) | Fast |
| **ContentProvider** | `adb shell content query/insert` | Slower (fallback) |

Auto-fallback: if TCP fails, ContentProvider is used automatically.

### UI State Output Format

```
index. ClassName: resourceId; checkedState, text - bounds(x1,y1,x2,y2)
```

## Key Types

| Type | Description |
|------|-------------|
| `DeviceDriver` | Async trait for device operations |
| `AndroidDriver` | Primary driver (ADB + Portal) |
| `RecordingDriver<D>` | Proxy that logs mutating actions |
| `PortalClient` | Portal TCP + ContentProvider client |
| `PortalManager` | APK lifecycle & setup |
| `AndroidStateProvider` | UI state fetch + process pipeline |
| `UIState` | Processed UI state (elements, formatted text, phone state) |
| `Element` | Single UI element with bounds, text, class |
| `TreeFilter` / `TreeFormatter` | Traits for custom UI processing |
| `ConciseFilter` / `IndexedFormatter` | Default implementations |

## Examples

```bash
cargo run -p droidrun-core --example driver_basics
cargo run -p droidrun-core --example state_provider
cargo run -p droidrun-core --example recording
cargo run -p droidrun-core --example element_search
cargo run -p droidrun-core --example portal_setup
cargo run -p droidrun-core --example app_automation
```

## Requirements

- Rust 1.85.0+
- ADB server running
- Android device/emulator with [DroidRun Portal](https://github.com/droidrun/droidrun-portal) installed
- Portal accessibility service enabled

## License

MIT
