use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

use crate::errors::{AppError, AppResult, ProgressState};
use crate::{config, database, uploader};

pub struct BackgroundWatcher {
    watcher: Option<RecommendedWatcher>,
    path: Option<String>,
    pending_files: Arc<Mutex<Vec<String>>>,
    last_activity: Arc<Mutex<Option<Instant>>>,
    batch_active: Arc<std::sync::atomic::AtomicBool>,
    start_time: std::time::SystemTime,
}

impl BackgroundWatcher {
    pub fn new() -> Self {
        Self {
            watcher: None,
            path: None,
            pending_files: Arc::new(Mutex::new(Vec::new())),
            last_activity: Arc::new(Mutex::new(None)),
            batch_active: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            start_time: std::time::SystemTime::now(),
        }
    }

    pub fn start(&mut self, app_handle: AppHandle, path_str: String) -> Result<(), String> {
        if self.watcher.is_some() {
            self.stop();
        }

        let (tx, rx) = channel();

        // Create watcher
        let mut watcher = RecommendedWatcher::new(tx, Config::default())
            .map_err(|e| format!("Failed to create watcher: {}", e))?;

        let root_path = Path::new(&path_str);
        if !root_path.exists() {
            return Err(format!("Directory does not exist: {}", path_str));
        }

        // Watch root directory
        watcher
            .watch(root_path, RecursiveMode::Recursive)
            .map_err(|e| format!("Failed to watch root directory: {}", e))?;

        // Explicitly watch current month folder if it exists (extra robust for NAS)
        let now = chrono::Local::now();
        let month_folder = now.format("%Y-%m").to_string();
        let month_path = root_path.join(&month_folder);
        if month_path.exists() {
            log::info!("Explicitly watching month folder: {}", month_path.display());
            let _ = watcher.watch(&month_path, RecursiveMode::NonRecursive);
        }

        self.watcher = Some(watcher);
        self.path = Some(path_str.clone());

        log::info!("Background watcher started on: {}", path_str);

        let handle_clone = app_handle.clone();
        let pending_files = self.pending_files.clone();
        let last_activity = self.last_activity.clone();
        let batch_active = self.batch_active.clone();
        let start_time = self.start_time;

        // Spawn a thread to handle events
        thread::spawn(move || {
            for res in rx {
                match res {
                    Ok(event) => {
                        if is_new_image_event(&event) {
                            let handle = handle_clone.clone();
                            let pending = pending_files.clone();
                            let activity = last_activity.clone();
                            let active = batch_active.clone();
                            let start_time = start_time;

                            // Trigger / Reset Batch Logic
                            tauri::async_runtime::spawn(async move {
                                for path_buf in event.paths {
                                    let path_str = path_buf.to_string_lossy().to_string();
                                    if is_image_file(&path_str) {
                                        // Filter by time: Only process files created/modified after app start
                                        if let Ok(meta) = std::fs::metadata(&path_buf) {
                                            let file_time = meta
                                                .created()
                                                .or_else(|_| meta.modified())
                                                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                                            if file_time < start_time {
                                                log::debug!("Ignoring old file: {}", path_str);
                                                continue;
                                            }
                                        }

                                        log::info!("Detected file for auto-upload: {}", path_str);

                                        // Add to pending
                                        if let Ok(mut q) = pending.lock() {
                                            if !q.contains(&path_str) {
                                                q.push(path_str);
                                            }
                                        }

                                        // Update activity
                                        if let Ok(mut t) = activity.lock() {
                                            *t = Some(Instant::now());
                                        }

                                        // Start monitor if not running
                                        if !active.load(std::sync::atomic::Ordering::SeqCst) {
                                            start_batch_monitor(
                                                handle.clone(),
                                                pending.clone(),
                                                activity.clone(),
                                                active.clone(),
                                                start_time,
                                            );
                                        }
                                    }
                                }
                            });
                        }
                    }
                    Err(e) => log::error!("Watch error: {:?}", e),
                }
            }
        });

        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(path) = &self.path {
            log::info!("Stopping background watcher on: {}", path);
        }
        self.watcher = None;
        self.path = None;
        // Clear pending on stop
        if let Ok(mut q) = self.pending_files.lock() {
            q.clear();
        }
        if let Ok(mut t) = self.last_activity.lock() {
            *t = None;
        }
        self.batch_active
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

fn start_batch_monitor(
    app_handle: AppHandle,
    pending_files: Arc<Mutex<Vec<String>>>,
    last_activity: Arc<Mutex<Option<Instant>>>,
    batch_active: Arc<std::sync::atomic::AtomicBool>,
    start_time: std::time::SystemTime,
) {
    batch_active.store(true, std::sync::atomic::Ordering::SeqCst);

    tauri::async_runtime::spawn(async move {
        log::info!("Starting background batch monitor...");
        let mut last_scan_check = Instant::now();

        loop {
            // Check if we should process
            let config = match config::load_config() {
                Ok(c) => c,
                Err(_) => {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
            };

            if !config.enable_auto_upload {
                log::info!("Auto-upload disabled, stopping batch monitor.");
                batch_active.store(false, std::sync::atomic::Ordering::SeqCst);
                break;
            }

            // Periodic subfolder check (every 60s) to handle NAS issues and month rollovers
            if last_scan_check.elapsed() > Duration::from_secs(60) {
                if let Some(root_str) = &config.vrchat_path {
                    let root_path = Path::new(root_str);
                    let now = chrono::Local::now();
                    let month_folder = now.format("%Y-%m").to_string();
                    let month_path = root_path.join(&month_folder);

                    if month_path.exists() {
                        log::debug!("Periodic scan: month folder {} exists", month_folder);
                        // We can't easily re-add to the watcher here without access to it,
                        // but we can manually scan for files that might have been missed
                        if let Ok(entries) = std::fs::read_dir(&month_path) {
                            for entry in entries.flatten() {
                                let path = entry.path();
                                if path.is_file() {
                                    let path_str = path.to_string_lossy().to_string();
                                    if is_image_file(&path_str) {
                                        // Check if already in database to avoid duplicates
                                        let is_processed = database::is_file_processed(&path_str)
                                            .await
                                            .unwrap_or(false);

                                        // Filter by time: Only process files created/modified after app start
                                        let file_time = entry
                                            .metadata()
                                            .ok()
                                            .and_then(|m| {
                                                m.created().or_else(|_| m.modified()).ok()
                                            })
                                            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

                                        if !is_processed && file_time >= start_time {
                                            if let Ok(mut q) = pending_files.lock() {
                                                if !q.contains(&path_str) {
                                                    log::info!(
                                                        "Found missed file via scan: {}",
                                                        path_str
                                                    );
                                                    q.push(path_str);
                                                    if let Ok(mut t) = last_activity.lock() {
                                                        if t.is_none() {
                                                            *t = Some(Instant::now());
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                last_scan_check = Instant::now();
            }

            let delay = Duration::from_secs(config.auto_upload_delay_seconds as u64);
            let mut should_upload = false;

            {
                if let Ok(activity) = last_activity.lock() {
                    if let Some(last_t) = *activity {
                        if last_t.elapsed() >= delay {
                            should_upload = true;
                        }
                    }
                }
            }

            if should_upload {
                let files_to_upload = {
                    if let Ok(mut q) = pending_files.lock() {
                        let result = q.clone();
                        q.clear();
                        result
                    } else {
                        Vec::new()
                    }
                };

                if !files_to_upload.is_empty() {
                    log::info!(
                        "Batch stable. Processing {} files for auto-upload.",
                        files_to_upload.len()
                    );
                    match process_auto_upload_batch(files_to_upload, &app_handle).await {
                        Ok(session_id) => {
                            // Sequential: Wait for this session to finish before monitor exits
                            // This ensures we don't spawn multiple concurrent auto-upload sessions
                            log::info!("Monitoring auto-upload session {}...", session_id);
                            loop {
                                tokio::time::sleep(Duration::from_secs(2)).await;
                                let is_active = {
                                    let state = app_handle.state::<ProgressState>();
                                    let progress = state.inner().lock();
                                    match progress {
                                        Ok(p) => p
                                            .get(&session_id)
                                            .map(|s| s.session_status == "active")
                                            .unwrap_or(false),
                                        Err(_) => false,
                                    }
                                };
                                if !is_active {
                                    log::info!("Auto-upload session {} completed.", session_id);
                                    break;
                                }

                                // Check if auto-upload was disabled mid-upload
                                let config = config::load_config().ok();
                                if config.map(|c| !c.enable_auto_upload).unwrap_or(false) {
                                    log::warn!("Auto-upload disabled during active session.");
                                    break;
                                }
                            }
                        }
                        Err(e) => log::error!("Batch auto-upload failed: {}", e),
                    }
                }

                // Reset activity and exit loop since batch is processed
                if let Ok(mut t) = last_activity.lock() {
                    *t = None;
                }
                batch_active.store(false, std::sync::atomic::Ordering::SeqCst);
                break;
            }

            // Sleep a bit before checking again
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
    });
}

fn is_new_image_event(event: &Event) -> bool {
    // We want to catch:
    // 1. New files created (Create)
    // 2. Files finished being written/moved (Modify)
    // 3. Files renamed/moved (Modify(Name))
    match event.kind {
        EventKind::Create(_) => true,
        EventKind::Modify(_) => true,
        _ => false,
    }
}

fn is_image_file(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".webp")
}

async fn process_auto_upload_batch(
    file_paths: Vec<String>,
    app_handle: &AppHandle,
) -> AppResult<String> {
    let config = config::load_config().map_err(|e| AppError::Config(e.to_string()))?;

    if !config.enable_auto_upload || file_paths.is_empty() {
        return Err(AppError::UploadFailed {
            reason: "Auto-upload disabled or no files".to_string(),
        });
    }

    let webhook_id = match config.auto_upload_webhook_id {
        Some(id) => id,
        None => {
            return Err(AppError::UploadFailed {
                reason: "No auto-upload webhook configured".to_string(),
            })
        }
    };

    // --- Resilience Filtering ---
    let mut valid_paths = Vec::new();
    let mut missing_count = 0;
    let mut duplicate_count = 0;

    for path in file_paths {
        let p = Path::new(&path);
        if !p.exists() {
            missing_count += 1;
            continue;
        }

        match database::is_file_processed(&path).await {
            Ok(true) => {
                duplicate_count += 1;
                continue;
            }
            Ok(false) => {
                valid_paths.push(path);
            }
            Err(e) => {
                log::warn!(
                    "Failed to check if file is processed: {}. Proceeding anyway.",
                    e
                );
                valid_paths.push(path);
            }
        }
    }

    if missing_count > 0 || duplicate_count > 0 {
        log::info!(
            "Background batch filtering: {} valid, {} missing, {} duplicates removed.",
            valid_paths.len(),
            missing_count,
            duplicate_count
        );
    }

    if valid_paths.is_empty() {
        log::info!(
            "All files in background batch were filtered out (missing or already processed)."
        );
        return Ok("no_files_remaining".to_string());
    }
    // ----------------------------

    // Get the webhook to check its forum status
    let webhook = database::get_webhook_by_id(webhook_id).await?;
    let is_forum = webhook.is_forum;

    let options = uploader::SessionOptions {
        webhook_id,
        file_paths: valid_paths,
        group_by_metadata: config.auto_upload_group_by_metadata,
        max_images_per_message: config.auto_upload_batch_size,
        is_forum_channel: is_forum || config.auto_upload_forum_channel,
        include_player_names: config.auto_upload_include_players,
        grouping_time_window: config.auto_upload_time_window,
        group_by_world: config.auto_upload_group_by_world,
        upload_quality: Some(config.upload_quality),
        compression_format: Some(config.compression_format.clone()),
        single_thread_mode: config.auto_upload_single_thread,
        merge_no_metadata: config.auto_upload_merge_no_metadata,
    };

    log::info!(
        "🚀 Auto-upload session starting for '{}' (Forum: {})",
        webhook.name,
        is_forum
    );

    uploader::SessionManager::start_session(app_handle, options).await
}
