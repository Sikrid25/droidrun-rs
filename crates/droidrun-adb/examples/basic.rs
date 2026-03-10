//! Basic ADB operations — connect, shell, device info.
//!
//! ```bash
//! cargo run -p droidrun-adb --example basic
//! ```

use droidrun_adb::AdbServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create server handle (default: localhost:5037)
    let server = AdbServer::default();

    // ── Server Info ──────────────────────────────────────────────
    let version = server.version().await?;
    println!("ADB server version: {version}");

    // ── List Devices ─────────────────────────────────────────────
    let devices = server.devices().await?;
    if devices.is_empty() {
        println!("No devices connected. Run: adb start-server");
        return Ok(());
    }

    println!("\nConnected devices:");
    for d in &devices {
        println!("  {:<30} {}", d.serial, d.state);
    }

    // ── Get First Online Device ──────────────────────────────────
    let device = server.device().await?;
    println!("\nUsing device: {}", device.serial);

    // ── Device State ─────────────────────────────────────────────
    let state = device.get_state().await?;
    println!("State: {state} (online: {})", state.is_online());

    // ── Shell Commands ───────────────────────────────────────────
    let sdk = device.shell("getprop ro.build.version.sdk").await?;
    println!("SDK version: {}", sdk.trim());

    let model = device.shell("getprop ro.product.model").await?;
    println!("Model: {}", model.trim());

    let date = device.get_date().await?;
    println!("Device date: {date}");

    // ── List Packages ────────────────────────────────────────────
    let packages = device.list_packages(&["-3"]).await?; // third-party only
    println!("\nInstalled apps ({} third-party):", packages.len());
    for p in packages.iter().take(10) {
        println!("  {p}");
    }
    if packages.len() > 10 {
        println!("  ... and {} more", packages.len() - 10);
    }

    Ok(())
}
