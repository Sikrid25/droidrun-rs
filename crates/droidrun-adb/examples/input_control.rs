//! Device input control — tap, swipe, key events.
//!
//! ```bash
//! cargo run -p droidrun-adb --example input_control
//! ```

use droidrun_adb::AdbServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = AdbServer::default();
    let device = server.device().await?;
    println!("Device: {}", device.serial);

    // ── Key Events ───────────────────────────────────────────────
    // Common Android key codes:
    //   3  = Home
    //   4  = Back
    //   24 = Volume Up
    //   25 = Volume Down
    //   26 = Power
    //   66 = Enter
    //   82 = Menu

    println!("\nPressing Home...");
    device.keyevent(3).await?;
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // ── Tap ──────────────────────────────────────────────────────
    // Tap the center of the screen (approximate for most devices)
    println!("Tapping at (540, 1200)...");
    device.tap(540, 1200).await?;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // ── Swipe ────────────────────────────────────────────────────
    // Swipe up (like scrolling a page down)
    println!("Swiping up...");
    device.swipe(540, 1600, 540, 400, 300).await?;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Swipe down (like scrolling a page up)
    println!("Swiping down...");
    device.swipe(540, 400, 540, 1600, 300).await?;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // ── Text Input (via ADB shell) ───────────────────────────────
    // Note: this uses basic ADB text input which has limitations.
    // For better text input, use droidrun-core's Portal keyboard.
    println!("Typing text via shell...");
    device.shell("input text 'hello'").await?;

    // ── Go Home ──────────────────────────────────────────────────
    println!("Going home...");
    device.keyevent(3).await?;

    println!("\nDone!");
    Ok(())
}
