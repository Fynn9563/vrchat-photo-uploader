#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{
    CustomMenuItem, GlobalShortcutManager, Manager, SystemTray, SystemTrayEvent, SystemTrayMenu,
    SystemTrayMenuItem,
};

mod commands;
mod config;
mod database;
mod errors;
mod image_processor;
mod metadata_editor;
mod security;
mod single_instance;
mod uploader;

use commands::*;

// Progress state type
type ProgressState = Arc<Mutex<HashMap<String, UploadProgress>>>;

fn main() {
    // Initialize logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    log::info!("Starting VRChat Photo Uploader");

    // Check for single instance BEFORE starting Tauri
    if single_instance::check_single_instance().is_err() {
        log::info!("Application is already running. Exiting this instance.");
        std::process::exit(0);
    }

    // Register cleanup handlers
    single_instance::register_cleanup_handler();

    // Migrate configuration if needed
    if let Err(e) = config::migrate_config() {
        log::error!("Failed to migrate configuration: {}", e);
    }

    let tray_menu = SystemTrayMenu::new()
        .add_item(CustomMenuItem::new("upload_files", "📁 Upload Files"))
        .add_item(CustomMenuItem::new("open_vrchat_folder", "📂 Open VRChat Folder"))
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new("show", "🖼️ Show Window"))
        .add_item(CustomMenuItem::new("settings", "⚙️ Settings"))
        .add_item(CustomMenuItem::new("metadata_editor", "📝 Metadata Editor"))
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new("about", "ℹ️ About"))
        .add_item(CustomMenuItem::new("quit", "❌ Quit"));

    let system_tray = SystemTray::new()
        .with_menu(tray_menu)
        .with_tooltip("VRChat Photo Uploader");

    tauri::Builder::default()
        .system_tray(system_tray)
        .on_system_tray_event(|app, event| match event {
            SystemTrayEvent::LeftClick {
                position: _,
                size: _,
                ..
            } => {
                let window = app.get_window("main").unwrap();
                if window.is_visible().unwrap_or(false) {
                    if let Err(e) = window.hide() {
                        log::error!("Failed to hide window: {}", e);
                    }
                } else {
                    if let Err(e) = window.show() {
                        log::error!("Failed to show window: {}", e);
                    }
                    if let Err(e) = window.set_focus() {
                        log::error!("Failed to focus window: {}", e);
                    }
                }
            }
            SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
                "upload_files" => {
                    if let Some(window) = app.get_window("main") {
                        if let Err(e) = window.emit("upload-files-request", {}) {
                            log::error!("Failed to emit file upload event: {}", e);
                        }
                        if let Err(e) = window.show() {
                            log::error!("Failed to show window: {}", e);
                        }
                        if let Err(e) = window.set_focus() {
                            log::error!("Failed to focus window: {}", e);
                        }
                    }
                }
                "open_vrchat_folder" => {
                    if let Some(window) = app.get_window("main") {
                        if let Err(e) = window.emit("open-vrchat-folder-request", {}) {
                            log::error!("Failed to emit open VRChat folder event: {}", e);
                        }
                        // Window will be shown by frontend only if folder selection dialog is needed
                    }
                }
                "show" => {
                    let window = app.get_window("main").unwrap();
                    if let Err(e) = window.show() {
                        log::error!("Failed to show window: {}", e);
                    }
                    if let Err(e) = window.set_focus() {
                        log::error!("Failed to focus window: {}", e);
                    }
                }
                "settings" => {
                    if let Some(window) = app.get_window("main") {
                        if let Err(e) = window.emit("show-settings", {}) {
                            log::error!("Failed to emit settings event: {}", e);
                        }
                        if let Err(e) = window.show() {
                            log::error!("Failed to show window: {}", e);
                        }
                        if let Err(e) = window.set_focus() {
                            log::error!("Failed to focus window: {}", e);
                        }
                    }
                }
                "about" => {
                    if let Some(window) = app.get_window("main") {
                        if let Err(e) = window.emit("show-about", {}) {
                            log::error!("Failed to emit about event: {}", e);
                        }
                        if let Err(e) = window.show() {
                            log::error!("Failed to show window: {}", e);
                        }
                        if let Err(e) = window.set_focus() {
                            log::error!("Failed to focus window: {}", e);
                        }
                    }
                }
                "metadata_editor" => {
                    if let Some(window) = app.get_window("main") {
                        if let Err(e) = window.emit("show-metadata-editor", {}) {
                            log::error!("Failed to emit metadata editor event: {}", e);
                        }
                        if let Err(e) = window.show() {
                            log::error!("Failed to show window: {}", e);
                        }
                        if let Err(e) = window.set_focus() {
                            log::error!("Failed to focus window: {}", e);
                        }
                    }
                }
                "quit" => {
                    log::info!("Application quit requested from tray");
                    single_instance::cleanup_lock_file();
                    app.exit(0);
                }
                _ => {}
            },
            SystemTrayEvent::DoubleClick {
                position: _,
                size: _,
                ..
            } => {
                let window = app.get_window("main").unwrap();
                if let Err(e) = window.show() {
                    log::error!("Failed to show window: {}", e);
                }
                if let Err(e) = window.set_focus() {
                    log::error!("Failed to focus window: {}", e);
                }
            }
            _ => {}
        })
        .manage(ProgressState::new(Mutex::new(HashMap::new())))
        .invoke_handler(tauri::generate_handler![
            get_webhooks,
            add_webhook,
            delete_webhook,
            upload_images,
            get_upload_progress,
            retry_failed_upload,
            retry_failed_group,
            get_image_metadata,
            update_image_metadata,
            get_app_config,
            save_app_config,
            compress_image,
            cleanup_old_data,
            get_file_hash,
            cancel_upload_session,
            get_image_info,
            should_compress_image,
            cleanup_temp_files,
            shell_open,
            debug_extract_metadata
        ])
        .setup(|app| {
            log::info!("Setting up application...");
            // Start the signal checker for single instance
            single_instance::start_signal_checker(app.handle());

            // Block setup until database is initialized
            tauri::async_runtime::block_on(async {
                match database::init_database().await {
                    Ok(()) => {
                        log::info!("Database initialized successfully");
                    }
                    Err(e) => {
                        log::error!("Failed to initialize database: {}", e);
                        // Could still continue with app startup
                    }
                }
            });

            // Set up global shortcuts for screenshot upload
            let mut shortcuts = app.global_shortcut_manager();

            // Get the app handle for emitting events
            let shortcut_app_handle = app.handle();

            // Register global shortcuts
            if let Err(e) = shortcuts.register("CommandOrControl+Shift+U", move || {
                log::info!("Global shortcut triggered: Upload files");

                // Emit event to frontend to trigger file dialog
                if let Some(window) = shortcut_app_handle.get_window("main") {
                    if let Err(e) = window.emit("global-shortcut-upload", {}) {
                        log::error!("Failed to emit global shortcut event: {}", e);
                    } else {
                        log::info!("Global shortcut event emitted successfully");
                    }

                    // Also show and focus the window
                    if let Err(e) = window.show() {
                        log::error!("Failed to show window from global shortcut: {}", e);
                    }
                    if let Err(e) = window.set_focus() {
                        log::error!("Failed to focus window from global shortcut: {}", e);
                    }
                }
            }) {
                log::warn!("Failed to register global shortcut Ctrl+Shift+U: {}", e);
            }

            // Set window title with version
            if let Some(window) = app.get_window("main") {
                let version = app.package_info().version.to_string();
                let title = format!("VRChat Photo Uploader v{}", version);
                if let Err(e) = window.set_title(&title) {
                    log::warn!("Failed to set window title: {}", e);
                }
            }

            // Schedule auto-cleanup task - but wait for database to be ready
            let cleanup_app_handle = app.handle();
            tauri::async_runtime::spawn(async move {
                // Wait a bit for database to initialize
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;

                let mut interval =
                    tokio::time::interval(std::time::Duration::from_secs(24 * 60 * 60)); // Daily

                loop {
                    interval.tick().await;

                    // Check if database is initialized before cleanup
                    if database::DB_POOL.get().is_some() {
                        if let Err(e) = config::auto_cleanup().await {
                            log::error!("Auto-cleanup failed: {}", e);
                        } else {
                            log::info!("Auto-cleanup completed successfully");
                        }

                        // Emit cleanup completion event
                        if let Some(window) = cleanup_app_handle.get_window("main") {
                            if let Err(e) = window.emit("auto-cleanup-completed", {}) {
                                log::error!("Failed to emit cleanup event: {}", e);
                            }
                        }
                    } else {
                        log::warn!("Skipping auto-cleanup - database not initialized");
                    }
                }
            });

            // Initialize security cleanup on startup
            tauri::async_runtime::spawn(async {
                if let Err(e) = security::FileSystemGuard::cleanup_temp_files() {
                    log::warn!("Failed to cleanup temp files: {}", e);
                }
            });

            log::info!("Application setup completed successfully");
            Ok(())
        })
        .on_window_event(|event| {
            match event.event() {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    // Hide to tray instead of closing
                    event.window().hide().unwrap();
                    api.prevent_close();
                }
                _ => {}
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| match event {
            tauri::RunEvent::ExitRequested { .. } => {
                single_instance::cleanup_lock_file();
            }
            _ => {}
        });
}
