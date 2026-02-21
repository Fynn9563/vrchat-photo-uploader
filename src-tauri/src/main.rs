#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(non_snake_case)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

pub mod background_watcher;
mod commands;
mod config;
mod database;
mod errors;
mod image_processor;
mod metadata_editor;
mod security;
mod single_instance;

mod uploader;

#[cfg(test)]
pub mod test_helpers;

use commands::*;

/// Progress state type
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
        log::error!("Failed to migrate configuration: {e}");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_os::init())
        .manage(ProgressState::new(Mutex::new(HashMap::new())))
        .manage(Mutex::new(background_watcher::BackgroundWatcher::new()))
        .invoke_handler(tauri::generate_handler![
            get_webhooks,
            add_webhook,
            update_webhook,
            delete_webhook,
            upload_images,
            get_upload_progress,
            retry_failed_upload,
            retry_failed_group,
            get_image_metadata,
            get_image_metadata_with_source,
            update_image_metadata,
            get_app_config,
            save_app_config,
            compress_image,
            cleanup_old_data,
            get_file_hash,
            cancel_upload_session,
            get_image_info,
            get_image_info_batch,
            generate_thumbnail,
            generate_thumbnails_batch,
            should_compress_image,
            cleanup_temp_files,
            shell_open,
            debug_extract_metadata,
            check_for_updates,
            get_user_webhook_overrides,
            add_user_webhook_override,
            delete_user_webhook_override
        ])
        .setup(|app| {
            log::info!("Setting up application...");

            // Register updater plugin
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;

            // Register global shortcut plugin
            {
                use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState};
                let shortcut_app_handle = app.handle().clone();
                app.handle().plugin(
                    tauri_plugin_global_shortcut::Builder::new()
                        .with_handler(move |_app, shortcut, event| {
                            if event.state == ShortcutState::Pressed
                                && shortcut.matches(Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyU)
                            {
                                log::info!("Global shortcut triggered: Upload files");
                                if let Some(window) =
                                    shortcut_app_handle.get_webview_window("main")
                                {
                                    if let Err(e) = window.emit("global-shortcut-upload", ()) {
                                        log::error!(
                                            "Failed to emit global shortcut event: {e}"
                                        );
                                    } else {
                                        log::info!("Global shortcut event emitted successfully");
                                    }
                                    if let Err(e) = window.show() {
                                        log::error!(
                                            "Failed to show window from global shortcut: {e}"
                                        );
                                    }
                                    if let Err(e) = window.set_focus() {
                                        log::error!(
                                            "Failed to focus window from global shortcut: {e}"
                                        );
                                    }
                                }
                            }
                        })
                        .build(),
                )?;

                // Register the shortcut after plugin is initialized
                use tauri_plugin_global_shortcut::GlobalShortcutExt;
                app.global_shortcut().on_shortcut(
                    tauri_plugin_global_shortcut::Shortcut::new(
                        Some(Modifiers::CONTROL | Modifiers::SHIFT),
                        Code::KeyU,
                    ),
                    |_, _, _| {
                        // Handled by the handler above
                    },
                )?;
            }

            // Build system tray menu
            let upload_files =
                MenuItem::with_id(app, "upload_files", "📁 Upload Files", true, None::<&str>)?;
            let open_vrchat = MenuItem::with_id(
                app,
                "open_vrchat_folder",
                "📂 Open VRChat Folder",
                true,
                None::<&str>,
            )?;
            let sep1 = PredefinedMenuItem::separator(app)?;
            let show =
                MenuItem::with_id(app, "show", "🖼️ Show Window", true, None::<&str>)?;
            let settings =
                MenuItem::with_id(app, "settings", "⚙️ Settings", true, None::<&str>)?;
            let metadata_editor = MenuItem::with_id(
                app,
                "metadata_editor",
                "📝 Metadata Editor",
                true,
                None::<&str>,
            )?;
            let sep2 = PredefinedMenuItem::separator(app)?;
            let about = MenuItem::with_id(app, "about", "ℹ️ About", true, None::<&str>)?;
            let check_updates = MenuItem::with_id(
                app,
                "check_updates",
                "🔄 Check for Updates",
                true,
                None::<&str>,
            )?;
            let sep3 = PredefinedMenuItem::separator(app)?;
            let quit = MenuItem::with_id(app, "quit", "❌ Quit", true, None::<&str>)?;

            let menu = Menu::with_items(
                app,
                &[
                    &upload_files,
                    &open_vrchat,
                    &sep1,
                    &show,
                    &settings,
                    &metadata_editor,
                    &sep2,
                    &about,
                    &check_updates,
                    &sep3,
                    &quit,
                ],
            )?;

            // Build tray icon
            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("VRChat Photo Uploader")
                .icon(app.default_window_icon().unwrap().clone())
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
                        "upload_files" => {
                            if let Some(window) = app.get_webview_window("main") {
                                if let Err(e) = window.emit("upload-files-request", ()) {
                                    log::error!("Failed to emit file upload event: {e}");
                                }
                                if let Err(e) = window.show() {
                                    log::error!("Failed to show window: {e}");
                                }
                                if let Err(e) = window.set_focus() {
                                    log::error!("Failed to focus window: {e}");
                                }
                            }
                        }
                        "open_vrchat_folder" => {
                            if let Some(window) = app.get_webview_window("main") {
                                if let Err(e) = window.emit("open-vrchat-folder-request", ()) {
                                    log::error!(
                                        "Failed to emit open VRChat folder event: {e}"
                                    );
                                }
                            }
                        }
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                if let Err(e) = window.show() {
                                    log::error!("Failed to show window: {e}");
                                }
                                if let Err(e) = window.set_focus() {
                                    log::error!("Failed to focus window: {e}");
                                }
                            }
                        }
                        "settings" => {
                            if let Some(window) = app.get_webview_window("main") {
                                if let Err(e) = window.emit("show-settings", ()) {
                                    log::error!("Failed to emit settings event: {e}");
                                }
                                if let Err(e) = window.show() {
                                    log::error!("Failed to show window: {e}");
                                }
                                if let Err(e) = window.set_focus() {
                                    log::error!("Failed to focus window: {e}");
                                }
                            }
                        }
                        "about" => {
                            if let Some(window) = app.get_webview_window("main") {
                                if let Err(e) = window.emit("show-about", ()) {
                                    log::error!("Failed to emit about event: {e}");
                                }
                                if let Err(e) = window.show() {
                                    log::error!("Failed to show window: {e}");
                                }
                                if let Err(e) = window.set_focus() {
                                    log::error!("Failed to focus window: {e}");
                                }
                            }
                        }
                        "metadata_editor" => {
                            if let Some(window) = app.get_webview_window("main") {
                                if let Err(e) = window.emit("show-metadata-editor", ()) {
                                    log::error!("Failed to emit metadata editor event: {e}");
                                }
                                if let Err(e) = window.show() {
                                    log::error!("Failed to show window: {e}");
                                }
                                if let Err(e) = window.set_focus() {
                                    log::error!("Failed to focus window: {e}");
                                }
                            }
                        }
                        "check_updates" => {
                            log::info!("Check for updates requested from tray");
                            let app_handle = app.clone();
                            tauri::async_runtime::spawn(async move {
                                if let Err(e) = commands::check_for_updates(app_handle).await {
                                    log::error!("Failed to check for updates: {e}");
                                }
                            });
                        }
                        "quit" => {
                            log::info!("Application quit requested from tray");
                            single_instance::cleanup_lock_file();
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    match event {
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } => {
                            let app = tray.app_handle();
                            if let Some(window) = app.get_webview_window("main") {
                                if window.is_visible().unwrap_or(false) {
                                    if let Err(e) = window.hide() {
                                        log::error!("Failed to hide window: {e}");
                                    }
                                } else {
                                    if let Err(e) = window.show() {
                                        log::error!("Failed to show window: {e}");
                                    }
                                    if let Err(e) = window.set_focus() {
                                        log::error!("Failed to focus window: {e}");
                                    }
                                }
                            }
                        }
                        TrayIconEvent::DoubleClick {
                            button: MouseButton::Left,
                            ..
                        } => {
                            let app = tray.app_handle();
                            if let Some(window) = app.get_webview_window("main") {
                                if let Err(e) = window.show() {
                                    log::error!("Failed to show window: {e}");
                                }
                                if let Err(e) = window.set_focus() {
                                    log::error!("Failed to focus window: {e}");
                                }
                            }
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            // Start the signal checker for single instance
            single_instance::start_signal_checker(app.handle().clone());

            // Block setup until database is initialized
            tauri::async_runtime::block_on(async {
                match database::init_database().await {
                    Ok(()) => {
                        log::info!("Database initialized successfully");
                    }
                    Err(e) => {
                        log::error!("Failed to initialize database: {e}");
                    }
                }
            });

            // Set window title with version
            if let Some(window) = app.get_webview_window("main") {
                let version = app.package_info().version.to_string();
                let title = format!("VRChat Photo Uploader v{version}");
                if let Err(e) = window.set_title(&title) {
                    log::warn!("Failed to set window title: {e}");
                }
            }

            // Schedule auto-cleanup task - but wait for database to be ready
            let cleanup_app_handle = app.handle().clone();
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
                            log::error!("Auto-cleanup failed: {e}");
                        } else {
                            log::info!("Auto-cleanup completed successfully");
                        }

                        // Emit cleanup completion event
                        if let Some(window) = cleanup_app_handle.get_webview_window("main") {
                            if let Err(e) = window.emit("auto-cleanup-completed", ()) {
                                log::error!("Failed to emit cleanup event: {e}");
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
                    log::warn!("Failed to cleanup temp files: {e}");
                }
            });

            // Initialize Background Watcher
            let watcher_app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(config) = config::load_config() {
                    if config.enable_auto_upload {
                        if let Some(path) = config.vrchat_path {
                            if let Ok(mut watcher) = watcher_app_handle
                                .state::<Mutex<background_watcher::BackgroundWatcher>>()
                                .lock()
                            {
                                if let Err(e) = watcher.start(watcher_app_handle.clone(), path) {
                                    log::error!("Failed to start background watcher: {e}");
                                }
                            }
                        }
                    }
                }
            });

            log::info!("Application setup completed successfully");
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Hide to tray instead of closing
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| if let tauri::RunEvent::ExitRequested { .. } = event {
            single_instance::cleanup_lock_file();
        });
}
