use tauri::Manager;
use uuid::Uuid;

use crate::commands::UploadProgress;
use crate::errors::{AppError, AppResult, ProgressState};
use crate::{database, security, uploader};

/// Central manager for upload sessions to ensure unified behavior
pub struct SessionManager;

#[derive(Debug, Clone)]
pub struct SessionOptions {
    pub webhook_id: i64,
    pub file_paths: Vec<String>,
    pub group_by_metadata: bool,
    pub max_images_per_message: u8,
    pub is_forum_channel: bool,
    pub include_player_names: bool,
    pub grouping_time_window: u32,
    pub group_by_world: bool,
    pub upload_quality: Option<u8>,
    pub compression_format: Option<String>,
    pub single_thread_mode: bool,
    pub merge_no_metadata: bool,
}

impl SessionManager {
    /// Starts a new upload session, handling all validation and initialization
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

        if options.webhook_id <= 0 {
            return Err(AppError::UploadFailed {
                reason: "Invalid webhook ID".to_string(),
            });
        }

        // 2. Forum specific constraints
        let effective_max_images =
            if options.is_forum_channel && options.max_images_per_message > 10 {
                log::warn!(
                    "Forum channel detected with max_images > 10. Reducing to 10 for reliability."
                );
                10
            } else {
                options.max_images_per_message
            };

        // 3. File path validation
        for file_path in &options.file_paths {
            security::InputValidator::validate_image_file(file_path)?;
        }

        // 4. Fetch Webhook
        let webhook = match database::get_webhook_by_id(options.webhook_id).await {
            Ok(w) => w,
            Err(AppError::Database(sqlx::Error::RowNotFound)) => {
                return Err(AppError::UploadFailed {
                    reason: "Selected webhook not found".to_string(),
                });
            }
            Err(e) => return Err(e),
        };

        // 5. Initialize Progress State
        {
            let mut progress = progress_state
                .lock()
                .map_err(|_| AppError::Internal("Failed to lock progress state".to_string()))?;
            progress.insert(
                session_id.clone(),
                UploadProgress {
                    total_images: options.file_paths.len(),
                    completed: 0,
                    current_image: None,
                    current_progress: 0.0,
                    failed_uploads: Vec::new(),
                    successful_uploads: Vec::new(),
                    session_status: "active".to_string(),
                    estimated_time_remaining: None,
                },
            );
        }

        // 6. Database Records
        database::create_upload_session(
            session_id.clone(),
            options.webhook_id,
            options.file_paths.len() as i32,
        )
        .await?;
        database::update_webhook_usage(options.webhook_id).await?;

        // 7. Spawn Upload Task
        let handle_clone = app_handle.clone();
        let session_id_clone = session_id.clone();
        let progress_state_clone = progress_state.inner().clone();

        // Load config for defaults if quality/format are missing
        let config = crate::config::load_config().ok();
        let quality = options
            .upload_quality
            .or(config.as_ref().map(|c| c.upload_quality))
            .unwrap_or(85);
        let format = options
            .compression_format
            .or(config.as_ref().map(|c| c.compression_format.clone()))
            .unwrap_or_else(|| "webp".to_string());

        tokio::spawn(async move {
            uploader::process_upload_queue(
                webhook,
                options.file_paths,
                options.group_by_metadata,
                effective_max_images,
                options.is_forum_channel,
                options.include_player_names,
                options.grouping_time_window,
                options.group_by_world,
                Some(quality),
                Some(format),
                options.single_thread_mode,
                options.merge_no_metadata,
                progress_state_clone,
                session_id_clone,
                handle_clone,
            )
            .await;
        });

        Ok(session_id)
    }
}
