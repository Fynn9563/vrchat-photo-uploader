use tauri::Manager;
use uuid::Uuid;

use crate::commands::UploadProgress;
use crate::errors::{AppError, AppResult, ProgressState};
use crate::uploader::progress_tracker::{
    emit_session_progress, is_session_cancelled, mark_session_completed,
};
use crate::{database, security, uploader};

/// Central manager for upload sessions to ensure unified behavior
pub struct SessionManager;

#[derive(Debug, Clone)]
pub struct SessionOptions {
    pub webhook_ids: Vec<i64>,
    pub file_paths: Vec<String>,
    pub group_by_metadata: bool,
    pub max_images_per_message: u8,
    pub include_player_names: bool,
    pub grouping_time_window: u32,
    pub group_by_world: bool,
    pub upload_quality: Option<u8>,
    pub compression_format: Option<String>,
    pub single_thread_mode: bool,
    pub merge_no_metadata: bool,
}

impl SessionManager {
    /// Starts a new upload session, handling all validation and initialization.
    /// Supports multiple webhooks — processes them sequentially within a single session.
    pub async fn start_session(
        app_handle: &tauri::AppHandle,
        options: SessionOptions,
    ) -> AppResult<String> {
        let session_id = Uuid::new_v4().to_string();
        let progress_state = app_handle.state::<ProgressState>();

        // 1. Initial Validation
        if options.file_paths.is_empty() {
            return Err(AppError::UploadFailed {
                reason: "No files provided".to_string(),
            });
        }

        if options.webhook_ids.is_empty() {
            return Err(AppError::UploadFailed {
                reason: "No webhooks specified".to_string(),
            });
        }

        for id in &options.webhook_ids {
            if *id <= 0 {
                return Err(AppError::UploadFailed {
                    reason: "Invalid webhook ID".to_string(),
                });
            }
        }

        // 2. File path validation
        for file_path in &options.file_paths {
            security::InputValidator::validate_image_file(file_path)?;
        }

        // 3. Fetch ALL webhooks (fail fast if any not found)
        let mut webhooks = Vec::new();
        for id in &options.webhook_ids {
            let webhook = match database::get_webhook_by_id(*id).await {
                Ok(w) => w,
                Err(AppError::Database(sqlx::Error::RowNotFound)) => {
                    return Err(AppError::UploadFailed {
                        reason: format!("Webhook with ID {id} not found"),
                    });
                }
                Err(e) => return Err(e),
            };
            webhooks.push(webhook);
        }

        let num_webhooks = webhooks.len();
        let total_images = options.file_paths.len() * num_webhooks;

        // 4. Initialize Progress State
        {
            let mut progress = progress_state
                .lock()
                .map_err(|_| AppError::Internal("Failed to lock progress state".to_string()))?;
            progress.insert(
                session_id.clone(),
                UploadProgress {
                    total_images,
                    completed: 0,
                    current_image: None,
                    current_progress: 0.0,
                    failed_uploads: Vec::new(),
                    successful_uploads: Vec::new(),
                    session_status: "active".to_string(),
                    estimated_time_remaining: None,
                    current_webhook_index: 0,
                    total_webhooks: num_webhooks,
                    current_webhook_name: webhooks[0].name.clone(),
                },
            );
        }

        // 5. Database Records (use first webhook ID for the session record)
        database::create_upload_session(
            session_id.clone(),
            options.webhook_ids[0],
            total_images as i32,
        )
        .await?;
        for id in &options.webhook_ids {
            database::update_webhook_usage(*id).await?;
        }

        // 6. Load config for defaults if quality/format are missing
        let config = crate::config::load_config().ok();
        let quality = options
            .upload_quality
            .or(config.as_ref().map(|c| c.upload_quality))
            .unwrap_or(85);
        let format = options
            .compression_format
            .or(config.as_ref().map(|c| c.compression_format.clone()))
            .unwrap_or_else(|| "webp".to_string());

        // 7. Spawn Coordinator Task
        let handle_clone = app_handle.clone();
        let session_id_clone = session_id.clone();
        let progress_state_clone = progress_state.inner().clone();

        tokio::spawn(async move {
            for (idx, webhook) in webhooks.into_iter().enumerate() {
                // Check cancellation before each webhook
                if is_session_cancelled(&progress_state_clone, &session_id_clone) {
                    log::info!(
                        "Session {} cancelled before webhook {}/{}",
                        session_id_clone,
                        idx + 1,
                        num_webhooks
                    );
                    return;
                }

                // Update current_webhook_index, name, reset status and clear per-webhook state
                {
                    if let Ok(mut progress) = progress_state_clone.lock() {
                        if let Some(p) = progress.get_mut(&session_id_clone) {
                            p.current_webhook_index = idx;
                            p.current_webhook_name = webhook.name.clone();
                            p.session_status = "active".to_string();
                            // Clear successful/failed uploads so frontend resets item states
                            p.successful_uploads.clear();
                            p.failed_uploads.clear();
                        }
                    }
                }

                let effective_max_images =
                    if webhook.is_forum && options.max_images_per_message > 10 {
                        log::warn!(
                            "Forum channel detected for webhook '{}', reducing max_images to 10.",
                            webhook.name
                        );
                        10
                    } else {
                        options.max_images_per_message
                    };

                log::info!(
                    "Session {} starting webhook {}/{} ('{}')",
                    session_id_clone,
                    idx + 1,
                    num_webhooks,
                    webhook.name
                );

                uploader::process_upload_queue(
                    webhook,
                    options.file_paths.clone(),
                    options.group_by_metadata,
                    effective_max_images,
                    options.include_player_names,
                    options.grouping_time_window,
                    options.group_by_world,
                    Some(quality),
                    Some(format.clone()),
                    options.single_thread_mode,
                    options.merge_no_metadata,
                    progress_state_clone.clone(),
                    session_id_clone.clone(),
                    handle_clone.clone(),
                    false, // coordinator handles completion
                )
                .await;

                // Check post-upload status: if failed or cancelled, stop iterating
                let should_stop = {
                    if let Ok(progress) = progress_state_clone.lock() {
                        if let Some(p) = progress.get(&session_id_clone) {
                            p.session_status == "failed" || p.session_status == "cancelled"
                        } else {
                            true // session missing, stop
                        }
                    } else {
                        true // lock failed, stop
                    }
                };

                if should_stop {
                    log::info!(
                        "Session {} stopped after webhook {}/{} (status changed)",
                        session_id_clone,
                        idx + 1,
                        num_webhooks
                    );
                    return;
                }

                // process_upload_queue leaves status as "active" (mark_completed=false)
                // Coordinator continues to next webhook
            }

            // All webhooks done — mark truly completed
            mark_session_completed(&progress_state_clone, &session_id_clone);
            emit_session_progress(&handle_clone, &progress_state_clone, &session_id_clone);
        });

        Ok(session_id)
    }
}
