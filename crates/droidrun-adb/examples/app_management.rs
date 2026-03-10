//! App Management — current app, app info, stop, list packages.
//!
//! ```bash
//! cargo run -p droidrun-adb --example app_management
//! ```

use droidrun_adb::AdbServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    let server = AdbServer::default();
    let device = server.device().await?;
    println!("Device: {}", device.serial);

    // ── Current foreground app ──────────────────────────────────
    println!("\n1. Current foreground app:");
    match device.app_current().await {
        Ok(app) => println!("   {}", app),
        Err(e) => println!("   Could not determine: {e}"),
    }

    // ── List packages ───────────────────────────────────────────
    println!("\n2. Third-party packages:");
    let packages = device.list_packages(&["-3"]).await?;
    for p in &packages {
        println!("   {p}");
    }
    println!("   Total: {}", packages.len());

    // ── App info ────────────────────────────────────────────────
    println!("\n3. App details (com.android.settings):");
    let info = device.app_info("com.android.settings").await?;
    println!("   Package: {}", info.package);
    println!("   Version: {}", info.version_name.as_deref().unwrap_or("?"));
    println!("   Version Code: {:?}", info.version_code);
    println!("   Install Path: {}", info.install_path.as_deref().unwrap_or("?"));

    // ── Start app ───────────────────────────────────────────────
    println!("\n4. Starting Settings...");
    let result = device.app_start("com.android.settings", None).await?;
    println!("   {}", result.trim());

    // Wait a moment
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // ── Check current app ───────────────────────────────────────
    println!("\n5. Current app after launch:");
    match device.app_current().await {
        Ok(app) => println!("   {}", app),
        Err(e) => println!("   Could not determine: {e}"),
    }

    // ── Stop app ────────────────────────────────────────────────
    println!("\n6. Stopping Settings...");
    device.app_stop("com.android.settings").await?;
    println!("   Stopped!");

    // ── Clear app data ──────────────────────────────────────────
    println!("\n7. Clearing Settings data...");
    let clear_result = device.app_clear("com.android.settings").await?;
    println!("   Result: {clear_result}");

    println!("\nDone!");
    Ok(())
}
