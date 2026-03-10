//! App Automation — launch app, navigate UI, interact with elements.
//!
//! A practical example showing how to automate an app workflow:
//! open an app, wait for it to load, read the UI, and interact.
//!
//! ```bash
//! cargo run -p droidrun-core --example app_automation
//! ```

use droidrun_core::driver::android::AndroidDriver;
use droidrun_core::driver::DeviceDriver;
use droidrun_core::ui::filter::ConciseFilter;
use droidrun_core::ui::formatter::IndexedFormatter;
use droidrun_core::ui::provider::AndroidStateProvider;
use droidrun_core::ui::search::{clickable, get_element_center, text_matches};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    let mut driver = AndroidDriver::new(None, true);
    driver.connect().await?;
    println!("Connected!");

    // ── Step 1: Go Home ──────────────────────────────────────────
    driver.press_key(3).await?;
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // ── Step 2: List Available Apps ──────────────────────────────
    let apps = driver.get_apps(false).await?;
    println!("\nAvailable apps:");
    for (i, app) in apps.iter().enumerate() {
        println!("  {}. {} ({})", i + 1, app.label, app.package);
    }

    // ── Step 3: Open Settings ────────────────────────────────────
    let target_package = "com.android.settings";
    println!("\nOpening {target_package}...");
    let result = driver.start_app(target_package, None).await?;
    println!("Result: {}", result.trim());

    // Wait for app to load
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // ── Step 4: Read UI State ────────────────────────────────────
    let provider = AndroidStateProvider::new(ConciseFilter, IndexedFormatter, false);
    let state = provider.get_state(&driver).await?;

    println!("\nCurrent app: {} ({})", state.phone_state.current_app, state.phone_state.package_name);
    println!("Elements on screen: {}", state.elements.len());
    println!("\n{}", state.formatted_text);

    // ── Step 5: Find and Tap an Element ──────────────────────────
    // Search for a clickable element in the raw tree
    let tree = driver.get_ui_tree().await?;
    let a11y = tree.get("a11y_tree").unwrap_or(&tree);

    // Find all clickable elements
    let clickables = clickable()(&[a11y.clone()]);
    let with_text: Vec<_> = clickables
        .iter()
        .filter(|n| {
            n.get("text")
                .and_then(|v| v.as_str())
                .map(|s| !s.is_empty())
                .unwrap_or(false)
        })
        .collect();

    println!("\nClickable items with text:");
    for (i, node) in with_text.iter().take(10).enumerate() {
        let text = node.get("text").and_then(|v| v.as_str()).unwrap_or("");
        let (cx, cy) = get_element_center(node);
        println!("  {}. '{}' at ({}, {})", i + 1, text, cx, cy);
    }

    // ── Step 6: Search for Specific Text ─────────────────────────
    // Try to find "Network" or "Wi-Fi" or "Display" in Settings
    let search_terms = ["Network", "Wi-Fi", "Display", "Battery", "About"];
    for term in search_terms {
        let filter = text_matches(term);
        let results = filter(&[a11y.clone()]);
        if !results.is_empty() {
            let (cx, cy) = get_element_center(&results[0]);
            println!("\nFound '{term}' at ({cx}, {cy})");
            println!("Tapping...");
            driver.tap(cx, cy).await?;
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;

            // Read new state
            let new_state = provider.get_state(&driver).await?;
            println!("Now in: {} ({})", new_state.phone_state.current_app, new_state.phone_state.package_name);
            println!("Elements: {}", new_state.elements.len());
            break;
        }
    }

    // ── Step 7: Go Back and Home ─────────────────────────────────
    println!("\nGoing back...");
    driver.press_key(4).await?; // KEYCODE_BACK
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    driver.press_key(3).await?; // KEYCODE_HOME
    println!("Done!");

    Ok(())
}
