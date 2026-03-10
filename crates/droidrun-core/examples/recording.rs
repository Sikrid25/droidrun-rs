//! RecordingDriver — record all actions as JSON for macro replay.
//!
//! Wraps any DeviceDriver in a transparent proxy that logs
//! every mutating action (tap, swipe, text, key, etc.).
//!
//! ```bash
//! cargo run -p droidrun-core --example recording
//! ```

use droidrun_core::driver::android::AndroidDriver;
use droidrun_core::driver::recording::RecordingDriver;
use droidrun_core::driver::DeviceDriver;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    // ── Create inner driver and connect ──────────────────────────
    let mut inner = AndroidDriver::new(None, true);
    inner.connect().await?;
    println!("Connected to device");

    // ── Wrap in RecordingDriver ──────────────────────────────────
    let recorder = RecordingDriver::new(inner);

    // ── Perform some actions ─────────────────────────────────────
    // All mutating actions are recorded automatically
    println!("\nPerforming actions...");

    // Go home
    recorder.press_key(3).await?;
    println!("  [recorded] press_key(3) — Home");
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Tap
    recorder.tap(540, 1200).await?;
    println!("  [recorded] tap(540, 1200)");
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Swipe up
    recorder.swipe(540, 1800, 540, 600, 300).await?;
    println!("  [recorded] swipe(540,1800 -> 540,600, 300ms)");
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Text input
    recorder.input_text("automation test", false).await?;
    println!("  [recorded] input_text('automation test')");

    // These are NOT recorded (read-only operations):
    let _screenshot = recorder.screenshot(true).await?;
    println!("  [not recorded] screenshot (read-only)");

    let _tree = recorder.get_ui_tree().await?;
    println!("  [not recorded] get_ui_tree (read-only)");

    // Go home
    recorder.press_key(3).await?;
    println!("  [recorded] press_key(3) — Home");

    // ── View Recorded Actions ────────────────────────────────────
    let actions = recorder.recorded_actions();
    println!("\n--- Recorded {} actions ---", actions.len());

    // Pretty-print as JSON
    let json = recorder.to_json()?;
    println!("{json}");

    // ── Save to File ─────────────────────────────────────────────
    tokio::fs::write("recorded_actions.json", &json).await?;
    println!("\nSaved to recorded_actions.json");

    // ── Clear and Continue ───────────────────────────────────────
    recorder.clear_log();
    println!("Log cleared ({} actions)", recorder.recorded_actions().len());

    Ok(())
}
