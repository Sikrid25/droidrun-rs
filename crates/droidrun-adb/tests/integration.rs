/// Integration tests for ADB operations.
///
/// These tests require a running ADB server and connected device/emulator.
/// Run with: `cargo test -p droidrun-adb --test integration -- --nocapture`
///
/// To skip these tests: `cargo test --lib` (unit tests only)
use droidrun_adb::AdbServer;

fn skip_if_no_device() -> bool {
    std::env::var("SKIP_DEVICE_TESTS").is_ok()
}

// ══════════════════════════════════════════════════════════════
//  Original tests
// ══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_server_version() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    match server.version().await {
        Ok(version) => {
            println!("ADB server version: {version}");
            assert!(version > 0);
        }
        Err(e) => {
            println!("Skipping: ADB server not running ({e})");
        }
    }
}

#[tokio::test]
async fn test_list_devices() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    match server.devices().await {
        Ok(devices) => {
            println!("Found {} device(s):", devices.len());
            for d in &devices {
                println!("  {} ({})", d.serial, d.state);
            }
        }
        Err(e) => {
            println!("Skipping: {e}");
        }
    }
}

#[tokio::test]
async fn test_device_shell() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    let device = match server.device().await {
        Ok(d) => d,
        Err(_) => {
            println!("Skipping: no device");
            return;
        }
    };

    let output = device.shell("echo hello_from_rust").await.unwrap();
    assert!(output.contains("hello_from_rust"));
    println!("Shell output: {}", output.trim());
}

#[tokio::test]
async fn test_device_state() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    let device = match server.device().await {
        Ok(d) => d,
        Err(_) => return,
    };

    let state = device.get_state().await.unwrap();
    assert!(state.is_online());
    println!("Device state: {state}");
}

#[tokio::test]
async fn test_device_date() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    let device = match server.device().await {
        Ok(d) => d,
        Err(_) => return,
    };

    let date = device.get_date().await.unwrap();
    println!("Device date: {date}");
    assert!(!date.is_empty());
}

#[tokio::test]
async fn test_list_packages() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    let device = match server.device().await {
        Ok(d) => d,
        Err(_) => return,
    };

    let packages = device.list_packages(&["-3"]).await.unwrap();
    println!("Found {} third-party packages", packages.len());
    for p in packages.iter().take(5) {
        println!("  {p}");
    }
}

#[tokio::test]
async fn test_screenshot() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    let device = match server.device().await {
        Ok(d) => d,
        Err(_) => return,
    };

    let png = device.screencap().await.unwrap();
    assert!(!png.is_empty());
    // PNG magic bytes
    assert_eq!(&png[..4], &[0x89, 0x50, 0x4E, 0x47]);
    println!("Screenshot: {} bytes", png.len());
}

#[tokio::test]
async fn test_port_forward() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    let device = match server.device().await {
        Ok(d) => d,
        Err(_) => return,
    };

    // Forward a port
    let local_port = device.forward(0, 8080).await.unwrap();
    assert!(local_port > 0);
    println!("Forwarded localhost:{local_port} -> device:8080");

    // List forwards
    let forwards = device.forward_list().await.unwrap();
    let found = forwards.iter().any(|f| f.local_port() == Some(local_port));
    assert!(found, "forward not found in list");

    // Cleanup
    device.forward_remove(local_port).await.unwrap();
}

// ══════════════════════════════════════════════════════════════
//  New tests: shell2 (exit code)
// ══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_shell2_success() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    let device = match server.device().await {
        Ok(d) => d,
        Err(_) => return,
    };

    let result = device.shell2("echo hello").await.unwrap();
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("hello"));
    println!("shell2: exit_code={}, stdout={}", result.exit_code, result.stdout.trim());
}

#[tokio::test]
async fn test_shell2_failure() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    let device = match server.device().await {
        Ok(d) => d,
        Err(_) => return,
    };

    let result = device.shell2("ls /nonexistent_path_12345").await.unwrap();
    assert_ne!(result.exit_code, 0);
    println!("shell2 failure: exit_code={}", result.exit_code);
}

