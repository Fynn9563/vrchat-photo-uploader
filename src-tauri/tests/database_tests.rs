//! Database integration tests using in-memory SQLite.
//! Tests SQL schema, queries, and constraints directly against the pool.

use sqlx::{Pool, Row, Sqlite};

/// Helper to create an in-memory database with the app's schema.
async fn setup_db() -> Pool<Sqlite> {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory SQLite");

    // Create all tables (same schema as database.rs)
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
    .await
    .unwrap();

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
    .await
    .unwrap();

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
    .await
    .unwrap();

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
    .await
    .unwrap();

    pool
}

/// Insert a webhook and return its ID
async fn insert_webhook(pool: &Pool<Sqlite>, name: &str, url: &str, is_forum: bool) -> i64 {
    let result = sqlx::query("INSERT INTO webhooks (name, url, is_forum) VALUES (?, ?, ?)")
        .bind(name)
        .bind(url)
        .bind(is_forum)
        .execute(pool)
        .await
        .unwrap();
    result.last_insert_rowid()
}

#[tokio::test]
async fn test_insert_webhook_and_get_all() {
    let pool = setup_db().await;
    let id = insert_webhook(&pool, "Test Hook", "https://discord.com/api/webhooks/1/abc", false).await;
    assert!(id > 0);

    let rows = sqlx::query("SELECT id, name, url, is_forum FROM webhooks ORDER BY name ASC")
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<String, _>("name"), "Test Hook");
    assert_eq!(rows[0].get::<String, _>("url"), "https://discord.com/api/webhooks/1/abc");
    assert!(!rows[0].get::<bool, _>("is_forum"));
}

#[tokio::test]
async fn test_insert_duplicate_url_fails() {
    let pool = setup_db().await;
    insert_webhook(&pool, "Hook 1", "https://discord.com/api/webhooks/1/abc", false).await;

    let result = sqlx::query("INSERT INTO webhooks (name, url, is_forum) VALUES (?, ?, ?)")
        .bind("Hook 2")
        .bind("https://discord.com/api/webhooks/1/abc") // same URL
        .bind(false)
        .execute(&pool)
        .await;

    assert!(result.is_err(), "Duplicate URL should fail with UNIQUE constraint");
}

#[tokio::test]
async fn test_insert_duplicate_name_fails() {
    let pool = setup_db().await;
    insert_webhook(&pool, "Same Name", "https://discord.com/api/webhooks/1/abc", false).await;

    let result = sqlx::query("INSERT INTO webhooks (name, url, is_forum) VALUES (?, ?, ?)")
        .bind("Same Name") // same name
        .bind("https://discord.com/api/webhooks/2/def")
        .bind(false)
        .execute(&pool)
        .await;

    assert!(result.is_err(), "Duplicate name should fail with UNIQUE constraint");
}

