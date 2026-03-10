/// Integration tests for Portal communication.
///
/// These tests require a running emulator with droidrun-portal installed.
/// Run with: `cargo test -p droidrun-core --test integration -- --nocapture`
use droidrun_core::driver::android::AndroidDriver;
use droidrun_core::driver::DeviceDriver;
use droidrun_core::ui::filter::ConciseFilter;
use droidrun_core::ui::formatter::IndexedFormatter;
use droidrun_core::ui::provider::AndroidStateProvider;

fn skip_if_no_device() -> bool {
    std::env::var("SKIP_DEVICE_TESTS").is_ok()
}

async fn get_driver() -> Option<AndroidDriver> {
    if skip_if_no_device() {
        return None;
    }
    let mut driver = AndroidDriver::new(None, true);
    match driver.connect().await {
        Ok(()) => Some(driver),
        Err(e) => {
            println!("Skipping: cannot connect ({e})");
            None
        }
    }
}

#[tokio::test]
async fn test_portal_ping() {
    let Some(driver) = get_driver().await else {
        return;
    };
    let portal = driver.portal_client().unwrap();
    let result = portal.ping().await.unwrap();
    let status = result.get("status").and_then(|v| v.as_str()).unwrap_or("");
    assert_eq!(status, "success");
    println!("Portal ping: {}", serde_json::to_string_pretty(&result).unwrap());
}

#[tokio::test]
async fn test_portal_version() {
    let Some(driver) = get_driver().await else {
        return;
    };
    let portal = driver.portal_client().unwrap();
    let version = portal.get_version().await.unwrap();
    println!("Portal version: {version}");
    assert_ne!(version, "unknown");
}

#[tokio::test]
async fn test_portal_screenshot() {
    let Some(driver) = get_driver().await else {
        return;
    };
    let png = driver.screenshot(true).await.unwrap();
    assert!(!png.is_empty());
    println!("Portal screenshot: {} bytes", png.len());
}

#[tokio::test]
async fn test_portal_get_state() {
    let Some(driver) = get_driver().await else {
        return;
    };
    let state = driver.get_ui_tree().await.unwrap();
    assert!(state.get("a11y_tree").is_some() || state.get("phone_state").is_some());
    println!(
        "State keys: {:?}",
        state.as_object().map(|o| o.keys().collect::<Vec<_>>())
    );
}

#[tokio::test]
async fn test_portal_get_apps() {
    let Some(driver) = get_driver().await else {
        return;
    };
    let apps = driver.get_apps(false).await.unwrap();
    println!("Found {} apps", apps.len());
    for app in apps.iter().take(5) {
        println!("  {} ({})", app.label, app.package);
    }
}

#[tokio::test]
async fn test_state_provider() {
    let Some(driver) = get_driver().await else {
        return;
    };
    let provider = AndroidStateProvider::new(ConciseFilter, IndexedFormatter, false);
    let state = provider.get_state(&driver).await.unwrap();

    println!("Elements: {}", state.elements.len());
    println!("Screen: {}x{}", state.screen.width, state.screen.height);
    println!("Focused: '{}'", state.focused_text);
    println!("\n{}", state.formatted_text);

    // Verify we got elements
    assert!(state.screen.width > 0);
    assert!(state.screen.height > 0);
}

#[tokio::test]
async fn test_tap_and_state() {
    let Some(driver) = get_driver().await else {
        return;
    };
    // Go to home screen first
    driver.press_key(3).await.unwrap(); // KEYCODE_HOME
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let provider = AndroidStateProvider::new(ConciseFilter, IndexedFormatter, false);
    let state = provider.get_state(&driver).await.unwrap();

    println!("Home screen elements: {}", state.elements.len());
    assert!(!state.elements.is_empty());
}

#[tokio::test]
async fn test_input_text() {
    let Some(driver) = get_driver().await else {
        return;
    };
    // Note: input_text may return false if no text field is focused
    // (e.g. on home screen). We just verify the call doesn't error.
    let result = driver.input_text("hello from rust!", false).await.unwrap();
    println!("Input text result: {result}");
}

#[tokio::test]
async fn test_swipe() {
    let Some(driver) = get_driver().await else {
        return;
    };
    // Swipe up (like scrolling)
    driver.swipe(540, 1800, 540, 600, 300).await.unwrap();
    println!("Swipe succeeded");
}

#[tokio::test]
async fn test_key_event() {
    let Some(driver) = get_driver().await else {
        return;
    };
    // Press Home button
    driver.press_key(3).await.unwrap();
    println!("Home key pressed");
}