// ══════════════════════════════════════════════════════════════
//  New tests: system properties
// ══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_getprop() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    let device = match server.device().await {
        Ok(d) => d,
        Err(_) => return,
    };

    let sdk = device.getprop("ro.build.version.sdk").await.unwrap();
    assert!(!sdk.is_empty());
    println!("SDK: {sdk}");

    let model = device.prop_model().await.unwrap();
    assert!(!model.is_empty());
    println!("Model: {model}");
}

// ══════════════════════════════════════════════════════════════
//  New tests: reverse port forwarding
// ══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_reverse_forward() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    let device = match server.device().await {
        Ok(d) => d,
        Err(_) => return,
    };

    // Create reverse forward
    device.reverse(9999, 8888).await.unwrap();
    println!("Reverse: device:9999 -> host:8888");

    // List reverses
    let reverses = device.reverse_list().await.unwrap();
    println!("Reverse entries: {}", reverses.len());
    for r in &reverses {
        println!("  {} -> {}", r.remote, r.local);
    }

    // Cleanup
    device.reverse_remove(9999).await.unwrap();
    println!("Reverse removed");

    // Verify removal
    let reverses = device.reverse_list().await.unwrap();
    let found = reverses.iter().any(|r| r.remote_port() == Some(9999));
    assert!(!found, "reverse should have been removed");
}

// ══════════════════════════════════════════════════════════════
//  New tests: file sync protocol
// ══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_file_stat() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    let device = match server.device().await {
        Ok(d) => d,
        Err(_) => return,
    };

    // /system/build.prop always exists on Android
    let stat = device.stat("/system/build.prop").await.unwrap();
    assert!(stat.exists());
    assert!(stat.is_file());
    assert!(!stat.is_dir());
    assert!(stat.size > 0);
    println!("build.prop: mode={:o}, size={}, mtime={}", stat.mode, stat.size, stat.mtime);
}

#[tokio::test]
async fn test_file_stat_dir() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    let device = match server.device().await {
        Ok(d) => d,
        Err(_) => return,
    };

    let stat = device.stat("/system").await.unwrap();
    assert!(stat.exists());
    assert!(stat.is_dir());
    assert!(!stat.is_file());
    println!("/system: mode={:o}", stat.mode);
}

#[tokio::test]
async fn test_file_stat_not_found() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    let device = match server.device().await {
        Ok(d) => d,
        Err(_) => return,
    };

    let stat = device.stat("/nonexistent_file_12345").await.unwrap();
    assert!(!stat.exists());
    println!("Not found: mode={}", stat.mode);
}

#[tokio::test]
async fn test_file_list_dir() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    let device = match server.device().await {
        Ok(d) => d,
        Err(_) => return,
    };

    let entries = device.list_dir("/system").await.unwrap();
    assert!(!entries.is_empty());
    println!("/system has {} entries:", entries.len());
    for e in entries.iter().take(10) {
        let kind = if e.is_dir() { "DIR" } else { "FILE" };
        println!("  [{kind}] {} ({} bytes)", e.name, e.size);
    }
}

#[tokio::test]
async fn test_file_pull_bytes() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    let device = match server.device().await {
        Ok(d) => d,
        Err(_) => return,
    };

    // Push a known file first, then pull it via sync protocol
    let test_content = b"pull_bytes test content\n";
    let remote = "/data/local/tmp/_droidrun_pull_test.txt";
    device.push_bytes(test_content, remote).await.unwrap();

    let data = device.pull_bytes(remote).await.unwrap();
    assert_eq!(&data, test_content);
    println!("pull_bytes: {} bytes — content verified", data.len());

    // Cleanup
    device.remove(remote).await.unwrap();
}

