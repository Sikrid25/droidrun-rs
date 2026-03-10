use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use droidrun_core::driver::android::AndroidDriver;
use droidrun_core::driver::DeviceDriver;
use droidrun_core::portal::manager::PortalManager;
use droidrun_core::ui::filter::ConciseFilter;
use droidrun_core::ui::formatter::IndexedFormatter;
use droidrun_core::ui::provider::AndroidStateProvider;

#[derive(Parser)]
#[command(name = "droidrun", version, about = "Android device automation tool")]
struct Cli {
    /// Device serial number (auto-detect if not specified)
    #[arg(short, long, global = true)]
    serial: Option<String>,

    /// Use TCP mode for Portal communication (faster)
    #[arg(long, global = true, default_value_t = true)]
    tcp: bool,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Setup Portal on device
    Setup,

    /// Check device connection and Portal status
    Doctor,

    /// Take a screenshot
    Screenshot {
        /// Output file path
        #[arg(default_value = "screenshot.png")]
        output: PathBuf,
    },

    /// Tap at coordinates
    Tap { x: i32, y: i32 },

    /// Swipe from one point to another
    Swipe {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        /// Duration in milliseconds
        #[arg(short, long, default_value_t = 300)]
        duration: u32,
    },

    /// Type text into focused field
    Type {
        text: String,
        /// Clear existing text first
        #[arg(long)]
        clear: bool,
    },

    /// Send a key event
    Key {
        /// Android keycode (e.g., 4=Back, 3=Home, 66=Enter)
        keycode: i32,
    },

    /// Get UI state
    State {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// List installed apps
    Apps {
        /// Include system apps
        #[arg(long)]
        system: bool,
    },

    /// Start an app
    Open {
        /// Package name
        package: String,
        /// Activity name (auto-resolve if not specified)
        #[arg(short, long)]
        activity: Option<String>,
    },

    /// Run a shell command on device
    Shell {
        /// Command to run
        command: Vec<String>,
    },

    /// List connected devices
    Devices,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Setup logging
    let filter = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    match cli.command {
        Commands::Devices => {
            let server = droidrun_adb::AdbServer::default();
            let devices = server.devices().await?;
            if devices.is_empty() {
                println!("No devices connected.");
            } else {
                println!("{:<30} {}", "SERIAL", "STATE");
                for d in &devices {
                    println!("{:<30} {}", d.serial, d.state);
                }
            }
        }

        Commands::Setup => {
            let server = droidrun_adb::AdbServer::default();
            let device = server.resolve_device(cli.serial.as_deref()).await?;
            let manager = PortalManager::new(device);
            manager.setup("0.5.1", cli.verbose).await?;
            println!("Portal setup complete!");
        }

        Commands::Doctor => {
            let mut driver = AndroidDriver::new(cli.serial.as_deref(), cli.tcp);
            match driver.connect().await {
                Ok(()) => {
                    println!("✓ Device connected");
                    let portal = driver.portal_client()?;
                    match portal.ping().await {
                        Ok(result) => {
                            let method = result
                                .get("method")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                            println!("✓ Portal reachable via {method}");

                            let version = portal.get_version().await.unwrap_or("unknown".into());
                            println!("  Portal version: {version}");
                        }
                        Err(e) => println!("✗ Portal error: {e}"),
                    }
                    let date = driver.get_date().await?;
                    println!("  Device time: {date}");
                }
                Err(e) => {
                    println!("✗ Connection failed: {e}");
                    println!("\nTroubleshooting:");
                    println!("  1. Is ADB server running? (adb start-server)");
                    println!("  2. Is device connected? (adb devices)");
                    println!("  3. Is USB debugging enabled?");
                }
            }
        }

        Commands::Screenshot { output } => {
            let mut driver = AndroidDriver::new(cli.serial.as_deref(), cli.tcp);
            driver.connect().await?;
            let png = driver.screenshot(true).await?;
            tokio::fs::write(&output, &png).await?;
            println!("Screenshot saved to {}", output.display());
        }

        Commands::Tap { x, y } => {
            let mut driver = AndroidDriver::new(cli.serial.as_deref(), cli.tcp);
            driver.connect().await?;
            driver.tap(x, y).await?;
            println!("Tapped at ({x}, {y})");
        }

        Commands::Swipe {
            x1,
            y1,
            x2,
            y2,
            duration,
        } => {
            let mut driver = AndroidDriver::new(cli.serial.as_deref(), cli.tcp);
            driver.connect().await?;
            driver.swipe(x1, y1, x2, y2, duration).await?;
            println!("Swiped from ({x1},{y1}) to ({x2},{y2})");
        }

        Commands::Type { text, clear } => {
            let mut driver = AndroidDriver::new(cli.serial.as_deref(), cli.tcp);
            driver.connect().await?;
            driver.input_text(&text, clear).await?;
            println!("Typed: {text}");
        }

        Commands::Key { keycode } => {
            let mut driver = AndroidDriver::new(cli.serial.as_deref(), cli.tcp);
            driver.connect().await?;
            driver.press_key(keycode).await?;
            println!("Key event: {keycode}");
        }

        Commands::State { json } => {
            let mut driver = AndroidDriver::new(cli.serial.as_deref(), cli.tcp);
            driver.connect().await?;

            if json {
                let tree = driver.get_ui_tree().await?;
                println!("{}", serde_json::to_string_pretty(&tree)?);
            } else {
                let provider =
                    AndroidStateProvider::new(ConciseFilter, IndexedFormatter, false);
                let state = provider.get_state(&driver).await?;
                println!("{}", state.formatted_text);
            }
        }

        Commands::Apps { system } => {
            let mut driver = AndroidDriver::new(cli.serial.as_deref(), cli.tcp);
            driver.connect().await?;
            let apps = driver.get_apps(system).await?;
            println!("{:<50} {}", "PACKAGE", "LABEL");
            for app in &apps {
                println!("{:<50} {}", app.package, app.label);
            }
            println!("\nTotal: {} apps", apps.len());
        }

        Commands::Open { package, activity } => {
            let mut driver = AndroidDriver::new(cli.serial.as_deref(), cli.tcp);
            driver.connect().await?;
            let result = driver
                .start_app(&package, activity.as_deref())
                .await?;
            println!("{result}");
        }

        Commands::Shell { command } => {
            let cmd = command.join(" ");
            let server = droidrun_adb::AdbServer::default();
            let device = server.resolve_device(cli.serial.as_deref()).await?;
            let output = device.shell(&cmd).await?;
            print!("{output}");
        }
    }

    Ok(())
}