#[tokio::test]
async fn test_get_webhook_by_id() {
    let pool = setup_db().await;
    let id = insert_webhook(&pool, "Find Me", "https://discord.com/api/webhooks/1/abc", true).await;

    let row = sqlx::query("SELECT id, name, url, is_forum FROM webhooks WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(row.get::<String, _>("name"), "Find Me");
    assert!(row.get::<bool, _>("is_forum"));
}

#[tokio::test]
async fn test_get_webhook_by_id_not_found() {
    let pool = setup_db().await;

    let result = sqlx::query("SELECT id, name, url, is_forum FROM webhooks WHERE id = ?")
        .bind(9999i64)
        .fetch_optional(&pool)
        .await
        .unwrap();

    assert!(result.is_none());
}

#[tokio::test]
async fn test_update_webhook() {
    let pool = setup_db().await;
    let id = insert_webhook(&pool, "Original", "https://discord.com/api/webhooks/1/abc", false).await;

    sqlx::query("UPDATE webhooks SET name = ?, url = ?, is_forum = ? WHERE id = ?")
        .bind("Updated")
        .bind("https://discord.com/api/webhooks/2/def")
        .bind(true)
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();

    let row = sqlx::query("SELECT name, url, is_forum FROM webhooks WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(row.get::<String, _>("name"), "Updated");
    assert_eq!(row.get::<String, _>("url"), "https://discord.com/api/webhooks/2/def");
    assert!(row.get::<bool, _>("is_forum"));
}

#[tokio::test]
async fn test_delete_webhook() {
    let pool = setup_db().await;
    let id = insert_webhook(&pool, "Delete Me", "https://discord.com/api/webhooks/1/abc", false).await;

    let result = sqlx::query("DELETE FROM webhooks WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(result.rows_affected(), 1);

    let rows = sqlx::query("SELECT id FROM webhooks")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(rows.is_empty());
}

#[tokio::test]
async fn test_update_webhook_usage() {
    let pool = setup_db().await;
    let id = insert_webhook(&pool, "Used Hook", "https://discord.com/api/webhooks/1/abc", false).await;

    // Update usage twice
    for _ in 0..2 {
        sqlx::query(
            "UPDATE webhooks SET last_used_at = CURRENT_TIMESTAMP, use_count = use_count + 1 WHERE id = ?"
        )
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    }

    let row = sqlx::query("SELECT use_count, last_used_at FROM webhooks WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(row.get::<i32, _>("use_count"), 2);
    assert!(row.get::<Option<String>, _>("last_used_at").is_some());
}

#[tokio::test]
async fn test_record_upload_success() {
    let pool = setup_db().await;
    let webhook_id = insert_webhook(&pool, "Hook", "https://discord.com/api/webhooks/1/abc", false).await;

    sqlx::query(
        r#"INSERT INTO upload_history
        (file_path, file_name, file_hash, file_size, webhook_id, upload_status, error_message)
        VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind("/photos/test.png")
    .bind("test.png")
    .bind("abc123")
    .bind(1024i64)
    .bind(webhook_id)
    .bind("success")
    .bind(None::<String>)
    .execute(&pool)
    .await
    .unwrap();

    let row = sqlx::query("SELECT file_path, upload_status FROM upload_history WHERE webhook_id = ?")
        .bind(webhook_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(row.get::<String, _>("file_path"), "/photos/test.png");
    assert_eq!(row.get::<String, _>("upload_status"), "success");
}

#[tokio::test]
async fn test_record_upload_failure() {
    let pool = setup_db().await;
    let webhook_id = insert_webhook(&pool, "Hook", "https://discord.com/api/webhooks/1/abc", false).await;

    sqlx::query(
        r#"INSERT INTO upload_history
        (file_path, file_name, webhook_id, upload_status, error_message)
        VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind("/photos/fail.png")
    .bind("fail.png")
    .bind(webhook_id)
    .bind("failed")
    .bind("Network timeout")
    .execute(&pool)
    .await
    .unwrap();

    let row = sqlx::query("SELECT upload_status, error_message FROM upload_history WHERE file_name = ?")
        .bind("fail.png")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(row.get::<String, _>("upload_status"), "failed");
    assert_eq!(row.get::<String, _>("error_message"), "Network timeout");
}

#[tokio::test]
async fn test_create_upload_session() {
    let pool = setup_db().await;
    let webhook_id = insert_webhook(&pool, "Hook", "https://discord.com/api/webhooks/1/abc", false).await;

    sqlx::query("INSERT INTO upload_sessions (id, webhook_id, total_files) VALUES (?, ?, ?)")
        .bind("session_001")
        .bind(webhook_id)
        .bind(10i32)
        .execute(&pool)
        .await
        .unwrap();

    let row = sqlx::query("SELECT id, total_files, session_status FROM upload_sessions WHERE id = ?")
        .bind("session_001")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(row.get::<String, _>("id"), "session_001");
    assert_eq!(row.get::<i32, _>("total_files"), 10);
    assert_eq!(row.get::<String, _>("session_status"), "active");
}

#[tokio::test]
async fn test_update_upload_session_progress() {
    let pool = setup_db().await;
    let webhook_id = insert_webhook(&pool, "Hook", "https://discord.com/api/webhooks/1/abc", false).await;

    sqlx::query("INSERT INTO upload_sessions (id, webhook_id, total_files) VALUES (?, ?, ?)")
        .bind("session_002")
        .bind(webhook_id)
        .bind(5i32)
        .execute(&pool)
        .await
        .unwrap();

    // Update progress to 3/5
    sqlx::query(
        r#"UPDATE upload_sessions
        SET completed_files = ?, successful_uploads = ?, failed_uploads = ?,
            completed_at = CASE WHEN ? >= total_files THEN CURRENT_TIMESTAMP ELSE completed_at END,
            session_status = CASE WHEN ? >= total_files THEN 'completed' ELSE 'active' END
        WHERE id = ?"#,
    )
    .bind(3i32)
    .bind(2i32)
    .bind(1i32)
    .bind(3i32)
    .bind(3i32)
    .bind("session_002")
    .execute(&pool)
    .await
    .unwrap();

    let row = sqlx::query("SELECT completed_files, successful_uploads, failed_uploads, session_status FROM upload_sessions WHERE id = ?")
        .bind("session_002")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(row.get::<i32, _>("completed_files"), 3);
    assert_eq!(row.get::<i32, _>("successful_uploads"), 2);
    assert_eq!(row.get::<i32, _>("failed_uploads"), 1);
    assert_eq!(row.get::<String, _>("session_status"), "active");
}

#[tokio::test]
async fn test_session_completes_when_all_files_done() {
    let pool = setup_db().await;
    let webhook_id = insert_webhook(&pool, "Hook", "https://discord.com/api/webhooks/1/abc", false).await;

    sqlx::query("INSERT INTO upload_sessions (id, webhook_id, total_files) VALUES (?, ?, ?)")
        .bind("session_complete")
        .bind(webhook_id)
        .bind(2i32)
        .execute(&pool)
        .await
        .unwrap();

    // Complete all files
    sqlx::query(
        r#"UPDATE upload_sessions
        SET completed_files = ?, successful_uploads = ?, failed_uploads = ?,
            completed_at = CASE WHEN ? >= total_files THEN CURRENT_TIMESTAMP ELSE completed_at END,
            session_status = CASE WHEN ? >= total_files THEN 'completed' ELSE 'active' END
        WHERE id = ?"#,
    )
    .bind(2i32)
    .bind(2i32)
    .bind(0i32)
    .bind(2i32)
    .bind(2i32)
    .bind("session_complete")
    .execute(&pool)
    .await
    .unwrap();

    let row = sqlx::query("SELECT session_status, completed_at FROM upload_sessions WHERE id = ?")
        .bind("session_complete")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(row.get::<String, _>("session_status"), "completed");
    assert!(row.get::<Option<String>, _>("completed_at").is_some());
}

#[tokio::test]
async fn test_is_file_processed() {
    let pool = setup_db().await;
    let webhook_id = insert_webhook(&pool, "Hook", "https://discord.com/api/webhooks/1/abc", false).await;

    // File not processed yet
    let row = sqlx::query("SELECT COUNT(*) as count FROM upload_history WHERE file_path = ? AND upload_status = 'success'")
        .bind("/photos/new.png")
        .fetch_one(&pool)
        .await
        .unwrap();
    let count: i32 = row.get("count");
    assert_eq!(count, 0);

    // Record successful upload
    sqlx::query(
        "INSERT INTO upload_history (file_path, file_name, webhook_id, upload_status) VALUES (?, ?, ?, ?)",
    )
    .bind("/photos/new.png")
    .bind("new.png")
    .bind(webhook_id)
    .bind("success")
    .execute(&pool)
    .await
    .unwrap();

    // Now file should be processed
    let row = sqlx::query("SELECT COUNT(*) as count FROM upload_history WHERE file_path = ? AND upload_status = 'success'")
        .bind("/photos/new.png")
        .fetch_one(&pool)
        .await
        .unwrap();
    let count: i32 = row.get("count");
    assert!(count > 0);
}

#[tokio::test]
async fn test_user_webhook_override_crud() {
    let pool = setup_db().await;
    let webhook_id = insert_webhook(&pool, "Hook", "https://discord.com/api/webhooks/1/abc", false).await;

    // Add override
    let result = sqlx::query(
        "INSERT INTO user_webhook_overrides (user_id, user_display_name, webhook_id) VALUES (?, ?, ?)",
    )
    .bind("usr_test123")
    .bind("TestUser")
    .bind(webhook_id)
    .execute(&pool)
    .await
    .unwrap();

    let override_id = result.last_insert_rowid();
    assert!(override_id > 0);

    // Get overrides
    let rows = sqlx::query(
        "SELECT id, user_id, user_display_name, webhook_id FROM user_webhook_overrides ORDER BY id DESC",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<Option<String>, _>("user_id"), Some("usr_test123".to_string()));
    assert_eq!(rows[0].get::<Option<String>, _>("user_display_name"), Some("TestUser".to_string()));

    // Delete override
    let result = sqlx::query("DELETE FROM user_webhook_overrides WHERE id = ?")
        .bind(override_id)
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(result.rows_affected(), 1);

    let rows = sqlx::query("SELECT id FROM user_webhook_overrides")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(rows.is_empty());
}

#[tokio::test]
async fn test_multiple_webhooks_ordering() {
    let pool = setup_db().await;
    insert_webhook(&pool, "Zebra", "https://discord.com/api/webhooks/1/a", false).await;
    insert_webhook(&pool, "Alpha", "https://discord.com/api/webhooks/2/b", false).await;
    insert_webhook(&pool, "Middle", "https://discord.com/api/webhooks/3/c", false).await;

    let rows = sqlx::query(
        "SELECT name FROM webhooks ORDER BY last_used_at DESC, name ASC",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    // All have NULL last_used_at, so they should be ordered by name ASC
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].get::<String, _>("name"), "Alpha");
    assert_eq!(rows[1].get::<String, _>("name"), "Middle");
    assert_eq!(rows[2].get::<String, _>("name"), "Zebra");
}