#[tokio::test]
async fn test_file_push_pull_roundtrip() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    let device = match server.device().await {
        Ok(d) => d,
        Err(_) => return,
    };

    let test_data = b"hello from droidrun-adb push_bytes test!\n";
    let remote_path = "/data/local/tmp/_droidrun_test_push.txt";

    // Push bytes
    device.push_bytes(test_data, remote_path).await.unwrap();
    println!("Pushed {} bytes to {remote_path}", test_data.len());

    // Pull back
    let pulled = device.pull_bytes(remote_path).await.unwrap();
    assert_eq!(&pulled, test_data);
    println!("Pulled {} bytes — content matches!", pulled.len());

    // Pull to local file
    let local_tmp = std::env::temp_dir().join("_droidrun_test_pull.txt");
    device.pull(remote_path, &local_tmp).await.unwrap();
    let local_data = std::fs::read(&local_tmp).unwrap();
    assert_eq!(&local_data, test_data);
    println!("Pull to local file — matches!");

    // Cleanup
    device.remove(remote_path).await.unwrap();
    let _ = std::fs::remove_file(&local_tmp);
}

// ══════════════════════════════════════════════════════════════
//  New tests: file operations (shell-based)
// ══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_file_exists() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    let device = match server.device().await {
        Ok(d) => d,
        Err(_) => return,
    };

    assert!(device.exists("/system/build.prop").await.unwrap());
    assert!(!device.exists("/nonexistent_file_12345").await.unwrap());
    println!("exists() works correctly");
}

// ══════════════════════════════════════════════════════════════
//  New tests: app management
// ══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_app_current() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    let device = match server.device().await {
        Ok(d) => d,
        Err(_) => return,
    };

    match device.app_current().await {
        Ok(app) => {
            println!("Current app: {}", app);
            assert!(!app.package.is_empty());
        }
        Err(e) => {
            println!("app_current failed (may be on lock screen): {e}");
        }
    }
}

#[tokio::test]
async fn test_app_stop() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    let device = match server.device().await {
        Ok(d) => d,
        Err(_) => return,
    };

    // Force stop settings (harmless, always installed)
    device.app_stop("com.android.settings").await.unwrap();
    println!("app_stop: com.android.settings stopped");
}

// ══════════════════════════════════════════════════════════════
//  New tests: screen & display
// ══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_window_size() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    let device = match server.device().await {
        Ok(d) => d,
        Err(_) => return,
    };

    let size = device.window_size().await.unwrap();
    assert!(size.width > 0);
    assert!(size.height > 0);
    println!("Window size: {size}");
}

#[tokio::test]
async fn test_rotation() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    let device = match server.device().await {
        Ok(d) => d,
        Err(_) => return,
    };

    let rotation = device.rotation().await.unwrap();
    assert!(rotation <= 3);
    println!("Rotation: {rotation}");
}

#[tokio::test]
async fn test_is_screen_on() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    let device = match server.device().await {
        Ok(d) => d,
        Err(_) => return,
    };

    let on = device.is_screen_on().await.unwrap();
    println!("Screen on: {on}");
}

// ══════════════════════════════════════════════════════════════
//  New tests: device info
// ══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_get_features() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    let device = match server.device().await {
        Ok(d) => d,
        Err(_) => return,
    };

    let features = device.get_features().await.unwrap();
    assert!(!features.is_empty());
    println!("Features: {:?}", features);
}

#[tokio::test]
async fn test_get_serialno() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    let device = match server.device().await {
        Ok(d) => d,
        Err(_) => return,
    };

    let serialno = device.get_serialno().await.unwrap();
    assert!(!serialno.is_empty());
    println!("Serial: {serialno}");
}

// ══════════════════════════════════════════════════════════════
//  New tests: server-level forward list
// ══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_server_forward_list_all() {
    if skip_if_no_device() {
        return;
    }
    let server = AdbServer::default();
    let all_forwards = server.forward_list_all().await.unwrap();
    println!("All forwards across all devices: {}", all_forwards.len());
    for f in &all_forwards {
        println!("  {} {} -> {}", f.serial, f.local, f.remote);
    }
}
