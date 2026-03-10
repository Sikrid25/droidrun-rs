//! Port forwarding — set up, list, and clean up TCP port forwards.
//!
//! ```bash
//! cargo run -p droidrun-adb --example port_forward
//! ```

use droidrun_adb::AdbServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = AdbServer::default();
    let device = server.device().await?;
    println!("Device: {}", device.serial);

    // ── List Existing Forwards ───────────────────────────────────
    let forwards = device.forward_list().await?;
    println!("\nExisting forwards ({}):", forwards.len());
    for f in &forwards {
        println!("  {} -> {}", f.local, f.remote);
    }

    // ── Create a Forward (dynamic port allocation) ───────────────
    // local_port=0 means "let ADB server pick a free port"
    let local_port = device.forward(0, 8080).await?;
    println!("\nCreated forward: localhost:{local_port} -> device:8080");

    // ── Create a Forward (fixed port) ────────────────────────────
    let fixed_port = device.forward(19999, 9090).await?;
    println!("Created forward: localhost:{fixed_port} -> device:9090");

    // ── List Again ───────────────────────────────────────────────
    let forwards = device.forward_list().await?;
    println!("\nAll forwards ({}):", forwards.len());
    for f in &forwards {
        let local = f.local_port().map(|p| p.to_string()).unwrap_or("?".into());
        let remote = f.remote_port().map(|p| p.to_string()).unwrap_or("?".into());
        println!("  localhost:{local} -> device:{remote}");
    }

    // ── Cleanup ──────────────────────────────────────────────────
    device.forward_remove(local_port).await?;
    println!("\nRemoved forward on port {local_port}");

    device.forward_remove(fixed_port).await?;
    println!("Removed forward on port {fixed_port}");

    let remaining = device.forward_list().await?;
    println!("Remaining forwards: {}", remaining.len());

    Ok(())
}
