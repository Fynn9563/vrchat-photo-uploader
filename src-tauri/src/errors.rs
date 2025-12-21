use crate::commands::UploadProgress;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::Manager; // Add this import for emit_all
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Image processing error: {0}")]
    Image(#[from] image::ImageError),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid webhook URL: {url}")]
    InvalidWebhook { url: String },

    #[error("File not found: {path}")]
    FileNotFound { path: String },

    #[error("Invalid file type: {path}. Only image files are supported.")]
    InvalidFileType { path: String },

    #[error("File too large: {path}. Maximum size is 50MB.")]
    FileTooLarge { path: String },

    #[error("Metadata parsing error: {0}")]
    MetadataParsing(String),

    #[error("Upload failed: {reason}")]
    UploadFailed { reason: String },

    #[error("Validation error: {field} - {message}")]
    Validation { field: String, message: String },

    #[error("Rate limit exceeded. Retry after {retry_after_ms}ms")]
    RateLimit { retry_after_ms: u64 },

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Internal error: {0}")]
    Internal(String),

    // New upload-specific errors
    #[error("Upload cancelled during {phase} for session {session_id}")]
    UploadCancelled { phase: String, session_id: String },

    #[error("Progress update failed for session {session_id}")]
    ProgressUpdateFailed { session_id: String },

    #[error("Forum channel error: {message}")]
    ForumChannelError { message: String },
}

/// Convert to string for Tauri
impl From<AppError> for String {
    fn from(error: AppError) -> Self {
        error.to_string()
    }
}

/// Custom result type
pub type AppResult<T> = Result<T, AppError>;

/// Upload error helpers
impl AppError {
    pub fn validation(field: &str, message: &str) -> Self {
        Self::Validation {
            field: field.to_string(),
            message: message.to_string(),
        }
    }

    pub fn file_not_found(path: &str) -> Self {
        Self::FileNotFound {
            path: path.to_string(),
        }
    }

    pub fn invalid_file_type(path: &str) -> Self {
        Self::InvalidFileType {
            path: path.to_string(),
        }
    }

    pub fn file_too_large(path: &str) -> Self {
        Self::FileTooLarge {
            path: path.to_string(),
        }
    }

    pub fn invalid_webhook(url: &str) -> Self {
        Self::InvalidWebhook {
            url: url.to_string(),
        }
    }

    pub fn upload_cancelled(phase: &str, session_id: &str) -> Self {
        Self::UploadCancelled {
            phase: phase.to_string(),
            session_id: session_id.to_string(),
        }
    }

    pub fn progress_update_failed(session_id: &str, operation: &str) -> Self {
        log::error!(
            "Failed to acquire progress lock for {} in session {} (non-critical)",
            operation,
            session_id
        );
        Self::ProgressUpdateFailed {
            session_id: session_id.to_string(),
        }
    }

    pub fn forum_channel_error(message: &str) -> Self {
        Self::ForumChannelError {
            message: message.to_string(),
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            AppError::Network(_)
                | AppError::RateLimit { .. }
                | AppError::UploadFailed { .. }
                | AppError::Io(_)
                | AppError::ForumChannelError { .. }
        )
    }

    pub fn is_permanent(&self) -> bool {
        matches!(
            self,
            AppError::InvalidWebhook { .. }
                | AppError::FileNotFound { .. }
                | AppError::InvalidFileType { .. }
                | AppError::FileTooLarge { .. }
                | AppError::Validation { .. }
        )
    }
}

/// Progress state type
pub type ProgressState = Arc<Mutex<HashMap<String, UploadProgress>>>;

/// Safe progress state update
pub fn safe_progress_update<F>(
    progress_state: &ProgressState,
    session_id: &str,
    operation: &str,
    f: F,
) -> bool
where
    F: FnOnce(&mut UploadProgress),
{
    match progress_state.lock() {
        Ok(mut progress) => {
            if let Some(session_progress) = progress.get_mut(session_id) {
                f(session_progress);
                true
            } else {
                log::warn!(
                    "Session {} not found for {} operation",
                    session_id,
                    operation
                );
                false
            }
        }
        Err(e) => {
            log::error!(
                "Failed to acquire progress lock for {} in session {} (non-critical): {}",
                operation,
                session_id,
                e
            );
            false
        }
    }
}

pub fn safe_progress_read<F, R>(
    progress_state: &ProgressState,
    session_id: &str,
    operation: &str,
    f: F,
) -> Option<R>
where
    F: FnOnce(&UploadProgress) -> R,
{
    match progress_state.lock() {
        Ok(progress) => {
            if let Some(session_progress) = progress.get(session_id) {
                Some(f(session_progress))
            } else {
                log::warn!(
                    "Session {} not found for {} operation",
                    session_id,
                    operation
                );
                None
            }
        }
        Err(e) => {
            log::error!(
                "Failed to acquire progress lock for {} in session {} (non-critical): {}",
                operation,
                session_id,
                e
            );
            None
        }
    }
}

/// Emit UI event with error handling
pub fn safe_emit_event(app_handle: &tauri::AppHandle, event_name: &str, payload: &str) -> bool {
    match app_handle.emit_all(event_name, payload) {
        Ok(_) => {
            log::debug!(
                "Successfully emitted event '{}' with payload: {}",
                event_name,
                payload
            );
            true
        }
        Err(e) => {
            log::warn!(
                "Failed to emit event '{}' (non-critical): {}",
                event_name,
                e
            );
            false
        }
    }
}
