use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::commands::AppConfig;
use crate::errors::{AppError, AppResult};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub last_webhook_id: Option<i64>,
    pub group_by_metadata: bool,
    pub max_images_per_message: u8,
    pub enable_global_shortcuts: bool,
    pub theme: String,
    pub upload_quality: u8,
    pub auto_compress_threshold: u64, // File size in MB
    pub preserve_timestamps: bool,
    pub auto_cleanup_days: u32,
    pub rate_limit_delay_ms: u64,
    pub max_retry_attempts: u32,
    pub backup_original_files: bool,
    pub show_upload_notifications: bool,
    pub log_level: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            last_webhook_id: None,
            group_by_metadata: true,
            max_images_per_message: 10,
            enable_global_shortcuts: true,
            theme: "dark".to_string(),
            upload_quality: 85,
            auto_compress_threshold: 8, // 8MB
            preserve_timestamps: true,
            auto_cleanup_days: 30,
            rate_limit_delay_ms: 1000,
            max_retry_attempts: 3,
            backup_original_files: false,
            show_upload_notifications: true,
            log_level: "info".to_string(),
        }
    }
}

impl From<Config> for AppConfig {
    fn from(config: Config) -> Self {
        AppConfig {
            last_webhook_id: config.last_webhook_id,
            group_by_metadata: config.group_by_metadata,
            max_images_per_message: config.max_images_per_message,
            enable_global_shortcuts: config.enable_global_shortcuts,
            auto_compress_threshold: config.auto_compress_threshold,
            upload_quality: config.upload_quality,
        }
    }
}

impl From<AppConfig> for Config {
    fn from(app_config: AppConfig) -> Self {
        let mut config = Config::default();
        config.last_webhook_id = app_config.last_webhook_id;
        config.group_by_metadata = app_config.group_by_metadata;
        config.max_images_per_message = app_config.max_images_per_message;
        config.enable_global_shortcuts = app_config.enable_global_shortcuts;
        config.auto_compress_threshold = app_config.auto_compress_threshold;
        config.upload_quality = app_config.upload_quality;
        config
    }
}

fn get_config_path() -> AppResult<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| AppError::Config("Could not find config directory".to_string()))?
        .join("VRChat Photo Uploader");
    
    fs::create_dir_all(&config_dir)?;
    Ok(config_dir.join("config.json"))
}

pub fn load_config() -> AppResult<AppConfig> {
    let config_path = get_config_path()?;
    
    if config_path.exists() {
        let config_str = fs::read_to_string(&config_path)?;
        let config: Config = serde_json::from_str(&config_str)
            .unwrap_or_else(|e| {
                log::warn!("Failed to parse config file: {}. Using defaults.", e);
                Config::default()
            });
        
        // Validate config before returning
        validate_config(&config)?;
        
        Ok(config.into())
    } else {
        // Create default config
        let default_config = Config::default();
        save_config_internal(&default_config)?;
        Ok(default_config.into())
    }
}

pub fn save_config(app_config: AppConfig) -> AppResult<()> {
    let config: Config = app_config.into();
    validate_config(&config)?;
    save_config_internal(&config)
}

fn save_config_internal(config: &Config) -> AppResult<()> {
    let config_path = get_config_path()?;
    
    // Create backup of existing config
    if config_path.exists() {
        let backup_path = config_path.with_extension("json.bak");
        if let Err(e) = fs::copy(&config_path, &backup_path) {
            log::warn!("Failed to create config backup: {}", e);
        }
    }
    
    let config_str = serde_json::to_string_pretty(config)?;
    fs::write(&config_path, config_str)?;
    
    log::info!("Configuration saved successfully");
    Ok(())
}

pub fn get_data_directory() -> AppResult<PathBuf> {
    let data_dir = dirs::data_dir()
        .ok_or_else(|| AppError::Config("Could not find data directory".to_string()))?
        .join("VRChat Photo Uploader");
    
    fs::create_dir_all(&data_dir)?;
    Ok(data_dir)
}

pub fn get_logs_directory() -> AppResult<PathBuf> {
    let logs_dir = get_data_directory()?.join("logs");
    fs::create_dir_all(&logs_dir)?;
    Ok(logs_dir)
}

pub fn get_temp_directory() -> AppResult<PathBuf> {
    let temp_dir = std::env::temp_dir().join("vrchat_photo_uploader");
    fs::create_dir_all(&temp_dir)?;
    Ok(temp_dir)
}

// Utility functions for common paths
pub fn get_default_vrchat_screenshots_path() -> Option<PathBuf> {
    // Try to find VRChat's default screenshot directory
    if let Some(pictures_dir) = dirs::picture_dir() {
        let vrchat_path = pictures_dir.join("VRChat");
        if vrchat_path.exists() {
            return Some(vrchat_path);
        }
    }
    
    // Alternative common locations
    if let Some(documents_dir) = dirs::document_dir() {
        let vrchat_path = documents_dir.join("VRChat");
        if vrchat_path.exists() {
            return Some(vrchat_path);
        }
    }
    
    // Check AppData for Windows
    #[cfg(target_os = "windows")]
    {
        if let Some(local_appdata) = dirs::data_local_dir() {
            let vrchat_path = local_appdata.join("VRChat").join("VRChat");
            if vrchat_path.exists() {
                return Some(vrchat_path);
            }
        }
    }
    
    None
}

