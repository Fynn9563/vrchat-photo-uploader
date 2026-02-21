use crate::commands::UploadProgress;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::Emitter;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Image processing error: {0}")]
    Image(#[from] image::ImageError),

    #[error("Image processing failure: {0}")]
    ImageProcessing(String),

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
            "Failed to acquire progress lock for {operation} in session {session_id} (non-critical)"
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
                    "Session {session_id} not found for {operation} operation"
                );
                false
            }
        }
        Err(e) => {
            log::error!(
                "Failed to acquire progress lock for {operation} in session {session_id} (non-critical): {e}"
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
                    "Session {session_id} not found for {operation} operation"
                );
                None
            }
        }
        Err(e) => {
            log::error!(
                "Failed to acquire progress lock for {operation} in session {session_id} (non-critical): {e}"
            );
            None
        }
    }
}

/// Emit UI event with error handling
pub fn safe_emit_event(app_handle: &tauri::AppHandle, event_name: &str, payload: &str) -> bool {
    match app_handle.emit(event_name, payload) {
        Ok(_) => {
            log::debug!(
                "Successfully emitted event '{event_name}' with payload: {payload}"
            );
            true
        }
        Err(e) => {
            log::warn!(
                "Failed to emit event '{event_name}' (non-critical): {e}"
            );
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_retryable_upload_failed() {
        let err = AppError::UploadFailed { reason: "timeout".to_string() };
        assert!(err.is_retryable());
    }

    #[test]
    fn test_is_retryable_rate_limit() {
        let err = AppError::RateLimit { retry_after_ms: 5000 };
        assert!(err.is_retryable());
    }

    #[test]
    fn test_is_retryable_io() {
        let err = AppError::Io(std::io::Error::other("test"));
        assert!(err.is_retryable());
    }

    #[test]
    fn test_is_retryable_forum_channel() {
        let err = AppError::ForumChannelError { message: "wrong type".to_string() };
        assert!(err.is_retryable());
    }

    #[test]
    fn test_is_permanent_invalid_webhook() {
        let err = AppError::invalid_webhook("https://bad.url");
        assert!(err.is_permanent());
    }

    #[test]
    fn test_is_permanent_file_not_found() {
        let err = AppError::file_not_found("/missing/file.png");
        assert!(err.is_permanent());
    }

    #[test]
    fn test_is_permanent_invalid_file_type() {
        let err = AppError::invalid_file_type("video.mp4");
        assert!(err.is_permanent());
    }

    #[test]
    fn test_is_permanent_file_too_large() {
        let err = AppError::file_too_large("huge.png");
        assert!(err.is_permanent());
    }

    #[test]
    fn test_is_permanent_validation() {
        let err = AppError::validation("field", "bad value");
        assert!(err.is_permanent());
    }

    #[test]
    fn test_not_retryable_permanent_errors() {
        assert!(!AppError::file_not_found("f").is_retryable());
        assert!(!AppError::invalid_webhook("u").is_retryable());
        assert!(!AppError::invalid_file_type("t").is_retryable());
    }

    #[test]
    fn test_not_permanent_retryable_errors() {
        let err = AppError::UploadFailed { reason: "x".to_string() };
        assert!(!err.is_permanent());
    }

    #[test]
    fn test_validation_constructor() {
        let err = AppError::validation("email", "invalid format");
        match err {
            AppError::Validation { field, message } => {
                assert_eq!(field, "email");
                assert_eq!(message, "invalid format");
            }
            _ => panic!("Expected Validation variant"),
        }
    }

    #[test]
    fn test_file_not_found_constructor() {
        let err = AppError::file_not_found("/path/to/file");
        match err {
            AppError::FileNotFound { path } => assert_eq!(path, "/path/to/file"),
            _ => panic!("Expected FileNotFound variant"),
        }
    }

    #[test]
    fn test_upload_cancelled_constructor() {
        let err = AppError::upload_cancelled("compression", "session_123");
        match err {
            AppError::UploadCancelled { phase, session_id } => {
                assert_eq!(phase, "compression");
                assert_eq!(session_id, "session_123");
            }
            _ => panic!("Expected UploadCancelled variant"),
        }
    }

    #[test]
    fn test_forum_channel_error_constructor() {
        let err = AppError::forum_channel_error("not a forum");
        match err {
            AppError::ForumChannelError { message } => assert_eq!(message, "not a forum"),
            _ => panic!("Expected ForumChannelError variant"),
        }
    }

    #[test]
    fn test_display_contains_expected_text() {
        let err = AppError::UploadFailed { reason: "network timeout".to_string() };
        let display = format!("{err}");
        assert!(display.contains("network timeout"), "Display should contain reason: {display}");
    }

    #[test]
    fn test_into_string_conversion() {
        let err = AppError::file_not_found("test.png");
        let s: String = err.into();
        assert!(s.contains("test.png"));
    }
}
