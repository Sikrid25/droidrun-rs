//! Element Search — composable filters for finding UI elements.
//!
//! Demonstrates how to search the accessibility tree using
//! text matching, ID matching, and spatial filters.
//!
//! ```bash
//! cargo run -p droidrun-core --example element_search
//! ```

use droidrun_core::driver::android::AndroidDriver;
use droidrun_core::driver::DeviceDriver;
use droidrun_core::ui::search::{
    clickable, compose, flatten_tree, get_element_center, has_text, id_matches, text_matches,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    // Connect
    let mut driver = AndroidDriver::new(None, true);
    driver.connect().await?;

    // Go to home screen for consistent results
    driver.press_key(3).await?;
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // ── Get Raw UI Tree ──────────────────────────────────────────
    let tree = driver.get_ui_tree().await?;
    let a11y_tree = tree.get("a11y_tree").unwrap_or(&tree);

    // Flatten into a list for searching
    let all_nodes = flatten_tree(a11y_tree);
    println!("Total nodes in tree: {}", all_nodes.len());

    // ── Search by Text ───────────────────────────────────────────
    // Find elements containing specific text
    let chrome_filter = text_matches("Chrome");
    let chrome_results = chrome_filter(&[a11y_tree.clone()]);
    println!("\nElements matching 'Chrome': {}", chrome_results.len());
    for node in &chrome_results {
        let text = node.get("text").and_then(|v| v.as_str()).unwrap_or("");
        let class = node.get("className").and_then(|v| v.as_str()).unwrap_or("");
        let (cx, cy) = get_element_center(node);
        println!("  [{class}] '{text}' at ({cx}, {cy})");
    }

    // ── Search by Resource ID ────────────────────────────────────
    // Supports both full IDs and short names (after /)
    let id_filter = id_matches("icon");
    let id_results = id_filter(&[a11y_tree.clone()]);
    println!("\nElements with ID containing 'icon': {}", id_results.len());
    for node in id_results.iter().take(5) {
        let id = node.get("resourceId").and_then(|v| v.as_str()).unwrap_or("");
        let text = node.get("text").and_then(|v| v.as_str()).unwrap_or("");
        println!("  {id} -> '{text}'");
    }
    if id_results.len() > 5 {
        println!("  ... and {} more", id_results.len() - 5);
    }

    // ── Find Clickable Elements ──────────────────────────────────
    let clickable_filter = clickable();
    let clickable_results = clickable_filter(&[a11y_tree.clone()]);
    println!("\nClickable elements: {}", clickable_results.len());
    for node in clickable_results.iter().take(5) {
        let class = node.get("className").and_then(|v| v.as_str()).unwrap_or("");
        let text = node.get("text").and_then(|v| v.as_str()).unwrap_or("");
        let (cx, cy) = get_element_center(node);
        println!("  [{class}] '{text}' at ({cx}, {cy})");
    }

    // ── Find Elements with Text ──────────────────────────────────
    let text_filter = has_text();
    let text_results = text_filter(&[a11y_tree.clone()]);
    println!("\nElements with text: {}", text_results.len());
    for node in text_results.iter().take(5) {
        let text = node.get("text").and_then(|v| v.as_str()).unwrap_or("");
        let desc = node
            .get("contentDescription")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let display = if !text.is_empty() { text } else { desc };
        println!("  '{display}'");
    }

    // ── Compose Filters ──────────────────────────────────────────
    // Chain multiple filters: clickable + has text
    let composed = compose(vec![clickable(), has_text()]);
    let composed_results = composed(&[a11y_tree.clone()]);
    println!(
        "\nClickable elements WITH text: {}",
        composed_results.len()
    );
    for node in &composed_results {
        let text = node.get("text").and_then(|v| v.as_str()).unwrap_or("");
        let (cx, cy) = get_element_center(node);
        println!("  '{text}' at ({cx}, {cy})");
    }

    // ── Tap a Found Element ──────────────────────────────────────
    if let Some(target) = composed_results.first() {
        let (cx, cy) = get_element_center(target);
        let text = target.get("text").and_then(|v| v.as_str()).unwrap_or("?");
        println!("\nTapping '{text}' at ({cx}, {cy})...");
        driver.tap(cx, cy).await?;
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // Go back home
        driver.press_key(3).await?;
    }

    println!("\nDone!");
    Ok(())
}
