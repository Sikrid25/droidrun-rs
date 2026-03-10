//! AndroidDriver basics — connect, tap, type, screenshot, apps.
//!
//! Demonstrates the high-level DeviceDriver API that wraps ADB + Portal.
//!
//! ```bash
//! cargo run -p droidrun-core --example driver_basics
//! ```

use droidrun_core::driver::android::AndroidDriver;
use droidrun_core::driver::DeviceDriver;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for debug output
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    // ── Connect ──────────────────────────────────────────────────
    // serial: None = auto-detect first device
    // use_tcp: true = prefer fast TCP mode for Portal communication
    let mut driver = AndroidDriver::new(None, true);
    driver.connect().await?;
    println!("Connected!");

    // ── Device Date ──────────────────────────────────────────────
    let date = driver.get_date().await?;
    println!("Device date: {date}");

    // ── Screenshot ───────────────────────────────────────────────
    // hide_overlay: true = hide Portal overlay before capturing
    let png = driver.screenshot(true).await?;
    tokio::fs::write("driver_screenshot.png", &png).await?;
    println!("Screenshot: {} bytes -> driver_screenshot.png", png.len());

    // ── Go to Home Screen ────────────────────────────────────────
    driver.press_key(3).await?; // KEYCODE_HOME
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    println!("Home screen");

    // ── Get UI Tree ──────────────────────────────────────────────
    let tree = driver.get_ui_tree().await?;
    let keys: Vec<&String> = tree.as_object().map(|o| o.keys().collect()).unwrap_or_default();
    println!("UI tree keys: {keys:?}");

    // ── Tap ──────────────────────────────────────────────────────
    println!("Tapping at center...");
    driver.tap(540, 1200).await?;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // ── Swipe ────────────────────────────────────────────────────
    println!("Swiping up...");
    driver.swipe(540, 1800, 540, 600, 300).await?;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // ── List Apps ────────────────────────────────────────────────
    // Gets app labels from Portal (richer than plain package names)
    let apps = driver.get_apps(false).await?; // exclude system apps
    println!("\nInstalled apps ({}):", apps.len());
    for app in &apps {
        println!("  {:<50} {}", app.package, app.label);
    }

    // ── List Packages ────────────────────────────────────────────
    // Gets package names from ADB (faster, no labels)
    let packages = driver.list_packages(false).await?;
    println!("\nPackage count: {}", packages.len());

    // ── Text Input ───────────────────────────────────────────────
    // Uses Portal's keyboard IME (supports Unicode, fast)
    // Note: returns false if no text field is focused
    let typed = driver.input_text("Hello from droidrun-rs!", false).await?;
    println!("Input text result: {typed}");

    // ── Go Home Again ────────────────────────────────────────────
    driver.press_key(3).await?;
    println!("\nDone!");

    Ok(())
}
