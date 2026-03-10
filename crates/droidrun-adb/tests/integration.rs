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
