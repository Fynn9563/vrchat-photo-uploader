use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::commands::Webhook;
use crate::errors::{safe_emit_event, ProgressState};
use crate::{database, image_processor, security};

use super::discord_client::DiscordClient;
use super::image_groups::create_discord_payload;
use super::progress_tracker::{
    update_progress_current, update_progress_failure, update_progress_success,
};
use super::upload_queue::upload_image_chunk_with_thread_id;

/// Retry a failed upload
pub async fn retry_single_upload(
    webhook: Webhook,
    file_path: String,
    progress_state: ProgressState,
    session_id: String,
    app_handle: tauri::AppHandle,
) {
    let client = DiscordClient::new();

    if let Err(e) = security::InputValidator::validate_image_file(&file_path) {
        update_progress_failure(
            &progress_state,
            &session_id,
            file_path,
            e.to_string(),
            false,
        );
        return;
    }

    update_progress_current(&progress_state, &session_id, file_path.clone());

    let metadata = image_processor::extract_metadata(&file_path)
        .await
        .ok()
        .flatten();
    let timestamp = image_processor::get_timestamp_from_filename(&file_path);
    let all_players = metadata
        .as_ref()
        .map(|m| m.players.clone())
        .unwrap_or_default();
    let all_worlds = metadata
        .as_ref()
        .and_then(|m| m.world.clone())
        .map(|w| vec![w])
        .unwrap_or_default();

    let (text_fields, player_messages) = create_discord_payload(
        &all_worlds,
        &all_players,
        timestamp,
        true,
        0,
        webhook.is_forum,
        None,
        true,
    );

    let dummy_progress_state = Arc::new(Mutex::new(HashMap::new()));

    match upload_image_chunk_with_thread_id(
        &client,
        &webhook,
        vec![file_path.clone()],
        text_fields,
        None,
        &dummy_progress_state,
        "retry",
        &app_handle,
    )
    .await
    {
        Ok(response_data) => {
            // Send player messages if any (for single file retries)
            if !player_messages.is_empty() {
                // For forum channels, extract thread_id first
                let thread_id = if webhook.is_forum {
                    super::discord_client::extract_thread_id(&response_data)
                } else {
                    None
                };

                for (i, player_msg) in player_messages.iter().enumerate() {
                    if let Err(e) = client
                        .send_text_message(&webhook.url, player_msg, thread_id.as_deref())
                        .await
                    {
                        log::warn!("Failed to send player message {}: {}", i + 1, e);
                    }
                }
            }

            let file_name = Path::new(&file_path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let file_hash = image_processor::get_file_hash(&file_path).await.ok();
            let file_size = security::FileSystemGuard::get_file_size(&file_path).ok();
            let webhook_id = webhook.id;
            let file_path_for_db = file_path.clone();

            tokio::spawn(async move {
                let _ = database::record_upload(
                    file_path_for_db,
                    file_name,
                    file_hash,
                    file_size,
                    webhook_id,
                    "success",
                    None,
                )
                .await;
            });

            update_progress_success(&progress_state, &session_id, file_path.clone());
            log::info!("Successfully retried upload for {}", file_path);
        }
        Err(e) => {
            let is_retryable = e.is_retryable();
            let file_name = Path::new(&file_path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let error_message = format!("Retry failed: {}", e);
            let webhook_id = webhook.id;
            let file_path_for_db = file_path.clone();

            tokio::spawn(async move {
                let _ = database::record_upload(
                    file_path_for_db,
                    file_name,
                    None,
                    None,
                    webhook_id,
                    "failed",
                    Some(error_message),
                )
                .await;
            });

            update_progress_failure(
                &progress_state,
                &session_id,
                file_path.clone(),
                e.to_string(),
                is_retryable,
            );
            log::error!("Retry failed for {}: {}", file_path, e);
        }
    }

    safe_emit_event(&app_handle, "upload-progress", &session_id);
}
