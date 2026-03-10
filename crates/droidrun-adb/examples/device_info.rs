//! Device Info — system properties, screen, rotation, features, network.
//!
//! ```bash
//! cargo run -p droidrun-adb --example device_info
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

    // ── System properties ───────────────────────────────────────
    println!("\n--- System Properties ---");
    let model = device.prop_model().await?;
    let name = device.prop_name().await?;
    let codename = device.prop_device().await?;
    let sdk = device.getprop("ro.build.version.sdk").await?;
    let release = device.getprop("ro.build.version.release").await?;
    let brand = device.getprop("ro.product.brand").await?;

    println!("  Brand:    {brand}");
    println!("  Model:    {model}");
    println!("  Name:     {name}");
    println!("  Codename: {codename}");
    println!("  Android:  {release} (SDK {sdk})");

    // ── Serial number ───────────────────────────────────────────
    println!("\n--- Device Identity ---");
    let serialno = device.get_serialno().await?;
    println!("  Serial: {serialno}");

    let state = device.get_state().await?;
    println!("  State:  {state}");

    let date = device.get_date().await?;
    println!("  Date:   {date}");

    // ── Features ────────────────────────────────────────────────
    println!("\n--- ADB Features ---");
    let features = device.get_features().await?;
    for f in &features {
        println!("  - {f}");
    }

    // ── Screen info ─────────────────────────────────────────────
    println!("\n--- Screen ---");
    let size = device.window_size().await?;
    println!("  Size:     {size}");

    let rotation = device.rotation().await?;
    let rot_name = match rotation {
        0 => "Natural (portrait)",
        1 => "Left (landscape)",
        2 => "Inverted",
        3 => "Right (landscape)",
        _ => "Unknown",
    };
    println!("  Rotation: {rotation} ({rot_name})");

    let screen_on = device.is_screen_on().await?;
    println!("  Screen:   {}", if screen_on { "ON" } else { "OFF" });

    // ── Network ─────────────────────────────────────────────────
    println!("\n--- Network ---");
    match device.wlan_ip().await {
        Ok(ip) => println!("  WLAN IP: {ip}"),
        Err(_) => println!("  WLAN IP: not available (emulator or WiFi off)"),
    }

    // ── Shell2 with exit code ───────────────────────────────────
    println!("\n--- Shell2 (exit code) ---");
    let result = device.shell2("echo 'hello'; exit 0").await?;
    println!("  stdout: {}", result.stdout.trim());
    println!("  exit_code: {}", result.exit_code);

    let result = device.shell2("ls /nonexistent_12345").await?;
    println!("  error exit_code: {}", result.exit_code);

    println!("\nDone!");
    Ok(())
}
