use std::fs;
use std::path::PathBuf;
use sysinfo::{Pid, System};
use tauri::{AppHandle, Manager};

#[derive(Debug)]
pub struct SingleInstanceError;

/// Check if another instance of the application is already running
pub fn check_single_instance() -> Result<(), SingleInstanceError> {
    let lock_file = get_lock_file_path();

    // Check if lock file exists
    if lock_file.exists() {
        if let Ok(pid_str) = fs::read_to_string(&lock_file) {
            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                // Check if process with this PID is still running
                let mut system = System::new_all();
                system.refresh_processes();

                if let Some(process) = system.process(Pid::from(pid as usize)) {
                    // Check if it's actually our application
                    let process_name = process.name().to_lowercase();
                    if process_name.contains("vrchat")
                        || process_name.contains("photo")
                        || process_name.contains("uploader")
                    {
                        log::info!(
                            "Found existing instance (PID: {}), signaling it to show",
                            pid
                        );
                        signal_existing_instance();
                        return Err(SingleInstanceError); // Exit this instance
                    }
                }
            }
        }

        // If we get here, the lock file exists but process is dead - clean it up
        let _ = fs::remove_file(&lock_file);
    }

    // Create lock file with current process ID
    let current_pid = std::process::id();
    if let Err(e) = fs::write(&lock_file, current_pid.to_string()) {
        log::warn!("Failed to create lock file: {}", e);
    } else {
        log::info!("Created lock file with PID: {}", current_pid);
    }

    Ok(())
}

/// Get the path to the lock file
fn get_lock_file_path() -> PathBuf {
    let temp_dir = std::env::temp_dir();
    temp_dir.join("vrchat_photo_uploader.lock")
}

/// Signal an existing instance to show its window
fn signal_existing_instance() {
    // Create a signal file that the existing instance can detect
    let signal_file = std::env::temp_dir().join("vrchat_photo_uploader_show.signal");
    if let Err(e) = fs::write(&signal_file, "show") {
        log::warn!("Failed to create signal file: {}", e);
    } else {
        log::info!("Created signal file to show existing instance");
    }
}

/// Clean up the lock file when the application exits
pub fn cleanup_lock_file() {
    let lock_file = get_lock_file_path();
    if lock_file.exists() {
        if let Err(e) = fs::remove_file(&lock_file) {
            log::warn!("Failed to remove lock file: {}", e);
        } else {
            log::info!("Cleaned up lock file");
        }
    }
}

/// Register cleanup handler for Ctrl+C and other signals
pub fn register_cleanup_handler() {
    ctrlc::set_handler(move || {
        log::info!("Received Ctrl+C, cleaning up...");
        cleanup_lock_file();
        std::process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");
}

/// Start the signal checker that watches for show requests from other instances
pub fn start_signal_checker(app_handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
        let signal_file = std::env::temp_dir().join("vrchat_photo_uploader_show.signal");

        loop {
            interval.tick().await;

            if signal_file.exists() {
                log::info!("Received show signal from another instance");

                // Remove signal file
                let _ = fs::remove_file(&signal_file);

                // Show and focus window
                if let Some(window) = app_handle.get_window("main") {
                    if let Err(e) = window.show() {
                        log::error!("Failed to show window: {}", e);
                    }
                    if let Err(e) = window.unminimize() {
                        log::error!("Failed to unminimize window: {}", e);
                    }
                    if let Err(e) = window.set_focus() {
                        log::error!("Failed to focus window: {}", e);
                    }

                    // On Windows, use a trick to bring window to front
                    #[cfg(target_os = "windows")]
                    {
                        let _ = window.set_always_on_top(true);
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        let _ = window.set_always_on_top(false);
                    }

                    log::info!("Window shown and focused");
                }
            }
        }
    });
}
