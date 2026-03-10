//! Portal Setup & Health Check — install, configure, and verify Portal.
//!
//! Demonstrates PortalManager for APK lifecycle and PortalClient for
//! direct Portal communication.
//!
//! ```bash
//! cargo run -p droidrun-core --example portal_setup
//! ```

use droidrun_adb::AdbServer;
use droidrun_core::portal::client::PortalClient;
use droidrun_core::portal::manager::PortalManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    let server = AdbServer::default();
    let device = server.device().await?;
    println!("Device: {}", device.serial);

    // ── Check Current State ──────────────────────────────────────
    let packages = device.list_packages(&[]).await?;
    let is_installed = packages.iter().any(|p| p == "com.droidrun.portal");
    println!("Portal installed: {is_installed}");

    if !is_installed {
        // ── Install Portal ───────────────────────────────────────
        println!("\nInstalling Portal APK...");
        let manager = PortalManager::new(device.clone());
        let sdk = device.shell("getprop ro.build.version.sdk").await?;
        let sdk = sdk.trim();
        println!("Device SDK: {sdk}");

        // setup() downloads the APK, installs it, enables accessibility,
        // and configures the keyboard
        manager.setup(sdk, true).await?;
        println!("Portal setup complete!");
    }

    // ── Direct Portal Communication ──────────────────────────────
    // PortalClient communicates directly with the Portal APK
    let mut client = PortalClient::new(device.clone(), true, 8080);
    client.connect().await?;

    // Ping
    let ping = client.ping().await?;
    let method = ping.get("method").and_then(|v| v.as_str()).unwrap_or("?");
    println!("\nPortal reachable via: {method}");

    // Version
    let version = client.get_version().await?;
    println!("Portal version: {version}");

    // Screenshot via Portal (with overlay control)
    let png = client.take_screenshot(true).await?;
    println!("Portal screenshot: {} bytes", png.len());

    // Get accessibility state
    let state = client.get_state().await?;
    let keys: Vec<&String> = state.as_object().map(|o| o.keys().collect()).unwrap_or_default();
    println!("State keys: {keys:?}");

    // Get apps with labels
    let apps = client.get_apps(false).await?;
    println!("Apps (non-system): {}", apps.len());
    for app in apps.iter().take(5) {
        println!("  {} ({})", app.label, app.package);
    }

    // Text input via Portal keyboard
    let result = client.input_text("test input", false).await?;
    println!("Input text result: {result}");

    // ── Health Check (ensure_ready) ──────────────────────────────
    // This checks version compatibility, accessibility service,
    // and auto-fixes any issues. It will only upgrade, never downgrade.
    println!("\nRunning health check...");
    let manager = PortalManager::new(device.clone());
    let sdk = device.shell("getprop ro.build.version.sdk").await?;
    match manager.ensure_ready(sdk.trim(), false).await {
        Ok(()) => println!("Health check passed!"),
        Err(e) => println!("Health check warning: {e} (portal may still work)"),
    }

    Ok(())
}
