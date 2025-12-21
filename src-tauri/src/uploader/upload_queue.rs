use std::collections::HashMap;
use std::path::Path;
use tauri::Manager;
use tokio::time::{sleep, Duration, Instant};

use crate::commands::Webhook;
use crate::errors::{safe_emit_event, AppError, AppResult, ProgressState};
use crate::{database, image_processor, security};

use super::discord_client::{extract_thread_id, DiscordClient, UploadPayload};
use super::image_groups::{create_discord_payload, ImageGroup};
use super::progress_tracker::*;

/// Safe chunk size to stay under Discord's 8MB limit with overhead
const SAFE_CHUNK_SIZE_BYTES: u64 = 7 * 1024 * 1024;

/// Process the upload queue
#[allow(clippy::too_many_arguments)]
pub async fn process_upload_queue(
    webhook: Webhook,
    file_paths: Vec<String>,
    group_by_metadata: bool,
    max_images_per_message: u8,
    is_forum_channel: bool,
    include_player_names: bool,
    grouping_time_window: u32,
    group_by_world: bool,
    progress_state: ProgressState,
    session_id: String,
    app_handle: tauri::AppHandle,
) {
    let client = DiscordClient::new();

    // Initial cancellation check
    if is_session_cancelled(&progress_state, &session_id) {
        log::info!(
            "Session {} was cancelled before processing started",
            session_id
        );
        mark_session_cancelled(&progress_state, &session_id);
        return;
    }

    // Validate all files before starting
    let mut valid_files = Vec::new();
    for (i, file_path) in file_paths.iter().enumerate() {
        // Check cancellation every few files during validation
        if i % 5 == 0 && is_session_cancelled(&progress_state, &session_id) {
            log::info!(
                "Session {} cancelled during file validation at file {}",
                session_id,
                i
            );
            mark_session_cancelled(&progress_state, &session_id);
            return;
        }

        if let Err(e) = security::InputValidator::validate_image_file(file_path) {
            log::error!("File validation failed for {}: {}", file_path, e);
            update_progress_failure(
                &progress_state,
                &session_id,
                file_path.clone(),
                e.to_string(),
                false,
            );
        } else {
            valid_files.push(file_path.clone());
        }
    }

    if valid_files.is_empty() {
        log::warn!("No valid files to upload for session {}", session_id);
        mark_session_completed(&progress_state, &session_id);
        safe_emit_event(&app_handle, "upload-completed", &session_id);
        return;
    }

    // Check cancellation before grouping
    if is_session_cancelled(&progress_state, &session_id) {
        log::info!("Session {} cancelled before grouping images", session_id);
        mark_session_cancelled(&progress_state, &session_id);
        return;
    }

    // Show metadata loading phase for all files
    if let Some(first_file) = valid_files.first() {
        update_progress_current_with_phase(
            &progress_state,
            &session_id,
            first_file.clone(),
            "Loading metadata",
            0.0,
        );
        safe_emit_event(&app_handle, "upload-progress", &session_id);
    }

    // Emit loading metadata event for all files
    app_handle
        .emit_all(
            "upload-item-progress",
            serde_json::json!({
                "session_id": session_id,
                "phase": "loading_metadata",
                "file_paths": valid_files
            }),
        )
        .ok();

    // Group images if requested
    let groups = if group_by_metadata {
        super::image_groups::group_images_by_metadata(
            valid_files,
            grouping_time_window,
            group_by_world,
        ).await
    } else {
        super::image_groups::create_individual_groups_with_metadata(valid_files).await
    };

    // Emit grouping complete event
    app_handle
        .emit_all(
            "upload-item-progress",
            serde_json::json!({
                "session_id": session_id,
                "phase": "grouped",
                "total_groups": groups.len()
            }),
        )
        .ok();

    let start_time = Instant::now();
    let mut total_processed = 0;
    let total_groups = groups.len();

    log::info!(
        "Processing {} groups for session {}",
        total_groups,
        session_id
    );

    // Process each group
    for (group_index, group) in groups.into_iter().enumerate() {
        // Check cancellation before each group
        if is_session_cancelled(&progress_state, &session_id) {
            log::info!(
                "Session {} cancelled during group {} processing",
                session_id,
                group_index + 1
            );
            mark_session_cancelled(&progress_state, &session_id);
            return;
        }

        log::info!(
            "Processing group {} of {} (ID: {}, {} images)",
            group_index + 1,
            total_groups,
            group.group_id,
            group.images.len()
        );

        // Emit per-group progress
        app_handle
            .emit_all(
                "upload-item-progress",
                serde_json::json!({
                    "session_id": session_id,
                    "phase": "group_start",
                    "group_index": group_index,
                    "total_groups": total_groups,
                    "images_in_group": group.images.len(),
                    "file_paths": group.images
                }),
            )
            .ok();

        let group_success = process_image_group_with_failure_handling(
            &client,
            &webhook,
            group,
            max_images_per_message,
            is_forum_channel,
            include_player_names,
            &progress_state,
            &session_id,
            &app_handle,
            group_index == 0, // is_first_group
        )
        .await;

        if is_session_cancelled(&progress_state, &session_id) {
            log::info!(
                "Session {} cancelled after group {} processing",
                session_id,
                group_index + 1
            );
            mark_session_cancelled(&progress_state, &session_id);
            return;
        }

        if !group_success {
            log::error!(
                "Group {} failed - stopping remaining groups",
                group_index + 1
            );
            mark_session_failed(&progress_state, &session_id);
            safe_emit_event(&app_handle, "upload-failed", &session_id);
            return;
        }

        total_processed += 1;

        // Update estimated time remaining
        update_time_estimate(
            &progress_state,
            &session_id,
            start_time,
            total_processed,
            total_groups,
        );

        // Small delay between groups to be nice to Discord
        sleep(Duration::from_millis(500)).await;
    }

    if is_session_cancelled(&progress_state, &session_id) {
        log::info!("Session {} was cancelled before completion", session_id);
        mark_session_cancelled(&progress_state, &session_id);
        return;
    }

    // Mark session as completed
    mark_session_completed(&progress_state, &session_id);

    // Update database session status (non-blocking)
    let session_id_for_db = session_id.clone();
    tokio::spawn(async move {
        if let Ok(Some((_total, completed, successful, failed))) =
            database::get_upload_session_stats(&session_id_for_db).await
        {
            let _ = database::update_upload_session_progress(
                &session_id_for_db,
                completed,
                successful,
                failed,
            )
            .await;
        }
    });

    safe_emit_event(&app_handle, "upload-completed", &session_id);
}

