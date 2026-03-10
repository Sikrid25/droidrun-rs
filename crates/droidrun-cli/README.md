# droidrun-cli

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

Command-line tool for Android device automation. Provides subcommands for
tapping, swiping, typing, screenshots, UI inspection, app management,
and shell access.

Built on [droidrun-core](https://crates.io/crates/droidrun-core) and
[droidrun-adb](https://crates.io/crates/droidrun-adb).
Part of the [droidrun-rs](https://github.com/Sikrid25/droidrun-rs) workspace.

> **Note:** This crate is not published to crates.io. Install from source.

## Installation

```bash
git clone https://github.com/Sikrid25/droidrun-rs.git
cd droidrun-rs
cargo install --path crates/droidrun-cli
```

The binary `droidrun` will be installed to `~/.cargo/bin/`.

## Usage

```
droidrun [OPTIONS] <COMMAND>

Options:
  -s, --serial <SERIAL>  Device serial number (auto-detect if not specified)
      --tcp              Use TCP mode for Portal communication (default: true)
  -v, --verbose          Enable debug logging
  -h, --help             Print help
  -V, --version          Print version
```

## Commands

### Device Management

```bash
# List connected devices
droidrun devices

# Install & configure DroidRun Portal
droidrun setup

# Health check — device + Portal status
droidrun doctor
```

### Input

```bash
# Tap at coordinates
droidrun tap 540 1200

# Swipe gesture (with optional duration)
droidrun swipe 540 400 540 1600 --duration 300

# Type text (optionally clear field first)
droidrun type "hello world" --clear

# Send key event
droidrun key 4     # Back
droidrun key 3     # Home
droidrun key 66    # Enter
```

### Screenshot & UI

```bash
# Take a screenshot (default: screenshot.png)
droidrun screenshot
droidrun screenshot screen.png

# Get formatted UI state
droidrun state

# Get raw JSON UI tree
droidrun state --json
```

### Apps

```bash
# List third-party apps
droidrun apps

# Include system apps
droidrun apps --system

# Launch an app
droidrun open com.example.app

# Launch with specific activity
droidrun open com.android.settings --activity .Settings
```

### Shell

```bash
# Run a shell command on device
droidrun shell getprop ro.build.version.sdk
droidrun shell pm list packages -3
droidrun shell dumpsys battery
```

## Common Key Codes

| Code | Key |
|------|-----|
| 3 | Home |
| 4 | Back |
| 24 | Volume Up |
| 25 | Volume Down |
| 26 | Power |
| 66 | Enter |
| 67 | Backspace |
| 82 | Menu |
| 187 | App Switch |

## Requirements

- Rust 1.85.0+ (for building)
- ADB server running (`adb start-server`)
- Android device/emulator connected
- [DroidRun Portal](https://github.com/droidrun/droidrun-portal) installed (for `state`, `screenshot`, `type`, `doctor` commands)

## License

MIT
