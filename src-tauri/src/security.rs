use regex::Regex;
use std::path::Path;
use crate::errors::{AppError, AppResult};

pub struct InputValidator;

impl InputValidator {
    pub fn validate_webhook_name(name: &str) -> AppResult<()> {
        let trimmed = name.trim();
        
        if trimmed.is_empty() {
            return Err(AppError::validation("name", "Webhook name cannot be empty"));
        }
        
        if trimmed.len() > 100 {
            return Err(AppError::validation("name", "Webhook name too long (max 100 characters)"));
        }
        
        // Check for potentially dangerous characters
        let safe_chars = Regex::new(r"^[a-zA-Z0-9\s\-_\.]+$").unwrap();
        if !safe_chars.is_match(trimmed) {
            return Err(AppError::validation("name", "Webhook name contains invalid characters"));
        }
        
        Ok(())
    }
    
    pub fn validate_webhook_url(url: &str) -> AppResult<()> {
        let trimmed = url.trim();
        
        if trimmed.is_empty() {
            return Err(AppError::validation("url", "Webhook URL cannot be empty"));
        }
        
        // More comprehensive URL validation
        let webhook_pattern = Regex::new(
            r"^https://(discord\.com|discordapp\.com)/api/webhooks/\d{17,19}/[\w\-_]{68}$"
        ).unwrap();
        
        if !webhook_pattern.is_match(trimmed) {
            return Err(AppError::invalid_webhook(trimmed));
        }
        
        // Additional URL safety checks
        if trimmed.len() > 500 {
            return Err(AppError::validation("url", "Webhook URL too long"));
        }
        
        Ok(())
    }
    
    pub fn validate_file_path(path: &str) -> AppResult<()> {
        if path.trim().is_empty() {
            return Err(AppError::validation("file_path", "File path cannot be empty"));
        }
        
        let path_obj = Path::new(path);
        
        // Check for path traversal attempts
        if path.contains("..") || path.contains("~") {
            return Err(AppError::validation("file_path", "Invalid file path detected"));
        }
        
        // Ensure it's an image file
        if let Some(extension) = path_obj.extension() {
            let ext = extension.to_string_lossy().to_lowercase();
            if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp") {
                return Err(AppError::invalid_file_type(path));
            }
        } else {
            return Err(AppError::validation("file_path", "File must have an extension"));
        }
        
        // Check file exists and is readable
        if !path_obj.exists() {
            return Err(AppError::file_not_found(path));
        }
        
        if !path_obj.is_file() {
            return Err(AppError::validation("file_path", "Path is not a file"));
        }
        
        Ok(())
    }
    
    pub fn sanitize_filename(filename: &str) -> String {
        // Remove or replace unsafe characters in filenames
        let unsafe_chars = Regex::new(r#"[<>:"/\\|?*\x00-\x1f]"#).unwrap();
        let sanitized = unsafe_chars.replace_all(filename.trim(), "_");
        
        // Limit length
        if sanitized.len() > 255 {
            format!("{}...", &sanitized[..252])
        } else {
            sanitized.to_string()
        }
    }
    
    pub fn validate_image_file(file_path: &str) -> AppResult<()> {
        Self::validate_file_path(file_path)?;
        
        // Additional image-specific validation
        let metadata = std::fs::metadata(file_path)?;
        
        // Check file size (max 25MB for Discord)
        const MAX_FILE_SIZE: u64 = 25 * 1024 * 1024;
        if metadata.len() > MAX_FILE_SIZE {
            return Err(AppError::file_too_large(file_path));
        }
        
        // Verify it's actually an image by trying to open it
        image::open(file_path)?;
        
        Ok(())
    }
    
    pub fn validate_upload_settings(max_images: u8, _group_metadata: bool) -> AppResult<()> {
        if max_images == 0 || max_images > 10 {
            return Err(AppError::validation("max_images", "Must be between 1 and 10"));
        }
        
        Ok(())
    }
}

// File system security utilities
pub struct FileSystemGuard;

impl FileSystemGuard {
    pub fn create_secure_temp_file(original_path: &str) -> AppResult<std::path::PathBuf> {
        let temp_dir = std::env::temp_dir().join("vrchat_uploader_secure");
        std::fs::create_dir_all(&temp_dir)?;
        
        // Generate secure random filename
        let random_name = uuid::Uuid::new_v4().to_string();
        let extension = Path::new(original_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("tmp");
        
        let temp_path = temp_dir.join(format!("{}.{}", random_name, extension));
        
        Ok(temp_path)
    }
    
    pub fn cleanup_temp_files() -> AppResult<()> {
        let temp_dir = std::env::temp_dir().join("vrchat_uploader_secure");
        if temp_dir.exists() {
            std::fs::remove_dir_all(&temp_dir)?;
        }
        Ok(())
    }
    
    pub fn get_file_size(path: &str) -> AppResult<u64> {
        let metadata = std::fs::metadata(path)?;
        Ok(metadata.len())
    }
    
}