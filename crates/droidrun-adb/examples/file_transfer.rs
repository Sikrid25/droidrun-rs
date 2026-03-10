//! File Transfer — push, stat, list_dir, pull, and compare.
//!
//! ```bash
//! cargo run -p droidrun-adb --example file_transfer
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

    let remote_dir = "/data/local/tmp";
    let remote_path = format!("{remote_dir}/_droidrun_file_test.txt");
    let test_content = b"Hello from droidrun-adb file transfer example!\nLine 2\nLine 3\n";

    // ── Push bytes ──────────────────────────────────────────────
    println!("\n1. Pushing {} bytes...", test_content.len());
    device.push_bytes(test_content, &remote_path).await?;
    println!("   Pushed to {remote_path}");

    // ── Stat ────────────────────────────────────────────────────
    println!("\n2. Stat:");
    let stat = device.stat(&remote_path).await?;
    println!("   mode={:o}, size={}, is_file={}", stat.mode, stat.size, stat.is_file());
    assert!(stat.exists());
    assert!(stat.is_file());

    // ── Exists ──────────────────────────────────────────────────
    println!("\n3. Exists check:");
    let exists = device.exists(&remote_path).await?;
    println!("   {remote_path} exists: {exists}");
    assert!(exists);

    let not_exists = device.exists("/nonexistent_12345").await?;
    println!("   /nonexistent_12345 exists: {not_exists}");
    assert!(!not_exists);

    // ── List directory ──────────────────────────────────────────
    println!("\n4. List {remote_dir}:");
    let entries = device.list_dir(remote_dir).await?;
    println!("   {} entries:", entries.len());
    for e in entries.iter().take(10) {
        let kind = if e.is_dir() { "DIR " } else { "FILE" };
        println!("   [{kind}] {} ({} bytes)", e.name, e.size);
    }
    if entries.len() > 10 {
        println!("   ... and {} more", entries.len() - 10);
    }

    // ── Pull bytes ──────────────────────────────────────────────
    println!("\n5. Pull bytes:");
    let pulled = device.pull_bytes(&remote_path).await?;
    assert_eq!(&pulled, test_content);
    println!("   Pulled {} bytes — content matches!", pulled.len());

    // ── Pull to local file ──────────────────────────────────────
    println!("\n6. Pull to local file:");
    let local_tmp = std::env::temp_dir().join("_droidrun_pulled.txt");
    device.pull(&remote_path, &local_tmp).await?;
    let local_data = std::fs::read(&local_tmp)?;
    assert_eq!(&local_data, test_content);
    println!("   Saved to {} — verified!", local_tmp.display());

    // ── Cleanup ─────────────────────────────────────────────────
    println!("\n7. Cleanup:");
    device.remove(&remote_path).await?;
    println!("   Removed {remote_path}");
    let _ = std::fs::remove_file(&local_tmp);
    println!("   Removed {}", local_tmp.display());

    println!("\nAll file transfer operations passed!");
    Ok(())
}
