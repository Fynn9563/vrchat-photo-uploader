use crate::commands::FailedUpload;
use crate::errors::{safe_progress_read, safe_progress_update, ProgressState};
use std::path::Path;
use tokio::time::Instant;

/// Check if an upload session has been cancelled
pub fn is_session_cancelled(progress_state: &ProgressState, session_id: &str) -> bool {
    safe_progress_read(
        progress_state,
        session_id,
        "cancellation check",
        |progress| progress.session_status == "cancelled",
    )
    .unwrap_or(true) // Treat missing/locked session as cancelled for safety
}

/// Mark an upload session as cancelled
pub fn mark_session_cancelled(progress_state: &ProgressState, session_id: &str) {
    safe_progress_update(progress_state, session_id, "mark cancelled", |progress| {
        progress.session_status = "cancelled".to_string();
        progress.estimated_time_remaining = Some(0);
        log::info!(
            "Marked session {} as cancelled with {} completed uploads",
            session_id,
            progress.completed
        );
    });
}

/// Update progress to show current file being processed
pub fn update_progress_current(
    progress_state: &ProgressState,
    session_id: &str,
    file_path: String,
) {
    safe_progress_update(
        progress_state,
        session_id,
        "current file update",
        |progress| {
            progress.current_image = Some(file_path.clone());
            progress.current_progress = 0.0;
            log::debug!("Progress: Currently uploading {}", file_path);
        },
    );
}

/// Update progress with phase information (e.g., "Compressing", "Uploading")
pub fn update_progress_current_with_phase(
    progress_state: &ProgressState,
    session_id: &str,
    file_path: String,
    phase: &str,
    progress_percent: f32,
) {
    safe_progress_update(progress_state, session_id, "phase update", |progress| {
        let filename = Path::new(&file_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();

        progress.current_image = Some(format!("{} - {}", phase, filename));
        progress.current_progress = progress_percent;
        log::debug!(
            "Progress: {} {} ({}%)",
            phase,
            file_path,
            progress_percent as u32
        );
    });
}

/// Mark a file upload as successful
pub fn update_progress_success(
    progress_state: &ProgressState,
    session_id: &str,
    file_path: String,
) {
    safe_progress_update(progress_state, session_id, "success update", |progress| {
        progress.completed += 1;
        progress.successful_uploads.push(file_path.clone());
        progress.current_progress = 100.0;

        // Remove from failed uploads if it was previously failed
        progress.failed_uploads.retain(|f| f.file_path != file_path);

        log::info!(
            "Progress: Successfully uploaded {} ({}/{})",
            file_path,
            progress.completed,
            progress.total_images
        );
    });
}

/// Mark a file upload as failed
pub fn update_progress_failure(
    progress_state: &ProgressState,
    session_id: &str,
    file_path: String,
    error: String,
    is_retryable: bool,
) {
    safe_progress_update(progress_state, session_id, "failure update", |progress| {
        progress.completed += 1;

        // Check if this file already failed, increment retry count
        if let Some(existing_failure) = progress
            .failed_uploads
            .iter_mut()
            .find(|f| f.file_path == file_path)
        {
            existing_failure.retry_count += 1;
            existing_failure.error = error.clone();
            existing_failure.is_retryable = is_retryable;
        } else {
            progress.failed_uploads.push(FailedUpload {
                file_path: file_path.clone(),
                error: error.clone(),
                retry_count: 0,
                is_retryable,
            });
        }

        log::warn!(
            "Progress: Failed to upload {} - {} ({}/{})",
            file_path,
            error,
            progress.completed,
            progress.total_images
        );
    });
}

/// Mark a group of files as failed (used for forum channel group failures)
pub fn update_progress_group_failure(
    progress_state: &ProgressState,
    session_id: &str,
    file_path: String,
    error: String,
    is_retryable: bool,
    group_id: String,
) {
    safe_progress_update(
        progress_state,
        session_id,
        "group failure update",
        |progress| {
            progress.completed += 1;

            progress.failed_uploads.push(FailedUpload {
                file_path: file_path.clone(),
                error: format!("[Group: {}] {}", group_id, error),
                retry_count: 0,
                is_retryable,
            });

            log::warn!(
                "Progress: Group failure for {} in group {} - {}",
                file_path,
                group_id,
                error
            );
        },
    );
}

/// Update the estimated time remaining for upload completion
pub fn update_time_estimate(
    progress_state: &ProgressState,
    session_id: &str,
    start_time: Instant,
    completed: usize,
    total: usize,
) {
    if completed == 0 {
        return;
    }

    let elapsed = start_time.elapsed().as_secs_f64();
    let rate = completed as f64 / elapsed;
    let remaining = total - completed;

    // Account for potential compression overhead (estimate 30% longer if compression is likely needed)
    let compression_factor = 1.3; // Assume 30% overhead for compression if needed
    let estimated_seconds = if rate > 0.0 {
        ((remaining as f64 / rate) * compression_factor) as u64
    } else {
        0
    };

    safe_progress_update(
        progress_state,
        session_id,
        "time estimate update",
        |progress| {
            progress.estimated_time_remaining = Some(estimated_seconds);

            // Log time estimate for debugging
            if estimated_seconds > 0 {
                let minutes = estimated_seconds / 60;
                let seconds = estimated_seconds % 60;
                log::debug!(
                    "ETA updated: {}m {}s (rate: {:.2} images/sec, remaining: {})",
                    minutes,
                    seconds,
                    rate,
                    remaining
                );
            }
        },
    );
}

/// Mark session as completed
pub fn mark_session_completed(progress_state: &ProgressState, session_id: &str) {
    safe_progress_update(progress_state, session_id, "mark completed", |progress| {
        progress.session_status = "completed".to_string();
        progress.estimated_time_remaining = Some(0);

        log::info!(
            "Session {} completed: {}/{} successful, {} failed",
            session_id,
            progress.successful_uploads.len(),
            progress.total_images,
            progress.failed_uploads.len()
        );
    });
}

/// Mark session as failed
pub fn mark_session_failed(progress_state: &ProgressState, session_id: &str) {
    safe_progress_update(progress_state, session_id, "mark failed", |progress| {
        progress.session_status = "failed".to_string();
        progress.estimated_time_remaining = Some(0);

        log::error!(
            "Session {} marked as failed: {}/{} successful, {} failed",
            session_id,
            progress.successful_uploads.len(),
            progress.total_images,
            progress.failed_uploads.len()
        );
    });
}
