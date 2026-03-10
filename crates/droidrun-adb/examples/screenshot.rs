//! Take a screenshot via ADB and save as PNG.
//!
//! ```bash
//! cargo run -p droidrun-adb --example screenshot
//! cargo run -p droidrun-adb --example screenshot -- output.png
//! ```

use droidrun_adb::AdbServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = std::env::args().nth(1).unwrap_or("screenshot.png".into());

    let server = AdbServer::default();
    let device = server.device().await?;
    println!("Device: {}", device.serial);

    // Take screenshot (returns raw PNG bytes)
    println!("Taking screenshot...");
    let png = device.screencap().await?;

    // Verify it's a valid PNG (magic bytes: 89 50 4E 47)
    if png.len() > 4 && png[..4] == [0x89, 0x50, 0x4E, 0x47] {
        println!("Valid PNG: {} bytes ({:.1} KB)", png.len(), png.len() as f64 / 1024.0);
    } else {
        eprintln!("Warning: data doesn't look like a PNG ({} bytes)", png.len());
    }

    // Save to file
    tokio::fs::write(&output_path, &png).await?;
    println!("Saved to: {output_path}");

    Ok(())
}