pub fn validate_config(config: &Config) -> AppResult<()> {
    if config.max_images_per_message == 0 || config.max_images_per_message > 10 {
        return Err(AppError::validation("max_images_per_message", "Must be between 1 and 10"));
    }
    
    if config.upload_quality == 0 || config.upload_quality > 100 {
        return Err(AppError::validation("upload_quality", "Must be between 1 and 100"));
    }
    
    if config.auto_compress_threshold == 0 {
        return Err(AppError::validation("auto_compress_threshold", "Must be greater than 0"));
    }
    
    if config.auto_cleanup_days == 0 {
        return Err(AppError::validation("auto_cleanup_days", "Must be greater than 0"));
    }
    
    if config.rate_limit_delay_ms < 100 {
        return Err(AppError::validation("rate_limit_delay_ms", "Must be at least 100ms"));
    }
    
    if config.max_retry_attempts > 10 {
        return Err(AppError::validation("max_retry_attempts", "Must be 10 or fewer"));
    }
    
    // Validate theme
    let valid_themes = ["dark", "light", "auto"];
    if !valid_themes.contains(&config.theme.as_str()) {
        return Err(AppError::validation("theme", "Must be 'dark', 'light', or 'auto'"));
    }
    
    // Validate log level
    let valid_log_levels = ["error", "warn", "info", "debug", "trace"];
    if !valid_log_levels.contains(&config.log_level.as_str()) {
        return Err(AppError::validation("log_level", "Must be a valid log level"));
    }
    
    Ok(())
}

// Configuration migration for version updates
pub fn migrate_config() -> AppResult<()> {
    let config_path = get_config_path()?;
    
    if !config_path.exists() {
        return Ok(()); // Nothing to migrate
    }
    
    let config_str = fs::read_to_string(&config_path)?;
    
    // Try to parse as current version first
    if serde_json::from_str::<Config>(&config_str).is_ok() {
        return Ok(()); // Already current version
    }
    
    // Try to parse as older versions and migrate
    log::info!("Migrating configuration to new format");
    
    // For now, just create a new default config if migration fails
    let default_config = Config::default();
    save_config_internal(&default_config)?;
    
    // Backup the old config
    let backup_path = config_path.with_extension("json.old");
    fs::copy(&config_path, &backup_path)?;
    
    log::info!("Old configuration backed up to {}", backup_path.display());
    
    Ok(())
}

// Auto-cleanup functionality
pub async fn auto_cleanup() -> AppResult<()> {
    let config = load_config()?;
    let cleanup_days = Config::from(config).auto_cleanup_days as i32;
    
    // Cleanup old upload sessions
    let sessions_cleaned = crate::database::cleanup_old_upload_sessions(cleanup_days).await?;
    
    // Cleanup old upload history
    let history_cleaned = crate::database::cleanup_old_upload_history(cleanup_days).await?;
    
    // Cleanup temp files
    if let Ok(temp_dir) = get_temp_directory() {
        cleanup_old_files(&temp_dir, cleanup_days)?;
    }
    
    // Cleanup old log files
    if let Ok(logs_dir) = get_logs_directory() {
        cleanup_old_files(&logs_dir, cleanup_days)?;
    }
    
    log::info!(
        "Auto-cleanup completed: {} sessions, {} history entries cleaned", 
        sessions_cleaned, 
        history_cleaned
    );
    
    Ok(())
}

fn cleanup_old_files(directory: &PathBuf, days: i32) -> AppResult<()> {
    if !directory.exists() {
        return Ok(());
    }
    
    let cutoff_time = std::time::SystemTime::now() 
        - std::time::Duration::from_secs((days as u64) * 24 * 60 * 60);
    
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_file() {
            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    if modified < cutoff_time {
                        if let Err(e) = fs::remove_file(&path) {
                            log::warn!("Failed to remove old file {}: {}", path.display(), e);
                        } else {
                            log::debug!("Removed old file: {}", path.display());
                        }
                    }
                }
            }
        }
    }
    
    Ok(())
}

// Reset configuration to defaults
pub fn reset_config() -> AppResult<()> {
    let config_path = get_config_path()?;
    
    // Backup existing config
    if config_path.exists() {
        let backup_path = config_path.with_extension("json.reset_backup");
        fs::copy(&config_path, &backup_path)?;
        log::info!("Existing config backed up to {}", backup_path.display());
    }
    
    // Save default config
    let default_config = Config::default();
    save_config_internal(&default_config)?;
    
    log::info!("Configuration reset to defaults");
    Ok(())
}