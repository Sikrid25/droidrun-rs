# droidrun-rs

Pure Rust rewrite of the Android device automation layer from
[droidrun](https://github.com/droidrun/droidrun) (Python). Zero Python
dependencies — fully async with tokio.

## Quick Reference

```bash
# Build
cargo build

# Run all tests (needs ADB + emulator with droidrun-portal)
cargo test -- --nocapture

# Unit tests only (no device needed)
cargo test --lib

# Integration tests per crate
cargo test -p droidrun-adb --test integration -- --nocapture
cargo test -p droidrun-core --test integration -- --nocapture

# Skip device tests via env var
SKIP_DEVICE_TESTS=1 cargo test

# Run CLI
cargo run -p droidrun-cli -- devices
cargo run -p droidrun-cli -- doctor
cargo run -p droidrun-cli -- state
```

## Project Structure

```
droidrun-rs/
├── Cargo.toml                  # Workspace root
├── crates/
│   ├── droidrun-adb/           # Low-level async ADB client
│   │   ├── src/
│   │   │   ├── lib.rs          # Public API re-exports
│   │   │   ├── connection.rs   # ADB wire protocol over TCP
│   │   │   ├── device.rs       # Per-device operations
│   │   │   ├── server.rs       # Device discovery, lifecycle, tracking
│   │   │   ├── models.rs       # DeviceState, DeviceInfo, ForwardEntry, FileStat, ShellOutput, ...
│   │   │   └── error.rs        # AdbError (+ SyncError, UninstallFailed, RootFailed)
│   │   ├── examples/           # 8 examples (basic, screenshot, port_forward, input_control, file_transfer, app_management, reverse_forward, device_info)
│   │   └── tests/
│   │       └── integration.rs  # 27 tests (needs ADB server)
│   │
│   ├── droidrun-core/          # High-level automation framework
│   │   ├── src/
│   │   │   ├── lib.rs          # Public API re-exports
│   │   │   ├── error.rs        # DroidrunError
│   │   │   ├── driver/
│   │   │   │   ├── mod.rs      # DeviceDriver trait, Action, AppInfo, Point
│   │   │   │   ├── android.rs  # AndroidDriver (ADB + Portal)
│   │   │   │   └── recording.rs# RecordingDriver<D> proxy
│   │   │   ├── portal/
│   │   │   │   ├── mod.rs      # Constants
│   │   │   │   ├── client.rs   # PortalClient (TCP + ContentProvider)
│   │   │   │   ├── manager.rs  # APK lifecycle & setup + is_version_newer()
│   │   │   │   ├── a11y.rs     # Accessibility service control
│   │   │   │   └── keyboard.rs # DroidRun keyboard IME setup
│   │   │   ├── ui/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── state.rs    # UIState, Element, PhoneState
│   │   │   │   ├── filter.rs   # TreeFilter trait + ConciseFilter
│   │   │   │   ├── formatter.rs# TreeFormatter trait + IndexedFormatter
│   │   │   │   ├── provider.rs # AndroidStateProvider (fetch + process)
│   │   │   │   ├── search.rs   # Composable element filter functions
│   │   │   │   ├── coord.rs    # Normalized [0-1000] ↔ absolute pixel
│   │   │   │   └── geometry.rs # Bounds, overlap, clear-point finding
│   │   │   └── helpers/
│   │   │       └── mod.rs
│   │   ├── examples/           # 6 examples (driver_basics, state_provider, recording, ...)
│   │   └── tests/
│   │       └── integration.rs  # 10 tests (needs emulator + portal)
│   │
│   └── droidrun-cli/           # CLI binary
│       ├── Cargo.toml
│       └── src/
│           └── main.rs         # `droidrun` binary with clap
```

## Architecture

### Three-Crate Design

```
┌──────────────┐
│ droidrun-cli │  CLI tool (clap)
└──────┬───────┘
       │
┌──────▼────────┐
│ droidrun-core │  DeviceDriver trait, Portal, UI state pipeline
└──────┬────────┘
       │
┌──────▼───────┐
│ droidrun-adb │  Raw ADB wire protocol, async TCP
└──────────────┘
```

### ADB Wire Protocol (`droidrun-adb`)

- TCP connection to ADB server at `127.0.0.1:5037`
- Request format: `{length:04X}{command}` (4-char hex length prefix)
- Response: `OKAY` or `FAIL{length:04X}{error_message}`
- Each operation opens a **fresh TCP connection** (stateless)
- Sync protocol for file operations: `STAT`, `LIST`/`DENT`, `SEND`/`DATA`/`DONE`, `RECV`/`DATA`/`DONE`, `QUIT`
- Reverse forwarding: `reverse:forward:tcp:<remote>;tcp:<local>` protocol

**Key notes**:
- `forward(0, remote_port)` uses a double-OKAY protocol. The server sends OKAY (accepted), then OKAY + port (allocation result).
- `shell2()` uses a sentinel pattern (`DROIDRUN_EXIT:$?`) to extract exit codes from shell commands.
- `track_devices()` and `logcat()` use long-lived connections with `tokio::sync::mpsc` channels for streaming.

### Portal Communication (`droidrun-core/portal`)

DroidRun Portal is an Android APK that provides:
- Accessibility tree access (UI element tree)
- Keyboard input (custom IME)
- Screenshot with overlay control

**Dual transport with auto-fallback:**

| Mode | Mechanism | Speed | Reliability |
|------|-----------|-------|-------------|
| TCP | HTTP to Portal server on port 8080 (ADB forwarded) | Fast | Needs port forward |
| ContentProvider | `adb shell content query/insert` commands | Slower | Always works |

Portal response envelope: `{"status":"success","result":...}` or
`{"status":"success","data":...}` (legacy). The `parse_content_provider_output`
function unwraps this envelope automatically in all code paths.

Content provider output format: `Row: 0 result={json}`

### UI State Pipeline (`droidrun-core/ui`)

```
Portal API → raw a11y tree JSON
     ↓
 TreeFilter (ConciseFilter)
     ↓  removes off-screen & tiny elements
 TreeFormatter (IndexedFormatter)
     ↓  assigns sequential indices, formats text
 UIState { elements, formatted_text, phone_state, screen }
```

Output format:
```
index. ClassName: resourceId; checkedState, text - bounds(x1,y1,x2,y2)
```

Coordinate system: normalized `[0-1000]` ↔ absolute pixels. Use
`to_absolute(norm, dimension)` and `to_normalized(abs, dimension)`.

## Key Types & Traits

### `DeviceDriver` trait (async)

```rust
#[async_trait]
pub trait DeviceDriver: Send + Sync {
    async fn connect(&mut self) -> Result<()>;
    async fn tap(&self, x: i32, y: i32) -> Result<()>;
    async fn swipe(&self, x1: i32, y1: i32, x2: i32, y2: i32, duration_ms: u32) -> Result<()>;
    async fn input_text(&self, text: &str, clear: bool) -> Result<bool>;
    async fn press_key(&self, keycode: i32) -> Result<()>;
    async fn screenshot(&self, hide_overlay: bool) -> Result<Vec<u8>>;
    async fn get_ui_tree(&self) -> Result<serde_json::Value>;
    async fn get_apps(&self, include_system: bool) -> Result<Vec<AppInfo>>;
    // ... plus start_app, install_app, list_packages, get_date, drag
}
```

### Implementations

- **`AndroidDriver`** — Primary driver. Uses ADB for basic input, Portal for
  UI tree/screenshots/keyboard. Creates `PortalClient` on `connect()`.
- **`RecordingDriver<D: DeviceDriver>`** — Proxy that logs all mutating
  actions (tap, swipe, text) as `RecordedAction` JSON. Pass-through for reads.

### Error Types

```rust
// droidrun-adb
enum AdbError {
    Io, ServerFailed, Protocol, NoDevice, DeviceNotOnline,
    DeviceNotFound, ShellError, InstallFailed, UninstallFailed,
    SyncError, RootFailed, Utf8, Parse, ConnectionRefused, Timeout,
}

// droidrun-core
enum DroidrunError {
    Adb(AdbError), Http, Json, Io,
    NotConnected, PortalNotInstalled, PortalAccessibilityDisabled,
    PortalSetupFailed, PortalCommError,
    ElementNotFound(usize), ElementNoBounds(usize), ElementObscured(usize),
    InvalidBounds, NoDimensions, Parse, NotSupported, Timeout,
}
```

## CLI Commands

Binary name: `droidrun`

| Command | Description | Key flags |
|---------|-------------|-----------|
| `devices` | List connected devices | |
| `setup` | Install & configure Portal on device | |
| `doctor` | Health check (device + Portal) | |
| `screenshot [file]` | Save screenshot PNG | default: screenshot.png |
| `tap <x> <y>` | Tap at pixel coordinates | |
| `swipe <x1> <y1> <x2> <y2>` | Swipe gesture | `--duration <ms>` |
| `type <text>` | Type text into focused field | `--clear` |
| `key <keycode>` | Send key event (3=Home, 4=Back) | |
| `state` | Show UI state | `--json` for raw JSON |
| `apps` | List installed apps | `--system` |
| `open <package>` | Launch app | `--activity <name>` |
| `shell <cmd...>` | Run shell command on device | |

Global flags: `--serial <s>`, `--tcp`, `--verbose`

## Testing

### Test Categories

| Category | Count | Location | Requires |
|----------|-------|----------|----------|
| Unit tests (adb) | 29 | `src/**/*.rs` `#[cfg(test)]` | Nothing |
| Unit tests (core) | 54 | `src/**/*.rs` `#[cfg(test)]` | Nothing |
| Integration (adb) | 27 | `crates/droidrun-adb/tests/` | ADB server + device |
| Integration (core) | 10 | `crates/droidrun-core/tests/` | Emulator + Portal APK |
| Doc tests | 2 | `lib.rs` doc comments | Nothing (compile only) |
| **Total** | **122** | | |

### Running Tests

```bash
# All tests including integration (device must be connected)
cargo test -- --nocapture

# Unit tests only (no device needed)
cargo test --lib

# Skip device tests via env var
SKIP_DEVICE_TESTS=1 cargo test

# Specific integration suite
cargo test -p droidrun-adb --test integration -- --nocapture
cargo test -p droidrun-core --test integration -- --nocapture

# Single test
cargo test -p droidrun-core --test integration -- test_portal_ping --nocapture
```

### Integration Test Requirements

- ADB server running (`adb start-server`)
- Android emulator or physical device connected
- `com.droidrun.portal` APK installed
- Accessibility service enabled for Portal
- Tests auto-skip if `SKIP_DEVICE_TESTS` env var is set

## Examples

```bash
# Run a specific example
cargo run -p droidrun-adb --example basic
cargo run -p droidrun-core --example driver_basics
```

### droidrun-adb Examples

| Example | Description |
|---------|-------------|
| `basic` | Connect to device, shell commands, device info, list packages |
| `screenshot` | Take and save PNG screenshot |
| `port_forward` | Dynamic (port 0) and fixed port forwarding lifecycle |
| `input_control` | Tap, swipe, key events (Home/Back/Enter) |
| `file_transfer` | Push, stat, list_dir, pull, pull_bytes via sync protocol |
| `app_management` | app_current, app_info, list_packages, app_stop, app_clear |
| `reverse_forward` | Reverse port forwarding: create, list, remove |
| `device_info` | getprop, window_size, rotation, features, wlan_ip, shell2 |

### droidrun-core Examples

| Example | Description |
|---------|-------------|
| `driver_basics` | High-level `AndroidDriver` API — connect, state, tap, apps |
| `state_provider` | Full UI pipeline: filter → format → UIState |
| `recording` | `RecordingDriver` proxy for action logging (JSON output) |
| `element_search` | Composable filters: `text_matches`, `id_matches`, `clickable`, `below` |
| `portal_setup` | `PortalManager` lifecycle + `PortalClient` direct communication |
| `app_automation` | Practical workflow: list apps → open Settings → find & tap → navigate |

> All 14 examples require a connected device/emulator. droidrun-core examples also need droidrun-portal installed.

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| tokio | 1 (full) | Async runtime |
| reqwest | 0.13 | HTTP client (Portal TCP) |
| serde / serde_json | 1 | JSON serialization |
| base64 | 0.22 | Encoding (keyboard text, screenshots) |
| thiserror | 2 | Error type macros |
| anyhow | 1 | CLI error handling |
| tracing | 0.1 | Structured logging |
| async-trait | 0.1 | Async trait support |
| regex | 1 | Pattern matching |
| clap | 4 (derive) | CLI argument parsing |

## Code Style & Conventions

- **Edition**: 2024, **MSRV**: 1.85.0
- **Async everywhere**: All I/O operations are `async fn`
- **Error handling**: `thiserror` for library crates, `anyhow` in CLI
- **Logging**: `tracing` macros (`debug!`, `warn!`, `info!`)
- **Naming**: snake_case for functions, CamelCase for types, SCREAMING_SNAKE for constants
- **Module structure**: `mod.rs` for module roots with child modules
- **Tests**: `#[cfg(test)] mod tests` in each source file + separate integration tests
- **No `unwrap()` in library code** — use `?` or explicit error handling
- **Trait-based extensibility**: `TreeFilter`, `TreeFormatter`, `DeviceDriver` are all traits

## Common Patterns

### Adding a new DeviceDriver method

1. Add method to `DeviceDriver` trait in `crates/droidrun-core/src/driver/mod.rs`
2. Implement in `AndroidDriver` (`driver/android.rs`)
3. Add pass-through + recording in `RecordingDriver` (`driver/recording.rs`)
4. Add corresponding `Action` variant if it's a new capability
5. Add CLI subcommand in `crates/droidrun-cli/src/main.rs` if user-facing

### Adding a new Portal endpoint

1. Add TCP method in `PortalClient` (e.g., `my_feature_tcp`)
2. Add ContentProvider fallback method (e.g., `my_feature_content_provider`)
3. Add public method that tries TCP → falls back to ContentProvider
4. Remember: `parse_content_provider_output` already unwraps the portal envelope

### Adding a new UI filter/formatter

1. Implement `TreeFilter` trait (see `ConciseFilter` in `ui/filter.rs`)
2. Or implement `TreeFormatter` trait (see `IndexedFormatter` in `ui/formatter.rs`)
3. Pass to `AndroidStateProvider::new(filter, formatter, use_normalized)`

## Portal Constants

```rust
PORTAL_PACKAGE:    "com.droidrun.portal"
A11Y_SERVICE:      "com.droidrun.portal/com.droidrun.portal.service.DroidrunAccessibilityService"
PORTAL_HTTP_PORT:  8080
KEYBOARD_IME:      "com.droidrun.portal/.input.DroidrunKeyboardIME"
PORTAL_REPO:       "droidrun/droidrun-portal"
```

## Known Behaviors

- `input_text()` returns `false` when no text field is focused (not an error)
- `forward(0, port)` uses ADB's double-OKAY protocol for port allocation
- Portal screenshots come as base64 JSON over TCP, raw PNG over ADB fallback
- `screencap -p` may have `\n` → `\r\n` conversion on some devices
- `parse_content_provider_output` unwraps portal envelope in ALL code paths —
  callers should NOT call `unwrap_portal_response` again

## Important Gotchas

### tempfile crate — `.keep()` is required

`NamedTempFile::into_temp_path()` returns a `TempPath` that **still deletes
on drop**. If you need the file to persist (e.g. for `install()` to read),
you must call `.keep()`:

```rust
// WRONG — file is deleted immediately when TempPath drops
tmp.into_temp_path();

// CORRECT — file persists on disk
let _ = tmp.into_temp_path().keep();
```

### Portal version management — no downgrade

`ensure_ready()` uses `is_version_newer()` to compare versions. It will only
upgrade when the expected version is strictly newer than installed. If the
device already has a newer Portal (e.g. v0.6.0) than what the version map
suggests (e.g. v0.4.6 for an SDK not in the map), it skips the reinstall.

### Portal envelope unwrapping

`parse_content_provider_output()` unwraps the portal response envelope
(`{"status":"success","result":...}`) in **all code paths** — direct JSON,
`result=` prefix, and last resort. Callers must never double-unwrap.
