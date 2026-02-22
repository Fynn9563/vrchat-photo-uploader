use sqlx::{Pool, Row, Sqlite, SqlitePool};
use std::sync::OnceLock;

use crate::commands::Webhook;
use crate::errors::{AppError, AppResult};

pub static DB_POOL: OnceLock<Pool<Sqlite>> = OnceLock::new();

pub async fn init_database() -> AppResult<()> {
    let data_dir = dirs::data_dir()
        .ok_or_else(|| AppError::Config("Could not find data directory".to_string()))?
        .join("VRChat Photo Uploader");

    // Ensure directory exists with proper permissions
    std::fs::create_dir_all(&data_dir)?;
    log::info!("Database directory: {}", data_dir.display());

    let db_path = data_dir.join("DiscordWebhooks.db");
    log::info!("Database path: {}", db_path.display());

    // Check if we can write to the directory
    let test_file = data_dir.join("test_write_permissions");
    match std::fs::write(&test_file, "test") {
        Ok(_) => {
            std::fs::remove_file(&test_file).ok();
            log::info!("Directory write permissions verified");
        }
        Err(e) => {
            log::error!("Cannot write to database directory: {e}");
            return Err(AppError::Config(format!(
                "No write permissions for database directory: {e}"
            )));
        }
    }

    let database_url = format!("sqlite:{}", db_path.display());
    log::info!("Connecting to database: {database_url}");

    // Try to create the file first if it doesn't exist
    if !db_path.exists() {
        log::info!(
            "Database file doesn't exist, creating: {}",
            db_path.display()
        );
        match std::fs::File::create(&db_path) {
            Ok(_) => {
                log::info!("Database file created successfully");
            }
            Err(e) => {
                log::error!("Failed to create database file: {e}");
                return Err(AppError::Config(format!(
                    "Cannot create database file: {e}"
                )));
            }
        }
    }

    // Try different connection approaches
    let connection_attempts = [
        format!("sqlite:{}", db_path.display()),
        format!(
            "sqlite:///{}",
            db_path.display().to_string().replace('\\', "/")
        ),
        format!("sqlite:{}", db_path.to_string_lossy()),
    ];

    let mut pool = None;
    let mut last_error = None;

    for (i, url) in connection_attempts.iter().enumerate() {
        log::info!("Connection attempt {}: {}", i + 1, url);
        match SqlitePool::connect(url).await {
            Ok(p) => {
                log::info!("Successfully connected with URL: {url}");
                pool = Some(p);
                break;
            }
            Err(e) => {
                log::warn!("Connection attempt {} failed: {}", i + 1, e);
                last_error = Some(e);
            }
        }
    }

    let pool = pool.ok_or_else(|| {
        let error_msg = format!(
            "Failed to connect to database after {} attempts. Last error: {}",
            connection_attempts.len(),
            last_error
                .map(|e| e.to_string())
                .unwrap_or_else(|| "Unknown error".to_string())
        );
        log::error!("{error_msg}");
        AppError::Config(error_msg)
    })?;

    // Create tables with better constraints and indexes
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS webhooks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            url TEXT NOT NULL UNIQUE,
            is_forum BOOLEAN NOT NULL DEFAULT FALSE,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            last_used_at DATETIME,
            use_count INTEGER DEFAULT 0
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // Create upload history table for analytics
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS upload_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_path TEXT NOT NULL,
            file_name TEXT NOT NULL,
            file_hash TEXT,
            file_size INTEGER,
            webhook_id INTEGER NOT NULL,
            upload_status TEXT NOT NULL DEFAULT 'success',
            error_message TEXT,
            uploaded_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            retry_count INTEGER DEFAULT 0,
            FOREIGN KEY (webhook_id) REFERENCES webhooks (id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // Create upload sessions table to track batch uploads
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS upload_sessions (
            id TEXT PRIMARY KEY,
            webhook_id INTEGER NOT NULL,
            total_files INTEGER NOT NULL,
            completed_files INTEGER DEFAULT 0,
            successful_uploads INTEGER DEFAULT 0,
            failed_uploads INTEGER DEFAULT 0,
            session_status TEXT NOT NULL DEFAULT 'active',
            started_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            completed_at DATETIME,
            FOREIGN KEY (webhook_id) REFERENCES webhooks (id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // Create table for user-specific webhook overrides
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS user_webhook_overrides (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT,
            user_display_name TEXT,
            webhook_id INTEGER NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (webhook_id) REFERENCES webhooks (id) ON DELETE CASCADE,
            UNIQUE(user_id, webhook_id),
            UNIQUE(user_display_name, webhook_id)
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // Create table for Discord user mappings (VRChat player → Discord @mention)
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS discord_user_mappings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            vrchat_display_name TEXT,
            vrchat_user_id TEXT,
            discord_user_id TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(vrchat_display_name),
            UNIQUE(vrchat_user_id)
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // Add indexes for better query performance
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_upload_history_hash ON upload_history(file_hash)")
        .execute(&pool)
        .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_upload_history_webhook ON upload_history(webhook_id)",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_upload_history_date ON upload_history(uploaded_at)",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_upload_history_status ON upload_history(upload_status)",
    )
    .execute(&pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_upload_history_path ON upload_history(file_path)")
        .execute(&pool)
        .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_upload_sessions_webhook ON upload_sessions(webhook_id)",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_upload_sessions_status ON upload_sessions(session_status)",
    )
    .execute(&pool)
    .await?;

    // Create triggers to update timestamps
    sqlx::query(
        r#"
        CREATE TRIGGER IF NOT EXISTS update_webhook_timestamp 
        AFTER UPDATE ON webhooks
        BEGIN
            UPDATE webhooks SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
        END
        "#,
    )
    .execute(&pool)
    .await?;

    DB_POOL
        .set(pool)
        .map_err(|_| AppError::Internal("Failed to set database pool".to_string()))?;

    // Run migrations after setting up the pool
    migrate_database().await?;

    log::info!("Database initialized successfully");
    Ok(())
}

pub async fn migrate_database() -> AppResult<()> {
    let pool = get_pool()?;

    // Check if upload_status column exists
    let column_check = sqlx::query(
        "SELECT name FROM pragma_table_info('upload_history') WHERE name = 'upload_status'",
    )
    .fetch_optional(pool)
    .await?;

    if column_check.is_none() {
        log::info!("Adding missing upload_status column to upload_history table");

        // Add the missing column
        sqlx::query(
            "ALTER TABLE upload_history ADD COLUMN upload_status TEXT NOT NULL DEFAULT 'success'",
        )
        .execute(pool)
        .await?;
    }

    // Check if error_message column exists
    let error_column_check = sqlx::query(
        "SELECT name FROM pragma_table_info('upload_history') WHERE name = 'error_message'",
    )
    .fetch_optional(pool)
    .await?;

    if error_column_check.is_none() {
        log::info!("Adding missing error_message column to upload_history table");

        sqlx::query("ALTER TABLE upload_history ADD COLUMN error_message TEXT")
            .execute(pool)
            .await?;
    }

    // Check if retry_count column exists
    let retry_column_check = sqlx::query(
        "SELECT name FROM pragma_table_info('upload_history') WHERE name = 'retry_count'",
    )
    .fetch_optional(pool)
    .await?;

    if retry_column_check.is_none() {
        log::info!("Adding missing retry_count column to upload_history table");

        sqlx::query("ALTER TABLE upload_history ADD COLUMN retry_count INTEGER DEFAULT 0")
            .execute(pool)
            .await?;
    }

    // Check if pinned column exists on webhooks table
    let pinned_column_check =
        sqlx::query("SELECT name FROM pragma_table_info('webhooks') WHERE name = 'pinned'")
            .fetch_optional(pool)
            .await?;

    if pinned_column_check.is_none() {
        log::info!("Adding pinned column to webhooks table");

        sqlx::query("ALTER TABLE webhooks ADD COLUMN pinned BOOLEAN NOT NULL DEFAULT FALSE")
            .execute(pool)
            .await?;
    }

    log::info!("Database migration completed successfully");
    Ok(())
}

fn get_pool() -> AppResult<&'static Pool<Sqlite>> {
    DB_POOL
        .get()
        .ok_or_else(|| AppError::Internal("Database not initialized".to_string()))
}

pub async fn get_all_webhooks() -> AppResult<Vec<Webhook>> {
    let pool = get_pool()?;

    let rows = sqlx::query(
        "SELECT id, name, url, is_forum, pinned FROM webhooks ORDER BY pinned DESC, last_used_at DESC, name ASC",
    )
    .fetch_all(pool)
    .await?;

    let mut webhooks = Vec::new();
    for row in rows {
        webhooks.push(Webhook {
            id: row.get("id"),
            name: row.get("name"),
            url: row.get("url"),
            is_forum: row.get("is_forum"),
            pinned: row.get("pinned"),
        });
    }

    Ok(webhooks)
}

pub async fn get_webhook_by_id(id: i64) -> AppResult<Webhook> {
    let pool = get_pool()?;

    let row = sqlx::query("SELECT id, name, url, is_forum, pinned FROM webhooks WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?;

    Ok(Webhook {
        id: row.get("id"),
        name: row.get("name"),
        url: row.get("url"),
        is_forum: row.get("is_forum"),
        pinned: row.get("pinned"),
    })
}

pub async fn insert_webhook(name: String, url: String, is_forum: bool) -> AppResult<i64> {
    let pool = get_pool()?;

    let result = sqlx::query("INSERT INTO webhooks (name, url, is_forum) VALUES (?, ?, ?)")
        .bind(name.clone())
        .bind(url.clone())
        .bind(is_forum)
        .execute(pool)
        .await;

    match result {
        Ok(result) => {
            let webhook_id = result.last_insert_rowid();
            log::info!("Added webhook: {name} (ID: {webhook_id})");
            Ok(webhook_id)
        }
        Err(sqlx::Error::Database(db_err))
            if db_err.code() == Some(std::borrow::Cow::Borrowed("2067")) =>
        {
            Err(AppError::validation(
                "url",
                "This webhook URL already exists. Each webhook URL can only be added once.",
            ))
        }
        Err(e) => Err(AppError::Database(e)),
    }
}

pub async fn update_webhook(id: i64, name: String, url: String, is_forum: bool) -> AppResult<()> {
    let pool = get_pool()?;

    sqlx::query("UPDATE webhooks SET name = ?, url = ?, is_forum = ? WHERE id = ?")
        .bind(name)
        .bind(url)
        .bind(is_forum)
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn delete_webhook(id: i64) -> AppResult<()> {
    let pool = get_pool()?;

    let result = sqlx::query("DELETE FROM webhooks WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::Database(sqlx::Error::RowNotFound));
    }

    log::info!("Deleted webhook with id: {id}");
    Ok(())
}

pub async fn toggle_webhook_pin(id: i64) -> AppResult<bool> {
    let pool = get_pool()?;

    let row = sqlx::query("SELECT pinned FROM webhooks WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?;

    let current: bool = row.get("pinned");
    let new_pinned = !current;

    sqlx::query("UPDATE webhooks SET pinned = ? WHERE id = ?")
        .bind(new_pinned)
        .bind(id)
        .execute(pool)
        .await?;

    log::info!("Toggled webhook {id} pinned: {current} -> {new_pinned}");
    Ok(new_pinned)
}

pub async fn update_webhook_usage(webhook_id: i64) -> AppResult<()> {
    let pool = get_pool()?;

    sqlx::query(
        "UPDATE webhooks SET last_used_at = CURRENT_TIMESTAMP, use_count = use_count + 1 WHERE id = ?"
    )
    .bind(webhook_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn record_upload(
    file_path: String,
    file_name: String,
    file_hash: Option<String>,
    file_size: Option<u64>,
    webhook_id: i64,
    status: &str,
    error_message: Option<String>,
) -> AppResult<()> {
    let pool = get_pool()?;

    sqlx::query(
        r#"
        INSERT INTO upload_history 
        (file_path, file_name, file_hash, file_size, webhook_id, upload_status, error_message) 
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(file_path)
    .bind(file_name)
    .bind(file_hash)
    .bind(file_size.map(|s| s as i64))
    .bind(webhook_id)
    .bind(status)
    .bind(error_message)
    .execute(pool)
    .await?;

    Ok(())
}

/// Upload session management
pub async fn create_upload_session(
    session_id: String,
    webhook_id: i64,
    total_files: i32,
) -> AppResult<()> {
    let pool = get_pool()?;

    sqlx::query("INSERT INTO upload_sessions (id, webhook_id, total_files) VALUES (?, ?, ?)")
        .bind(session_id)
        .bind(webhook_id)
        .bind(total_files)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn update_upload_session_progress(
    session_id: &str,
    completed_files: i32,
    successful_uploads: i32,
    failed_uploads: i32,
) -> AppResult<()> {
    let pool = get_pool()?;

    sqlx::query(
        r#"
        UPDATE upload_sessions 
        SET completed_files = ?, successful_uploads = ?, failed_uploads = ?, 
            completed_at = CASE WHEN ? >= total_files THEN CURRENT_TIMESTAMP ELSE completed_at END,
            session_status = CASE WHEN ? >= total_files THEN 'completed' ELSE 'active' END
        WHERE id = ?
        "#,
    )
    .bind(completed_files)
    .bind(successful_uploads)
    .bind(failed_uploads)
    .bind(completed_files)
    .bind(completed_files)
    .bind(session_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_upload_session_stats(session_id: &str) -> AppResult<Option<(i32, i32, i32, i32)>> {
    let pool = get_pool()?;

    let row = sqlx::query(
        "SELECT total_files, completed_files, successful_uploads, failed_uploads FROM upload_sessions WHERE id = ?"
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;

    if let Some(row) = row {
        Ok(Some((
            row.get("total_files"),
            row.get("completed_files"),
            row.get("successful_uploads"),
            row.get("failed_uploads"),
        )))
    } else {
        Ok(None)
    }
}

pub async fn cleanup_old_upload_sessions(days: i32) -> AppResult<u64> {
    let pool = get_pool()?;

    let result = sqlx::query(
        "DELETE FROM upload_sessions WHERE started_at < datetime('now', '-' || ? || ' days')",
    )
    .bind(days)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

pub async fn cleanup_old_upload_history(days: i32) -> AppResult<u64> {
    let pool = get_pool()?;

    let result = sqlx::query(
        "DELETE FROM upload_history WHERE uploaded_at < datetime('now', '-' || ? || ' days')",
    )
    .bind(days)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

// User Webhook Overrides
#[derive(Debug, serde::Serialize)]
pub struct UserWebhookOverride {
    pub id: i64,
    pub user_id: Option<String>,
    pub user_display_name: Option<String>,
    pub webhook_id: i64,
}

pub async fn get_user_webhook_overrides() -> AppResult<Vec<UserWebhookOverride>> {
    let pool = get_pool()?;

    let rows = sqlx::query(
        "SELECT id, user_id, user_display_name, webhook_id FROM user_webhook_overrides ORDER BY id DESC",
    )
    .fetch_all(pool)
    .await?;

    let mut overrides = Vec::new();
    for row in rows {
        overrides.push(UserWebhookOverride {
            id: row.get("id"),
            user_id: row.get("user_id"),
            user_display_name: row.get("user_display_name"),
            webhook_id: row.get("webhook_id"),
        });
    }

    Ok(overrides)
}

pub async fn add_user_webhook_override(
    user_id: Option<String>,
    user_display_name: Option<String>,
    webhook_id: i64,
) -> AppResult<i64> {
    let pool = get_pool()?;

    if user_id.is_none() && user_display_name.is_none() {
        return Err(AppError::validation(
            "user",
            "Must provide either User ID or User Display Name",
        ));
    }

    let result = sqlx::query(
        "INSERT INTO user_webhook_overrides (user_id, user_display_name, webhook_id) VALUES (?, ?, ?)",
    )
    .bind(user_id)
    .bind(user_display_name)
    .bind(webhook_id)
    .execute(pool)
    .await;

    match result {
        Ok(result) => Ok(result.last_insert_rowid()),
        Err(e) => Err(AppError::Database(e)),
    }
}

pub async fn delete_user_webhook_override(id: i64) -> AppResult<()> {
    let pool = get_pool()?;

    let result = sqlx::query("DELETE FROM user_webhook_overrides WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::Database(sqlx::Error::RowNotFound));
    }

    Ok(())
}

// Discord User Mappings (VRChat player → Discord @mention)
#[derive(Debug, serde::Serialize)]
pub struct DiscordUserMapping {
    pub id: i64,
    pub vrchat_display_name: Option<String>,
    pub vrchat_user_id: Option<String>,
    pub discord_user_id: String,
}

pub async fn get_discord_user_mappings() -> AppResult<Vec<DiscordUserMapping>> {
    let pool = get_pool()?;

    let rows = sqlx::query(
        "SELECT id, vrchat_display_name, vrchat_user_id, discord_user_id FROM discord_user_mappings ORDER BY id DESC",
    )
    .fetch_all(pool)
    .await?;

    let mut mappings = Vec::new();
    for row in rows {
        mappings.push(DiscordUserMapping {
            id: row.get("id"),
            vrchat_display_name: row.get("vrchat_display_name"),
            vrchat_user_id: row.get("vrchat_user_id"),
            discord_user_id: row.get("discord_user_id"),
        });
    }

    Ok(mappings)
}

pub async fn add_discord_user_mapping(
    vrchat_display_name: Option<String>,
    vrchat_user_id: Option<String>,
    discord_user_id: String,
) -> AppResult<i64> {
    let pool = get_pool()?;

    if vrchat_display_name.is_none() && vrchat_user_id.is_none() {
        return Err(AppError::validation(
            "user",
            "Must provide either VRChat Display Name or VRChat User ID",
        ));
    }

    if discord_user_id.is_empty() || !discord_user_id.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::validation(
            "discord_user_id",
            "Discord User ID must be a numeric ID",
        ));
    }

    let result = sqlx::query(
        "INSERT INTO discord_user_mappings (vrchat_display_name, vrchat_user_id, discord_user_id) VALUES (?, ?, ?)",
    )
    .bind(&vrchat_display_name)
    .bind(&vrchat_user_id)
    .bind(&discord_user_id)
    .execute(pool)
    .await;

    match result {
        Ok(result) => Ok(result.last_insert_rowid()),
        Err(e) => Err(AppError::Database(e)),
    }
}

pub async fn update_discord_user_mapping(
    id: i64,
    vrchat_display_name: Option<String>,
    vrchat_user_id: Option<String>,
    discord_user_id: String,
) -> AppResult<()> {
    let pool = get_pool()?;

    if vrchat_display_name.is_none() && vrchat_user_id.is_none() {
        return Err(AppError::validation(
            "user",
            "Must provide either VRChat Display Name or VRChat User ID",
        ));
    }

    if discord_user_id.is_empty() || !discord_user_id.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::validation(
            "discord_user_id",
            "Discord User ID must be a numeric ID",
        ));
    }

    let result = sqlx::query(
        "UPDATE discord_user_mappings SET vrchat_display_name = ?, vrchat_user_id = ?, discord_user_id = ? WHERE id = ?",
    )
    .bind(&vrchat_display_name)
    .bind(&vrchat_user_id)
    .bind(&discord_user_id)
    .bind(id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::Database(sqlx::Error::RowNotFound));
    }

    Ok(())
}

pub async fn delete_discord_user_mapping(id: i64) -> AppResult<()> {
    let pool = get_pool()?;

    let result = sqlx::query("DELETE FROM discord_user_mappings WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::Database(sqlx::Error::RowNotFound));
    }

    Ok(())
}

pub async fn is_file_processed(file_path: &str) -> AppResult<bool> {
    let pool = get_pool()?;
    let row = sqlx::query("SELECT COUNT(*) as count FROM upload_history WHERE file_path = ? AND upload_status = 'success'")
        .bind(file_path)
        .fetch_one(pool)
        .await?;

    let count: i32 = row.get("count");
    Ok(count > 0)
}