/// Process image group with error handling
#[allow(clippy::too_many_arguments)]
async fn process_image_group_with_failure_handling(
    client: &DiscordClient,
    webhook: &Webhook,
    group: ImageGroup,
    max_images_per_message: u8,
    is_forum_channel: bool,
    include_player_names: bool,
    progress_state: &ProgressState,
    session_id: &str,
    app_handle: &tauri::AppHandle,
    is_first_group: bool,
) -> bool {
    log::info!(
        "🚀 Starting group upload (ID: {}, {} images)",
        group.group_id,
        group.images.len()
    );

    if is_session_cancelled(progress_state, session_id) {
        log::info!(
            "❌ Session {} cancelled before group {} upload",
            session_id,
            group.group_id
        );
        return false;
    }

    // For forum channels, be extra careful about chunk sizes
    let effective_max_images = if is_forum_channel && max_images_per_message > 10 {
        log::warn!(
            "⚠️ Forum channel detected with max_images > 10, reducing to 10 to prevent issues"
        );
        10
    } else {
        max_images_per_message
    };

    let chunks: Vec<Vec<String>> = group
        .images
        .chunks(effective_max_images as usize)
        .map(|chunk| chunk.to_vec())
        .collect();

    if is_forum_channel {
        log::info!(
            "📋 Forum channel upload: {} chunks of max {} images each",
            chunks.len(),
            effective_max_images
        );

        if chunks.len() > 1 {
            log::info!(
                "⚠️ Multiple chunks detected for forum channel - thread_id extraction is CRITICAL"
            );
        }
    }

    let mut first_message = true;
    let mut thread_id: Option<String> = None;

    // Process chunks and stop on first failure OR cancellation
    for (chunk_index, chunk) in chunks.iter().enumerate() {
        if is_session_cancelled(progress_state, session_id) {
            log::info!(
                "❌ Session {} cancelled during chunk {} of group {}",
                session_id,
                chunk_index + 1,
                group.group_id
            );
            return false;
        }

        log::info!(
            "📤 Uploading chunk {} of {} in group {} ({} images)",
            chunk_index + 1,
            chunks.len(),
            group.group_id,
            chunk.len()
        );

        let (text_fields, overflow_messages) = create_discord_payload(
            &group.all_worlds,
            &group.all_players,
            group.timestamp,
            first_message,
            chunk_index,
            is_forum_channel && is_first_group, // Only first group creates new thread
            thread_id.as_deref(),
            include_player_names,
        );

        // If this is the first message and we have overflow player messages,
        // we need to send text first, then overflow, then images
        let mut text_fields_for_images = text_fields.clone();
        if first_message && !overflow_messages.is_empty() {
            // Send the main text message first (this creates the forum thread if applicable)
            log::info!("📤 Sending text message first (has {} overflow messages)", overflow_messages.len());

            let main_content = text_fields.get("content").cloned().unwrap_or_default();

            // For forum channels, include thread_name in first message
            if is_forum_channel && is_first_group {
                let thread_name = text_fields.get("thread_name").cloned();

                // Send as text with thread_name to create the thread
                match client.send_forum_text_message(
                    &webhook.url,
                    &main_content,
                    thread_name.as_deref(),
                ).await {
                    Ok(response_data) => {
                        // Extract thread_id from response
                        if let Some(extracted_thread_id) = extract_thread_id(&response_data) {
                            thread_id = Some(extracted_thread_id.clone());
                            log::info!("✅ Forum thread created with thread_id: {}", extracted_thread_id);

                            // Send overflow messages to the thread
                            for (i, overflow_msg) in overflow_messages.iter().enumerate() {
                                if let Err(e) = client.send_text_message(
                                    &webhook.url,
                                    overflow_msg,
                                    Some(&extracted_thread_id),
                                ).await {
                                    log::warn!("Failed to send overflow message {}: {}", i + 1, e);
                                }
                            }
                        } else {
                            log::error!("Failed to extract thread_id from forum response");
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to send forum text message: {}", e);
                        // Mark files as failed and return
                        for file_path in chunk.iter() {
                            update_progress_group_failure(
                                progress_state,
                                session_id,
                                file_path.clone(),
                                format!("Failed to create forum thread: {}", e),
                                true,
                                group.group_id.clone(),
                            );
                        }
                        return false;
                    }
                }

                // Clear text fields for image upload - images go to existing thread
                text_fields_for_images.clear();
            } else {
                // Non-forum channel: send text first, then overflow, then images
                if let Err(e) = client.send_text_message(
                    &webhook.url,
                    &main_content,
                    thread_id.as_deref(),
                ).await {
                    log::warn!("Failed to send initial text message: {}", e);
                } else {
                    // Send overflow messages
                    for (i, overflow_msg) in overflow_messages.iter().enumerate() {
                        if let Err(e) = client.send_text_message(
                            &webhook.url,
                            overflow_msg,
                            thread_id.as_deref(),
                        ).await {
                            log::warn!("Failed to send overflow message {}: {}", i + 1, e);
                        }
                    }
                }

                // Clear text fields for image upload
                text_fields_for_images.clear();
            }
        }

        // Enhanced payload validation for forum channels
        if is_forum_channel && chunk_index > 0 && thread_id.is_none() {
            log::error!("🔴 FATAL: Forum continuation chunk missing thread_id!");
            log::error!("   This will definitely cause Discord API error 400 code 220001");
            log::error!("   Failing group early to prevent API call");

            // Mark all remaining files as failed
            for remaining_chunk in chunks.iter().skip(chunk_index) {
                for file_path in remaining_chunk {
                    update_progress_group_failure(
                        progress_state,
                        session_id,
                        file_path.clone(),
                        "Forum continuation missing thread_id (thread_id extraction failed)"
                            .to_string(),
                        true,
                        group.group_id.clone(),
                    );
                }
            }
            return false;
        }

        // Update progress to show current files being uploaded/compressed
        for (file_index, file_path) in chunk.iter().enumerate() {
            if is_session_cancelled(progress_state, session_id) {
                log::info!(
                    "❌ Session {} cancelled while updating progress",
                    session_id
                );
                return false;
            }

            // Show initial progress for this file
            let file_progress = (file_index as f32 / chunk.len() as f32) * 100.0;
            update_progress_current_with_phase(
                progress_state,
                session_id,
                file_path.clone(),
                "Preparing",
                file_progress,
            );

            // Emit per-file progress event
            app_handle
                .emit_all(
                    "upload-item-progress",
                    serde_json::json!({
                        "session_id": session_id,
                        "phase": "preparing",
                        "file_path": file_path,
                        "file_index": file_index,
                        "chunk_size": chunk.len(),
                        "progress": file_progress
                    }),
                )
                .ok();
        }

        // Set main current image for the chunk
        if let Some(first_file) = chunk.first() {
            update_progress_current(progress_state, session_id, first_file.clone());
        }

        // Upload the chunk with thread_id support
        match upload_image_chunk_with_thread_id(
            client,
            webhook,
            chunk.clone(),
            text_fields_for_images,
            thread_id.as_deref(),
            progress_state,
            session_id,
            app_handle,
        )
        .await
        {
            Ok(response_data) => {
                if is_session_cancelled(progress_state, session_id) {
                    log::info!(
                        "❌ Session {} cancelled after successful chunk upload",
                        session_id
                    );
                    return false;
                }

                // For forum channels, extract thread_id from first response (if not already extracted via text message)
                if is_forum_channel && first_message && thread_id.is_none() {
                    log::info!(
                        "🔍 Attempting thread_id extraction from first forum message response..."
                    );

                    if let Some(extracted_thread_id) = extract_thread_id(&response_data) {
                        thread_id = Some(extracted_thread_id.clone());
                        log::info!(
                            "✅ SUCCESS: Forum post created with thread_id: {}",
                            extracted_thread_id
                        );
                    } else {
                        log::error!(
                            "❌ CRITICAL FAILURE: Failed to extract thread_id from forum response!"
                        );
                        log::error!("   Response length: {} bytes", response_data.len());
                        log::error!(
                            "   This will cause subsequent chunks to fail with error 220001"
                        );

                        // If we can't get the thread_id and have more chunks, fail the group immediately
                        if chunks.len() > 1 {
                            log::error!("🔴 Multiple chunks detected but no thread_id - failing group immediately");

                            // Mark all remaining files as failed
                            let remaining_files: Vec<String> = chunks
                                .iter()
                                .skip(chunk_index + 1)
                                .flatten()
                                .cloned()
                                .collect();

                            for file_path in &remaining_files {
                                update_progress_group_failure(progress_state, session_id, file_path.clone(),
                                    "Forum channel thread_id extraction failed - response missing thread info".to_string(), true, group.group_id.clone());
                            }

                            return false;
                        } else {
                            log::info!("ℹ️ Only one chunk, continuing despite thread_id extraction failure");
                        }
                    }
                }

                // Record successful uploads in database and update progress
                for (file_index, file_path) in chunk.iter().enumerate() {
                    let file_name = Path::new(file_path)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();

                    let file_hash = image_processor::get_file_hash(file_path).await.ok();
                    let file_size = security::FileSystemGuard::get_file_size(file_path).ok();

                    // Record in database (non-blocking)
                    let file_path_clone = file_path.clone();
                    let file_name_clone = file_name.clone();
                    let webhook_id = webhook.id;
                    tokio::spawn(async move {
                        let _ = database::record_upload(
                            file_path_clone,
                            file_name_clone,
                            file_hash,
                            file_size,
                            webhook_id,
                            "success",
                            None,
                        )
                        .await;
                    });

                    update_progress_success(progress_state, session_id, file_path.clone());

                    // Emit per-file success event
                    app_handle
                        .emit_all(
                            "upload-item-progress",
                            serde_json::json!({
                                "session_id": session_id,
                                "phase": "success",
                                "file_path": file_path,
                                "file_index": file_index,
                                "chunk_size": chunk.len()
                            }),
                        )
                        .ok();
                }

                log::info!(
                    "✅ Successfully uploaded chunk {} of group {} ({} images)",
                    chunk_index + 1,
                    group.group_id,
                    chunk.len()
                );
            }
            Err(e) => {
                log::error!("❌ CHUNK FAILED in group {}: {}", group.group_id, e);

                // Enhanced error logging for forum channels
                if is_forum_channel && e.to_string().contains("thread_name or thread_id") {
                    log::error!("🔴 FORUM CHANNEL ERROR 220001: Missing thread_name or thread_id");
                    log::error!("   Chunk index: {}", chunk_index);
                    log::error!("   Is first message: {}", first_message);
                    log::error!("   Thread ID available: {}", thread_id.is_some());

                    if chunk_index == 0 {
                        log::error!(
                            "   ❌ First message failed - likely webhook URL or thread_name issue"
                        );
                    } else {
                        log::error!(
                            "   ❌ Continuation message failed - thread_id should be in URL now"
                        );
                    }

                    log::error!(
                        "   💡 Check that wait=true and thread_id are in URL query parameters"
                    );
                }

                // Mark ALL remaining images in the group as failed (group failure)
                let remaining_files: Vec<String> =
                    chunks.iter().skip(chunk_index).flatten().cloned().collect();

                for file_path in &remaining_files {
                    let file_name = Path::new(file_path)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();

                    // Record failed upload in database (non-blocking)
                    let file_path_clone = file_path.clone();
                    let file_name_clone = file_name.clone();
                    let error_message = format!("Group failure: {}", e);
                    let webhook_id = webhook.id;
                    tokio::spawn(async move {
                        let _ = database::record_upload(
                            file_path_clone,
                            file_name_clone,
                            None,
                            None,
                            webhook_id,
                            "failed",
                            Some(error_message),
                        )
                        .await;
                    });

                    // Mark as group failure (retryable)
                    update_progress_group_failure(
                        progress_state,
                        session_id,
                        file_path.clone(),
                        format!("Forum channel group upload failed: {}", e),
                        true,
                        group.group_id.clone(),
                    );
                }

                // Emit progress update for failed group
                safe_emit_event(app_handle, "upload-progress", session_id);

                return false;
            }
        }

        first_message = false;

        // Emit progress update
        safe_emit_event(app_handle, "upload-progress", session_id);

        // Rate limiting delay between chunks (longer for forum channels)
        if is_forum_channel {
            sleep(Duration::from_millis(2000)).await; // 2s for forum channels
        } else {
            sleep(Duration::from_millis(1000)).await; // 1s for regular channels
        }
    }

    if is_forum_channel {
        log::info!(
            "🎉 Forum group {} completed successfully ({} images in {} chunks)",
            group.group_id,
            group.images.len(),
            chunks.len()
        );
    } else {
        log::info!(
            "✅ Group {} completed successfully ({} images)",
            group.group_id,
            group.images.len()
        );
    }
    true // Group succeeded
}

/// Upload image chunk with thread ID support
#[allow(clippy::too_many_arguments)]
pub async fn upload_image_chunk_with_thread_id(
    client: &DiscordClient,
    webhook: &Webhook,
    file_paths: Vec<String>,
    text_fields: HashMap<String, String>,
    thread_id: Option<&str>,
    progress_state: &ProgressState,
    session_id: &str,
    app_handle: &tauri::AppHandle,
) -> AppResult<String> {
    log::info!(
        "Starting upload of {} files for session {}",
        file_paths.len(),
        session_id
    );

    // Check cancellation before upload attempt
    if is_session_cancelled(progress_state, session_id) {
        return Err(AppError::upload_cancelled("upload start", session_id));
    }

    // Update progress to show upload phase
    if let Some(first_file) = file_paths.first() {
        update_progress_current_with_phase(
            progress_state,
            session_id,
            first_file.clone(),
            "Uploading",
            0.0,
        );
        safe_emit_event(app_handle, "upload-progress", session_id);

        // Emit streaming event for upload start
        app_handle
            .emit_all(
                "upload-item-progress",
                serde_json::json!({
                    "session_id": session_id,
                    "phase": "uploading",
                    "file_paths": file_paths,
                    "count": file_paths.len(),
                    "progress": 0
                }),
            )
            .ok();
    }

    // Try normal upload first
    let result = try_upload_chunk_with_thread_id(
        client,
        webhook,
        &file_paths,
        &text_fields,
        thread_id,
        progress_state,
        session_id,
    )
    .await;

    match result {
        Ok(response) => {
            log::info!(
                "Upload successful without compression for session {}",
                session_id
            );
            Ok(response)
        }
        Err(e) => {
            // Check cancellation before trying compression
            if is_session_cancelled(progress_state, session_id) {
                return Err(AppError::upload_cancelled("before compression", session_id));
            }

            // Check if it was a 413 error (Payload Too Large)
            if e.to_string().contains("413") || e.to_string().contains("Payload Too Large") {
                log::info!("Payload too large ({}), switching to compression mode for {} files in session {}", 
                    e.to_string().lines().next().unwrap_or("unknown error"), 
                    file_paths.len(),
                    session_id);
                upload_compressed_chunk_with_thread_id(
                    client,
                    webhook,
                    file_paths,
                    text_fields,
                    thread_id,
                    progress_state,
                    session_id,
                    app_handle,
                )
                .await
            } else {
                Err(e)
            }
        }
    }
}

/// Try upload without compression
async fn try_upload_chunk_with_thread_id(
    client: &DiscordClient,
    webhook: &Webhook,
    file_paths: &[String],
    text_fields: &HashMap<String, String>,
    thread_id: Option<&str>,
    progress_state: &ProgressState,
    session_id: &str,
) -> AppResult<String> {
    // Check cancellation before building payload
    if is_session_cancelled(progress_state, session_id) {
        return Err(AppError::upload_cancelled("payload creation", session_id));
    }

    // Log file sizes before upload
    let total_size: u64 = file_paths
        .iter()
        .filter_map(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .sum();
    let total_mb = total_size as f64 / 1024.0 / 1024.0;
    log::info!(
        "📤 Uploading {} files, total size: {:.2} MB",
        file_paths.len(),
        total_mb
    );

    let mut payload = UploadPayload::new();

    // Add text fields (no thread_id here!)
    for (key, value) in text_fields {
        payload.add_text_field(key.clone(), value.clone());
    }

    // Add image files with cancellation checks
    for (i, file_path) in file_paths.iter().enumerate() {
        // Check cancellation before each file
        if is_session_cancelled(progress_state, session_id) {
            return Err(AppError::upload_cancelled(
                &format!("adding file {} of {}", i + 1, file_paths.len()),
                session_id,
            ));
        }

        payload.add_file(file_path, format!("files[{}]", i)).await?;
    }

    // Final cancellation check before HTTP request
    if is_session_cancelled(progress_state, session_id) {
        return Err(AppError::upload_cancelled("HTTP request", session_id));
    }

    // Use the method that handles thread_id in URL
    client
        .send_webhook_with_thread_id(&webhook.url, &payload, thread_id)
        .await
}

/// Split files into chunks based on file size
/// Returns Vec of (file_paths, total_size) for each chunk
fn split_into_size_chunks(file_paths: &[String]) -> Vec<Vec<String>> {
    let mut chunks = Vec::new();
    let mut current_chunk = Vec::new();
    let mut current_size: u64 = 0;

    for path in file_paths {
        let file_size = std::fs::metadata(path)
            .map(|m| m.len())
            .unwrap_or(0);

        // If single file exceeds limit, put it alone in a chunk
        if file_size > SAFE_CHUNK_SIZE_BYTES {
            // First, save current chunk if not empty
            if !current_chunk.is_empty() {
                chunks.push(current_chunk);
                current_chunk = Vec::new();
                current_size = 0;
            }
            // Add oversized file as its own chunk
            chunks.push(vec![path.clone()]);
            continue;
        }

        // Would adding this file exceed the limit?
        if current_size + file_size > SAFE_CHUNK_SIZE_BYTES && !current_chunk.is_empty() {
            chunks.push(current_chunk);
            current_chunk = Vec::new();
            current_size = 0;
        }

        current_chunk.push(path.clone());
        current_size += file_size;
    }

    // Don't forget the last chunk
    if !current_chunk.is_empty() {
        chunks.push(current_chunk);
    }

    chunks
}

/// Upload with compression
#[allow(clippy::too_many_arguments)]
async fn upload_compressed_chunk_with_thread_id(
    client: &DiscordClient,
    webhook: &Webhook,
    file_paths: Vec<String>,
    text_fields: HashMap<String, String>,
    thread_id: Option<&str>,
    progress_state: &ProgressState,
    session_id: &str,
    app_handle: &tauri::AppHandle,
) -> AppResult<String> {
    let mut compressed_paths = Vec::new();
    let mut cleanup_paths = Vec::new();

    // Compress all images with progress updates and cancellation checks
    for (i, file_path) in file_paths.iter().enumerate() {
        // Check cancellation before each compression
        if is_session_cancelled(progress_state, session_id) {
            log::info!(
                "❌ Session {} cancelled during compression at file {}",
                session_id,
                i + 1
            );
            // Clean up any compressed files we've created so far
            for path in &cleanup_paths {
                tokio::fs::remove_file(path).await.ok();
            }
            return Err(AppError::upload_cancelled("compression", session_id));
        }

        // Update progress to show compression phase
        let compression_progress = (i as f32 / file_paths.len() as f32) * 50.0;
        let filename = Path::new(file_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();

        // Update UI progress and emit to frontend
        update_progress_current_with_phase(
            progress_state,
            session_id,
            file_path.clone(),
            "Compressing",
            compression_progress,
        );
        safe_emit_event(app_handle, "upload-progress", session_id);

        // Emit per-file compression progress
        app_handle
            .emit_all(
                "upload-item-progress",
                serde_json::json!({
                    "session_id": session_id,
                    "phase": "compressing",
                    "file_path": file_path,
                    "file_index": i,
                    "total": file_paths.len(),
                    "progress": compression_progress
                }),
            )
            .ok();

        log::info!(
            "Compressing image {} of {} ({}%): {}",
            i + 1,
            file_paths.len(),
            compression_progress as u32,
            filename
        );

        match image_processor::compress_image(file_path, 85).await {
            Ok(compressed_path) => {
                // Log compressed file size
                if let Ok(metadata) = std::fs::metadata(&compressed_path) {
                    let size_mb = metadata.len() as f64 / 1024.0 / 1024.0;
                    log::info!(
                        "  📷 {} -> {:.2} MB",
                        filename,
                        size_mb
                    );
                }
                compressed_paths.push(compressed_path.clone());
                cleanup_paths.push(compressed_path.clone());
            }
            Err(e) => {
                log::warn!("Failed to compress {}: {}, using original", file_path, e);
                compressed_paths.push(file_path.clone());
            }
        }
    }

    // Calculate and log total payload size before upload
    let total_size: u64 = compressed_paths
        .iter()
        .filter_map(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .sum();
    let total_mb = total_size as f64 / 1024.0 / 1024.0;
    log::info!(
        "📦 Total payload size: {:.2} MB ({} files)",
        total_mb,
        compressed_paths.len()
    );

    // Warn if approaching Discord's limit
    if total_size > 25 * 1024 * 1024 {
        log::warn!(
            "⚠️ Payload size ({:.2} MB) exceeds Discord's 25 MB limit!",
            total_mb
        );
    } else if total_size > 20 * 1024 * 1024 {
        log::warn!(
            "⚠️ Payload size ({:.2} MB) is close to Discord's 25 MB limit",
            total_mb
        );
    }

    // Final cancellation check before upload
    if is_session_cancelled(progress_state, session_id) {
        log::info!(
            "❌ Session {} cancelled after compression, before upload",
            session_id
        );
        // Clean up compressed files
        for path in &cleanup_paths {
            tokio::fs::remove_file(path).await.ok();
        }
        return Err(AppError::upload_cancelled("after compression", session_id));
    }

    // Update progress to show upload phase
    if let Some(first_file) = file_paths.first() {
        update_progress_current_with_phase(
            progress_state,
            session_id,
            first_file.clone(),
            "Uploading",
            50.0,
        );
        safe_emit_event(app_handle, "upload-progress", session_id);

        // Emit streaming event for compressed upload start
        app_handle
            .emit_all(
                "upload-item-progress",
                serde_json::json!({
                    "session_id": session_id,
                    "phase": "uploading_compressed",
                    "file_paths": file_paths,
                    "count": file_paths.len(),
                    "progress": 50
                }),
            )
            .ok();
    }

    log::info!(
        "Compression phase completed, starting upload of {} files",
        compressed_paths.len()
    );

    // Try uploading compressed files (remaining 50% of progress)
    let result = try_upload_chunk_with_thread_id(
        client,
        webhook,
        &compressed_paths,
        &text_fields,
        thread_id,
        progress_state,
        session_id,
    )
    .await;

    // Handle the result - if 413, try splitting into smaller chunks
    let final_result = match result {
        Ok(response) => Ok(response),
        Err(e) if e.to_string().contains("413") || e.to_string().contains("Payload Too Large") => {
            log::info!(
                "📦 Compressed upload still too large, splitting into size-based chunks..."
            );

            // Split compressed files into size-based chunks
            let chunks = split_into_size_chunks(&compressed_paths);
            log::info!(
                "📤 Split {} files into {} chunks based on size",
                compressed_paths.len(),
                chunks.len()
            );

            // Log chunk details
            for (i, chunk) in chunks.iter().enumerate() {
                let chunk_size: u64 = chunk
                    .iter()
                    .filter_map(|p| std::fs::metadata(p).ok())
                    .map(|m| m.len())
                    .sum();
                log::info!(
                    "  Chunk {}: {} files, {:.2} MB",
                    i + 1,
                    chunk.len(),
                    chunk_size as f64 / 1024.0 / 1024.0
                );
            }

            // Upload each chunk
            // For non-forum channels (thread_id is None), don't try to extract or use thread_id
            // For forum channels (thread_id is Some), keep using the same thread
            let mut last_response = String::new();

            for (chunk_idx, chunk_files) in chunks.iter().enumerate() {
                // Check cancellation before each chunk
                if is_session_cancelled(progress_state, session_id) {
                    // Cleanup before returning
                    for path in &cleanup_paths {
                        if !file_paths.contains(path) {
                            tokio::fs::remove_file(path).await.ok();
                        }
                    }
                    return Err(AppError::upload_cancelled("chunk upload", session_id));
                }

                log::info!(
                    "📤 Uploading chunk {}/{} ({} files)",
                    chunk_idx + 1,
                    chunks.len(),
                    chunk_files.len()
                );

                // Only include text content in first chunk
                // Continuation chunks upload silently without text
                let chunk_text_fields = if chunk_idx == 0 {
                    text_fields.clone()
                } else {
                    HashMap::new()
                };

                // For non-forum channels, don't use thread_id at all
                // For forum channels, use the existing thread_id passed in
                let chunk_result = try_upload_chunk_with_thread_id(
                    client,
                    webhook,
                    chunk_files,
                    &chunk_text_fields,
                    thread_id, // Use original thread_id, don't extract new ones
                    progress_state,
                    session_id,
                )
                .await;

                match chunk_result {
                    Ok(response) => {
                        last_response = response;
                        log::info!("✅ Chunk {}/{} uploaded successfully", chunk_idx + 1, chunks.len());
                    }
                    Err(chunk_err) => {
                        log::error!(
                            "❌ Failed to upload chunk {}/{}: {}",
                            chunk_idx + 1,
                            chunks.len(),
                            chunk_err
                        );
                        // Cleanup before returning
                        for path in &cleanup_paths {
                            if !file_paths.contains(path) {
                                tokio::fs::remove_file(path).await.ok();
                            }
                        }
                        return Err(chunk_err);
                    }
                }

                // Small delay between chunks to avoid rate limiting
                if chunk_idx < chunks.len() - 1 {
                    sleep(Duration::from_millis(500)).await;
                }
            }

            Ok(last_response)
        }
        Err(e) => Err(e),
    };

    // Clean up compressed files
    for path in &cleanup_paths {
        if !file_paths.contains(path) {
            if let Err(e) = tokio::fs::remove_file(path).await {
                log::warn!("Failed to cleanup compressed file {}: {}", path, e);
            } else {
                log::debug!("Cleaned up compressed file: {}", path);
            }
        }
    }

    final_result
}
