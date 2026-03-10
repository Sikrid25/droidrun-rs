//! UI State Provider — fetch, filter, and format the accessibility tree.
//!
//! Shows the full pipeline: raw a11y tree → filter → format → UIState.
//!
//! ```bash
//! cargo run -p droidrun-core --example state_provider
//! ```

use droidrun_core::driver::android::AndroidDriver;
use droidrun_core::driver::DeviceDriver;
use droidrun_core::ui::filter::ConciseFilter;
use droidrun_core::ui::formatter::IndexedFormatter;
use droidrun_core::ui::provider::AndroidStateProvider;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    // ── Connect ──────────────────────────────────────────────────
    let mut driver = AndroidDriver::new(None, true);
    driver.connect().await?;

    // Go to home screen for consistent state
    driver.press_key(3).await?;
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // ── Create State Provider ────────────────────────────────────
    // ConciseFilter: removes off-screen and tiny elements
    // IndexedFormatter: assigns sequential indices to elements
    // use_normalized: false = use absolute pixel coordinates
    let provider = AndroidStateProvider::new(ConciseFilter, IndexedFormatter, false);

    // ── Fetch State ──────────────────────────────────────────────
    // This calls Portal, applies filter + formatter, returns UIState
    let state = provider.get_state(&driver).await?;

    // ── Screen Info ──────────────────────────────────────────────
    println!("Screen: {}x{}", state.screen.width, state.screen.height);
    println!("Elements: {}", state.elements.len());
    println!("Focused: '{}'", state.focused_text);

    // ── Phone State ──────────────────────────────────────────────
    println!("\nPhone state:");
    println!("  App: {} ({})", state.phone_state.current_app, state.phone_state.package_name);
    println!("  Editable: {}", state.phone_state.is_editable);

    // ── Formatted Text Output ────────────────────────────────────
    // This is the human-readable representation used by LLMs
    println!("\n{}", state.formatted_text);

    // ── Element Access by Index ──────────────────────────────────
    println!("\n--- Element Details ---");
    let indices = state.all_indices();
    for &idx in indices.iter().take(5) {
        if let Some(elem) = state.get_element(idx) {
            // bounds is a comma-separated string: "left,top,right,bottom"
            println!(
                "  [{}] {} '{}' bounds=({})",
                idx, elem.class_name, elem.text, elem.bounds,
            );
        }
    }

    // ── Get Element Coordinates ──────────────────────────────────
    // Useful for tapping at the center of an element
    if let Some(first_idx) = indices.first() {
        match state.get_element_coords(*first_idx) {
            Ok((x, y)) => println!("\nElement {first_idx} center: ({x}, {y})"),
            Err(e) => println!("\nElement {first_idx} coords error: {e}"),
        }
    }

    // ── Get Clear Point ──────────────────────────────────────────
    // Finds a point inside the element that doesn't overlap children
    if let Some(first_idx) = indices.first() {
        match state.get_clear_point(*first_idx) {
            Ok((x, y)) => println!("Element {first_idx} clear tap point: ({x}, {y})"),
            Err(e) => println!("Element {first_idx} clear point error: {e}"),
        }
    }

    Ok(())
}
