# droidrun-adb

[![Crates.io](https://img.shields.io/crates/v/droidrun-adb.svg)](https://crates.io/crates/droidrun-adb)
[![Docs.rs](https://docs.rs/droidrun-adb/badge.svg)](https://docs.rs/droidrun-adb)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

Async ADB (Android Debug Bridge) client library for Rust. Implements the ADB
wire protocol + sync protocol directly over TCP using [tokio](https://tokio.rs/) —
no `adb` CLI dependency.

Part of the [droidrun-rs](https://github.com/Sikrid25/droidrun-rs) workspace.

## Features

- **70+ async methods** covering all common ADB operations
- **ADB wire protocol** — device discovery, shell, port forwarding, reverse forwarding
- **Sync protocol** — file push/pull/stat/list via binary SEND/RECV/STAT/LIST commands
- **Shell with exit codes** — `shell2()` returns stdout + exit code
- **Streaming APIs** — `track_devices()` and `logcat()` via `tokio::sync::mpsc` channels
- **App management** — install, uninstall, start, stop, clear, info
- **Device info** — properties, screen size, rotation, features, WLAN IP
- **Zero unsafe code**

## Quick Start

```toml
[dependencies]
droidrun-adb = "0.1"
tokio = { version = "1", features = ["full"] }
```

```rust
use droidrun_adb::AdbServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = AdbServer::default();
    let device = server.device().await?;

    // Shell with exit code
    let result = device.shell2("echo hello").await?;
    println!("stdout={}, exit_code={}", result.stdout.trim(), result.exit_code);

    // System properties
    let model = device.prop_model().await?;
    println!("Model: {model}");

    // Screenshot
    let png = device.screencap().await?;
    std::fs::write("screen.png", &png)?;

    Ok(())
}
```

## API Overview

### Server (`AdbServer`)

| Method | Description |
|--------|-------------|
| `devices()` | List connected devices |
| `device()` / `device_or(serial)` | Get first / specific device |
| `server_version()` | ADB server version |
| `server_kill()` | Kill ADB server |
| `connect_device(addr)` | Connect to remote device |
| `disconnect_device(addr)` | Disconnect remote device |
| `wait_for(serial, state, timeout)` | Wait for device state |
| `track_devices()` | Stream device connect/disconnect events |
| `forward_list_all()` | List all port forwards (all devices) |

### Device — Shell & System (`AdbDevice`)

| Method | Description |
|--------|-------------|
| `shell(cmd)` | Run shell command → String |
| `shell2(cmd)` | Run shell command → `ShellOutput { stdout, exit_code }` |
| `shell_bytes(cmd)` | Run shell command → raw bytes |
| `getprop(name)` | Get system property |
| `prop_model()` / `prop_name()` / `prop_device()` | Common property shortcuts |
| `root()` | Restart adbd as root |
| `tcpip(port)` | Switch adbd to TCP/IP mode |
| `reboot(mode)` | Reboot (Normal/Bootloader/Recovery/Sideload) |

### Device — File Sync Protocol

| Method | Description |
|--------|-------------|
| `push(local, remote)` | Push file to device |
| `push_bytes(data, remote)` | Push bytes to device |
| `pull(remote, local)` | Pull file from device |
| `pull_bytes(remote)` | Pull file as bytes |
| `stat(path)` | Get file info → `FileStat { mode, size, mtime }` |
| `list_dir(path)` | List directory → `Vec<SyncDirEntry>` |
| `exists(path)` | Check if file exists |
| `remove(path)` / `rmtree(path)` | Delete file/directory |

### Device — Port Forwarding

| Method | Description |
|--------|-------------|
| `forward(local, remote)` | Forward host→device (port 0 = dynamic) |
| `forward_list()` | List forwards for this device |
| `forward_remove(port)` | Remove a forward |
| `forward_remove_all()` | Remove all forwards |
| `reverse(remote, local)` | Reverse forward device→host |
| `reverse_list()` | List reverse forwards |
| `reverse_remove(port)` | Remove a reverse forward |
| `reverse_remove_all()` | Remove all reverse forwards |

### Device — Apps & Input

| Method | Description |
|--------|-------------|
| `install(apk)` | Install APK |
| `uninstall(pkg)` | Uninstall package |
| `app_start(pkg, activity)` | Start app |
| `app_stop(pkg)` | Force stop app |
| `app_clear(pkg)` | Clear app data |
| `app_current()` | Current foreground app |
| `app_info(pkg)` | App details (version, path) |
| `list_packages(flags)` | List installed packages |
| `tap(x, y)` | Tap at coordinates |
| `swipe(x1, y1, x2, y2, ms)` | Swipe gesture |
| `drag(sx, sy, ex, ey, ms)` | Drag gesture |
| `key_event(code)` | Send key event |
| `input_text(text)` | Type text |

### Device — Screen & Info

| Method | Description |
|--------|-------------|
| `screencap()` | Take screenshot → PNG bytes |
| `window_size()` | Screen dimensions |
| `rotation()` | Current rotation (0-3) |
| `is_screen_on()` | Screen state |
| `switch_screen(on)` | Toggle screen |
| `get_serialno()` | Device serial number |
| `get_features()` | ADB feature list |
| `wlan_ip()` | Device WLAN IP address |
| `logcat(filter)` | Stream logcat lines |

## Examples

```bash
cargo run -p droidrun-adb --example basic
cargo run -p droidrun-adb --example screenshot
cargo run -p droidrun-adb --example port_forward
cargo run -p droidrun-adb --example input_control
cargo run -p droidrun-adb --example file_transfer
cargo run -p droidrun-adb --example app_management
cargo run -p droidrun-adb --example reverse_forward
cargo run -p droidrun-adb --example device_info
```

## Requirements

- Rust 1.85.0+
- ADB server running (`adb start-server`)
- Android device or emulator connected

## License

MIT
