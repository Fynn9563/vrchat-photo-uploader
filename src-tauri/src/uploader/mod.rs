// Main uploader module - orchestrates all upload functionality
//
// This module is responsible for coordinating VRChat photo uploads to Discord
// with intelligent grouping, metadata extraction, and error handling.

pub mod discord_client;
pub mod image_groups;
pub mod progress_tracker;
pub mod retry;
pub mod upload_queue;

pub use retry::retry_single_upload;
pub use upload_queue::process_upload_queue;

// Legacy compatibility exports
use crate::commands::Webhook;
use crate::errors::ProgressState;

/// Legacy function wrapper for process_upload_queue
/// 
/// This maintains compatibility with existing code while delegating
/// to the new modular implementation.
pub async fn process_upload_queue_legacy(
    webhook: Webhook,
    file_paths: Vec<String>,
    group_by_metadata: bool,
    max_images_per_message: u8,
    is_forum_channel: bool,
    include_player_names: bool,
    progress_state: ProgressState,
    session_id: String,
    app_handle: tauri::AppHandle,
) {
    upload_queue::process_upload_queue(
        webhook,
        file_paths,
        group_by_metadata,
        max_images_per_message,
        is_forum_channel,
        include_player_names,
        progress_state,
        session_id,
        app_handle,
    ).await
}

/// Legacy function wrapper for retry_single_upload
/// 
/// Maintains backward compatibility while using the new retry module.
pub async fn retry_single_upload_legacy(
    webhook: Webhook,
    file_path: String,
    progress_state: ProgressState,
    session_id: String,
    app_handle: tauri::AppHandle,
) {
    retry::retry_single_upload(
        webhook,
        file_path,
        progress_state,
        session_id,
        app_handle,
    ).await
}


