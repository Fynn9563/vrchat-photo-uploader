use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{State, Manager};

use crate::{config, database, image_processor, metadata_editor, uploader};
use crate::security::InputValidator;
use crate::errors::AppError;

#[derive(Debug, Serialize, Deserialize)]
pub struct Webhook {
    pub id: i64,
    pub name: String,
    pub url: String,
    pub is_forum: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UploadRequest {
    pub webhook_id: i64,
    pub file_paths: Vec<String>,
    pub group_by_metadata: bool,
    pub max_images_per_message: u8,
    pub is_forum_channel: bool,
    pub include_player_names: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UploadProgress {
    pub total_images: usize,
    pub completed: usize,
    pub current_image: Option<String>,
    pub current_progress: f32,
    pub failed_uploads: Vec<FailedUpload>,
    pub successful_uploads: Vec<String>,
    pub session_status: String, // "active", "completed", "failed", "cancelled"
    pub estimated_time_remaining: Option<u64>, // seconds
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FailedUpload {
    pub file_path: String,
    pub error: String,
    pub retry_count: u32,
    pub is_retryable: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImageMetadata {
    pub author: Option<AuthorInfo>,
    pub world: Option<WorldInfo>,
    pub players: Vec<PlayerInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthorInfo {
    pub display_name: String,
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorldInfo {
    pub name: String,
    pub id: String,
    pub instance_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlayerInfo {
    pub display_name: String,
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub last_webhook_id: Option<i64>,
    pub group_by_metadata: bool,
    pub max_images_per_message: u8,
    pub enable_global_shortcuts: bool,
    pub auto_compress_threshold: u64, // MB
    pub upload_quality: u8,
}

// Progress state type (defined in main.rs, re-exported here for commands)
pub type ProgressState = Arc<Mutex<HashMap<String, UploadProgress>>>;

#[tauri::command]
pub async fn get_webhooks() -> Result<Vec<Webhook>, String> {
    database::get_all_webhooks()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn retry_failed_group(
    _session_id: String,
    file_paths: Vec<String>,
    webhook_id: i64,
    progress_state: State<'_, ProgressState>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    // Validate inputs
    if file_paths.is_empty() {
        return Err("No files provided for group retry".to_string());
    }
    
    if webhook_id <= 0 {
        return Err("Invalid webhook ID".to_string());
    }

    // Validate all file paths
    for file_path in &file_paths {
        InputValidator::validate_image_file(file_path)?;
    }
    
    let webhook = database::get_webhook_by_id(webhook_id)
        .await
        .map_err(|e| e.to_string())?;

    // Create new upload session for the retry
    let new_session_id = uuid::Uuid::new_v4().to_string();
    
    // Initialize progress for group retry
    {
        let mut progress = progress_state.lock().unwrap();
        progress.insert(new_session_id.clone(), UploadProgress {
            total_images: file_paths.len(),
            completed: 0,
            current_image: None,
            current_progress: 0.0,
            failed_uploads: Vec::new(),
            successful_uploads: Vec::new(),
            session_status: "active".to_string(),
            estimated_time_remaining: None,
        });
    }

    // Create upload session in database
    database::create_upload_session(
        new_session_id.clone(), 
        webhook_id, 
        file_paths.len() as i32
    ).await.map_err(|e| e.to_string())?;

    // Update webhook usage
    database::update_webhook_usage(webhook_id)
        .await
        .map_err(|e| e.to_string())?;

    // Start group retry process (with grouping enabled since it was a group failure)
    let progress_state_clone = progress_state.inner().clone();
    let new_session_id_clone = new_session_id.clone();
    let app_handle_clone = app_handle.clone();
    
    tokio::spawn(async move {
        uploader::process_upload_queue(
            webhook,
            file_paths,
            true, // group_by_metadata = true for group retry
            10,    // max_images_per_message = 10 (safe for forum channels)
            false, // is_forum_channel = false (default, will be set by UI if needed)
            true,  // include_player_names = true (default for retries)
            progress_state_clone,
            new_session_id_clone,
            app_handle_clone,
        ).await;
    });

    log::info!("Started group retry with session: {}", new_session_id);
    Ok(new_session_id)
}

#[tauri::command]
pub async fn resolve_temp_file_paths(temp_filenames: Vec<String>) -> Result<Vec<String>, String> {
    let mut resolved_paths = Vec::new();
    
    for filename in temp_filenames {
        // Check if it's already a full path
        if std::path::Path::new(&filename).is_absolute() {
            resolved_paths.push(filename);
        } else {
            // Resolve as temp file
            let temp_dir = std::env::temp_dir();
            let full_path = temp_dir.join(&filename);
            
            if full_path.exists() {
                resolved_paths.push(full_path.to_string_lossy().to_string());
            } else {
                return Err(format!("Temp file not found: {}", filename));
            }
        }
    }
    
    Ok(resolved_paths)
}

#[tauri::command]
pub async fn add_webhook(name: String, url: String) -> Result<(), String> {
    // Validate inputs
    InputValidator::validate_webhook_name(&name)?;
    InputValidator::validate_webhook_url(&url)?;
    
    // Sanitize name
    let sanitized_name = InputValidator::sanitize_filename(&name);
    
    database::insert_webhook(sanitized_name, url, false) // Always set is_forum to false
        .await
        .map(|_| ()) // Convert i64 to ()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_webhook(id: i64) -> Result<(), String> {
    if id <= 0 {
        return Err("Invalid webhook ID".to_string());
    }
    
    database::delete_webhook(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn upload_images(
    request: UploadRequest,
    progress_state: State<'_, ProgressState>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    // Validate request
    if request.file_paths.is_empty() {
        return Err("No files provided".to_string());
    }
    
    // Validate webhook_id before proceeding
    if request.webhook_id <= 0 {
        return Err("Please select a webhook before starting upload".to_string());
    }
    
    // SPECIAL VALIDATION FOR FORUM CHANNELS:
    let effective_max_images = if request.is_forum_channel {
        if request.max_images_per_message > 10 {
            log::warn!("Forum channel detected with max_images > 10. Reducing to 10 to prevent thread_id issues.");
            10
        } else {
            request.max_images_per_message
        }
    } else {
        request.max_images_per_message
    };
    
    InputValidator::validate_upload_settings(effective_max_images, request.group_by_metadata)?;
    
    // Validate all file paths
    for file_path in &request.file_paths {
        InputValidator::validate_image_file(file_path)?;
    }
    
    let session_id = uuid::Uuid::new_v4().to_string();
    
    // Initialize progress
    {
        let mut progress = progress_state.lock().unwrap();
        progress.insert(session_id.clone(), UploadProgress {
            total_images: request.file_paths.len(),
            completed: 0,
            current_image: None,
            current_progress: 0.0,
            failed_uploads: Vec::new(),
            successful_uploads: Vec::new(),
            session_status: "active".to_string(),
            estimated_time_remaining: None,
        });
    }

    // Get webhook details - this is where the error was occurring
    let webhook = match database::get_webhook_by_id(request.webhook_id).await {
        Ok(webhook) => webhook,
        Err(AppError::Database(sqlx::Error::RowNotFound)) => {
            return Err("Selected webhook not found. Please select a valid webhook.".to_string());
        }
        Err(e) => {
            log::error!("Database error when fetching webhook {}: {}", request.webhook_id, e);
            return Err(format!("Failed to fetch webhook: {}", e));
        }
    };

    // Create upload session in database
    database::create_upload_session(
        session_id.clone(), 
        request.webhook_id, 
        request.file_paths.len() as i32
    ).await.map_err(|e| e.to_string())?;

    // Update webhook usage
    database::update_webhook_usage(request.webhook_id)
        .await
        .map_err(|e| e.to_string())?;

    // Log forum channel usage for debugging
    if request.is_forum_channel {
        log::info!("Forum channel upload started: {} files, max {} per message", 
            request.file_paths.len(), effective_max_images);
    }

    // Start upload process
    let progress_state_clone = progress_state.inner().clone();
    let session_id_clone = session_id.clone();
    let app_handle_clone = app_handle.clone();
    
    tokio::spawn(async move {
        uploader::process_upload_queue(
            webhook,
            request.file_paths,
            request.group_by_metadata,
            effective_max_images,
            request.is_forum_channel,
            request.include_player_names,
            progress_state_clone,
            session_id_clone,
            app_handle_clone,
            ).await;
    });

    Ok(session_id)
}

#[tauri::command]
pub async fn get_upload_progress(
    session_id: String,
    progress_state: State<'_, ProgressState>,
) -> Result<Option<UploadProgress>, String> {
    let progress = progress_state.lock().unwrap();
    Ok(progress.get(&session_id).cloned())
}

#[tauri::command]
pub async fn retry_failed_upload(
    session_id: String,
    file_path: String,
    webhook_id: i64,
    progress_state: State<'_, ProgressState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    // Validate inputs
    InputValidator::validate_image_file(&file_path)?;
    
    if webhook_id <= 0 {
        return Err("Invalid webhook ID".to_string());
    }

    let webhook = database::get_webhook_by_id(webhook_id)
        .await
        .map_err(|e| e.to_string())?;

    let progress_state_clone = progress_state.inner().clone();
    let session_id_clone = session_id.clone();
    let app_handle_clone = app_handle.clone();
    
    tokio::spawn(async move {
        uploader::retry_single_upload(
            webhook,
            file_path,
            progress_state_clone,
            session_id_clone,
            app_handle_clone,
        ).await;
    });

    Ok(())
}

#[tauri::command]
pub async fn get_image_metadata(file_path: String) -> Result<Option<ImageMetadata>, String> {
    InputValidator::validate_image_file(&file_path)?;
    
    image_processor::extract_metadata(&file_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_image_metadata(
    file_path: String,
    metadata: ImageMetadata,
) -> Result<String, String> {
    InputValidator::validate_image_file(&file_path)?;
    
    metadata_editor::embed_metadata(&file_path, metadata)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn compress_image(file_path: String, quality: u8) -> Result<String, String> {
    InputValidator::validate_image_file(&file_path)?;
    
    if quality == 0 || quality > 100 {
        return Err("Quality must be between 1 and 100".to_string());
    }
    
    image_processor::compress_image(&file_path, quality)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_image_info(file_path: String) -> Result<(u32, u32, u64), String> {
    InputValidator::validate_image_file(&file_path)?;
    
    image_processor::get_image_info(&file_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn should_compress_image(file_path: String) -> Result<bool, String> {
    InputValidator::validate_image_file(&file_path)?;
    
    image_processor::should_compress_image(&file_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_app_config() -> Result<AppConfig, String> {
    config::load_config().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_app_config(config: AppConfig) -> Result<(), String> {
    // Validate config
    if let Some(max_images) = Some(config.max_images_per_message) {
        InputValidator::validate_upload_settings(max_images, config.group_by_metadata)?;
    }
    
    if config.upload_quality == 0 || config.upload_quality > 100 {
        return Err("Upload quality must be between 1 and 100".to_string());
    }
    
    config::save_config(config).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cleanup_old_data(days: i32) -> Result<(u64, u64), String> {
    if days <= 0 {
        return Err("Days must be a positive number".to_string());
    }
    
    let sessions_cleaned = database::cleanup_old_upload_sessions(days)
        .await
        .map_err(|e| e.to_string())?;
    
    let history_cleaned = database::cleanup_old_upload_history(days)
        .await
        .map_err(|e| e.to_string())?;
    
    Ok((sessions_cleaned, history_cleaned))
}

#[tauri::command]
pub async fn get_file_hash(file_path: String) -> Result<String, String> {
    InputValidator::validate_image_file(&file_path)?;
    
    image_processor::get_file_hash(&file_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cleanup_temp_files(temp_filenames: Vec<String>) -> Result<(), String> {
    for filename in temp_filenames {
        let temp_dir = std::env::temp_dir();
        let full_path = temp_dir.join(&filename);
        
        if full_path.exists() {
            if let Err(e) = std::fs::remove_file(&full_path) {
                log::warn!("Failed to cleanup temp file {}: {}", filename, e);
            } else {
                log::debug!("Cleaned up temp file: {}", filename);
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn debug_extract_metadata(file_path: String) -> Result<String, String> {
    InputValidator::validate_image_file(&file_path)?;
    
    log::info!("DEBUG: Starting detailed metadata extraction for {}", file_path);
    
    match image_processor::extract_metadata(&file_path).await {
        Ok(Some(metadata)) => {
            let debug_info = format!(
                "SUCCESS: Metadata extracted successfully!\n\
                 Author: {:?}\n\
                 World: {:?}\n\
                 Players: {} found\n\
                 First player: {:?}",
                metadata.author,
                metadata.world,
                metadata.players.len(),
                metadata.players.first()
            );
            log::info!("{}", debug_info);
            Ok(debug_info)
        },
        Ok(None) => {
            let debug_info = "No metadata found in file".to_string();
            log::warn!("{}", debug_info);
            Ok(debug_info)
        },
        Err(e) => {
            let debug_info = format!("ERROR: Failed to extract metadata: {}", e);
            log::error!("{}", debug_info);
            Err(debug_info)
        }
    }
}

#[tauri::command]
pub async fn shell_open(path: String) -> Result<(), String> {
    use std::process::Command;
    
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    
    Ok(())
}

#[tauri::command]
pub async fn cancel_upload_session(
    session_id: String, 
    progress_state: State<'_, ProgressState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    log::info!("Attempting to cancel upload session: {}", session_id);
    
    let mut progress = progress_state.lock().unwrap();
    
    if let Some(session_progress) = progress.get_mut(&session_id) {
        // Only cancel if session is currently active
        if session_progress.session_status == "active" {
            session_progress.session_status = "cancelled".to_string();
            session_progress.estimated_time_remaining = Some(0);
            
            log::info!("Upload session {} marked as cancelled", session_id);
            
            // Emit events to notify frontend
            app_handle.emit_all("upload-cancelled", &session_id).ok();
            app_handle.emit_all("upload-progress", &session_id).ok();
            
            return Ok(());
        } else {
            log::warn!("Cannot cancel session {} - current status: {}", 
                session_id, session_progress.session_status);
            return Err(format!("Session is not active (status: {})", session_progress.session_status));
        }
    } else {
        log::warn!("Attempted to cancel non-existent session: {}", session_id);
        return Err("Session not found".to_string());
    }
}