//! DroidRun Portal APK communication and lifecycle management.

pub mod a11y;
pub mod client;
pub mod keyboard;
pub mod manager;

/// Portal APK package name.
pub const PORTAL_PACKAGE: &str = "com.droidrun.portal";

/// Full accessibility service component name.
pub const A11Y_SERVICE: &str =
    "com.droidrun.portal/com.droidrun.portal.service.DroidrunAccessibilityService";

/// Default HTTP server port on device.
pub const PORTAL_HTTP_PORT: u16 = 8080;

/// DroidRun keyboard IME component.
pub const KEYBOARD_IME: &str = "com.droidrun.portal/.input.DroidrunKeyboardIME";

/// GitHub repo for Portal releases.
pub const PORTAL_REPO: &str = "droidrun/droidrun-portal";

/// GitHub API hosts (with fallback).
pub const GITHUB_API_HOSTS: &[&str] = &["https://api.github.com", "https://ungh.cc"];

/// Version map URL for SDK ↔ Portal compatibility.
pub const VERSION_MAP_URL: &str =
    "https://raw.githubusercontent.com/droidrun/gists/refs/heads/main/version_map_android.json";
