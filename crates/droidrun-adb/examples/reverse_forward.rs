//! Reverse Port Forwarding — create, list, and remove device→host forwards.
//!
//! ```bash
//! cargo run -p droidrun-adb --example reverse_forward
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

    // ── Create reverse forwards ─────────────────────────────────
    println!("\n1. Creating reverse port forwards...");
    device.reverse(9001, 3001).await?;
    println!("   device:9001 -> host:3001");

    device.reverse(9002, 3002).await?;
    println!("   device:9002 -> host:3002");

    // ── List reverse forwards ───────────────────────────────────
    println!("\n2. Listing reverse forwards:");
    let reverses = device.reverse_list().await?;
    for r in &reverses {
        println!("   {} -> {}", r.remote, r.local);
    }
    println!("   Total: {}", reverses.len());

    // ── Remove one ──────────────────────────────────────────────
    println!("\n3. Removing device:9001...");
    device.reverse_remove(9001).await?;

    let reverses = device.reverse_list().await?;
    println!("   Remaining forwards: {}", reverses.len());
    for r in &reverses {
        println!("   {} -> {}", r.remote, r.local);
    }

    // ── Remove all ──────────────────────────────────────────────
    println!("\n4. Removing all reverse forwards...");
    device.reverse_remove_all().await?;

    let reverses = device.reverse_list().await?;
    println!("   Remaining: {}", reverses.len());
    assert!(reverses.is_empty() || !reverses.iter().any(|r| r.remote_port() == Some(9002)));

    // ── Compare with regular forwards ───────────────────────────
    println!("\n5. Regular (host→device) forward for comparison:");
    let port = device.forward(0, 8080).await?;
    println!("   host:{port} -> device:8080");

    let forwards = device.forward_list().await?;
    for f in &forwards {
        println!("   {} -> {}", f.local, f.remote);
    }

    // Server-level forward list
    println!("\n6. Server-level forward list (all devices):");
    let all = server.forward_list_all().await?;
    for f in &all {
        println!("   [{}] {} -> {}", f.serial, f.local, f.remote);
    }

    // Cleanup
    device.forward_remove(port).await?;
    println!("\nDone!");
    Ok(())
}
