//! Shared test utilities for generating PNG fixtures, metadata, temp files, and test databases.

use std::io::Write;
use std::path::PathBuf;

/// A temporary file that is automatically deleted when dropped.
pub struct TempFile {
    pub path: PathBuf,
}

impl TempFile {
    pub fn path_str(&self) -> String {
        self.path.to_string_lossy().to_string()
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Creates a valid 1x1 white PNG file as raw bytes.
pub fn create_minimal_png() -> Vec<u8> {
    create_test_png(1, 1)
}

/// Creates a visible test PNG (200x200) with a colorful gradient pattern.
/// Useful for Discord upload tests where you want to see the image.
pub fn create_visible_test_png() -> Vec<u8> {
    create_test_png(200, 200)
}

/// Creates a valid PNG of the given dimensions with a colorful gradient pattern.
fn create_test_png(width: u32, height: u32) -> Vec<u8> {
    let mut buf = Vec::new();

    // PNG signature
    buf.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    // IHDR chunk
    let mut ihdr_data = Vec::with_capacity(13);
    ihdr_data.extend_from_slice(&width.to_be_bytes());
    ihdr_data.extend_from_slice(&height.to_be_bytes());
    ihdr_data.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit RGB
    write_png_chunk(&mut buf, b"IHDR", &ihdr_data);

    // Generate scanlines with a colorful gradient
    let mut raw_data = Vec::with_capacity((height as usize) * (width as usize * 3 + 1));
    for y in 0..height {
        raw_data.push(0); // filter byte = None
        for x in 0..width {
            let r = ((x * 255) / width.max(1)) as u8;
            let g = ((y * 255) / height.max(1)) as u8;
            let b = (((x + y) * 128) / (width + height).max(1)) as u8;
            raw_data.push(r);
            raw_data.push(g);
            raw_data.push(b);
        }
    }

    let compressed = deflate_bytes(&raw_data);
    write_png_chunk(&mut buf, b"IDAT", &compressed);

    // IEND chunk
    write_png_chunk(&mut buf, b"IEND", &[]);

    buf
}

/// Creates a visible 200x200 PNG with VRCX-style metadata embedded in a tEXt Description chunk.
pub fn create_png_with_metadata(metadata_json: &str) -> Vec<u8> {
    let width: u32 = 200;
    let height: u32 = 200;
    let mut buf = Vec::new();

    // PNG signature
    buf.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    // IHDR chunk
    let mut ihdr_data = Vec::with_capacity(13);
    ihdr_data.extend_from_slice(&width.to_be_bytes());
    ihdr_data.extend_from_slice(&height.to_be_bytes());
    ihdr_data.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit RGB
    write_png_chunk(&mut buf, b"IHDR", &ihdr_data);

    // tEXt chunk with "Description" keyword + null separator + metadata JSON
    let mut text_data = Vec::new();
    text_data.extend_from_slice(b"Description");
    text_data.push(0); // null separator
    text_data.extend_from_slice(metadata_json.as_bytes());
    write_png_chunk(&mut buf, b"tEXt", &text_data);

    // Generate scanlines with a colorful gradient
    let mut raw_data = Vec::with_capacity((height as usize) * (width as usize * 3 + 1));
    for y in 0..height {
        raw_data.push(0); // filter byte = None
        for x in 0..width {
            let r = ((x * 255) / width) as u8;
            let g = ((y * 255) / height) as u8;
            let b = (((x + y) * 128) / (width + height)) as u8;
            raw_data.push(r);
            raw_data.push(g);
            raw_data.push(b);
        }
    }

    let compressed = deflate_bytes(&raw_data);
    write_png_chunk(&mut buf, b"IDAT", &compressed);

    // IEND chunk
    write_png_chunk(&mut buf, b"IEND", &[]);

    buf
}

/// Creates a VRCX-compatible metadata JSON string for testing.
pub fn create_test_metadata(
    world_id: &str,
    world_name: &str,
    players: &[&str],
    timestamp: i64,
) -> String {
    let player_array: Vec<String> = players
        .iter()
        .map(|name| {
            format!(
                r#"{{"displayName":"{}","id":"usr_{}"}}"#,
                name,
                name.to_lowercase().replace(' ', "_")
            )
        })
        .collect();

    format!(
        r#"{{"application":"VRCX","version":2,"author":{{"displayName":"TestUser","id":"usr_test123"}},"world":{{"name":"{}","id":"{}","instanceId":"12345~private(usr_test123)"}},"players":[{}],"created_at":"{}"}}"#,
        world_name,
        world_id,
        player_array.join(","),
        chrono::DateTime::from_timestamp(timestamp, 0)
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_else(|| "2024-01-15T14:30:00Z".to_string()),
    )
}

/// Creates a PNG file of approximately the given size by padding IDAT data.
pub fn create_png_of_size(approx_bytes: usize) -> Vec<u8> {
    // Minimum PNG overhead is ~60 bytes. Generate enough pixel data to hit target.
    let pixel_data_size = approx_bytes.saturating_sub(100).max(4);
    // Create a larger image to generate enough data
    let width = 100u32;
    let height = (pixel_data_size / (width as usize * 3 + 1)).max(1) as u32;

    let mut buf = Vec::new();

    // PNG signature
    buf.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    // IHDR chunk
    let mut ihdr_data = Vec::new();
    ihdr_data.extend_from_slice(&width.to_be_bytes());
    ihdr_data.extend_from_slice(&height.to_be_bytes());
    ihdr_data.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit RGB
    write_png_chunk(&mut buf, b"IHDR", &ihdr_data);

    // Generate scanlines with random-ish data (poor compression = larger output)
    let mut raw_data = Vec::new();
    for row in 0..height {
        raw_data.push(0); // filter byte = None
        for col in 0..(width * 3) {
            raw_data.push(((row * 7 + col * 13 + 42) % 256) as u8);
        }
    }

    let compressed = deflate_bytes(&raw_data);
    write_png_chunk(&mut buf, b"IDAT", &compressed);
    write_png_chunk(&mut buf, b"IEND", &[]);

    // If still too small, pad with an unrecognized safe-to-copy chunk
    while buf.len() < approx_bytes {
        let padding_size = (approx_bytes - buf.len()).min(65000);
        let padding = vec![0u8; padding_size];
        // Insert before IEND (last 12 bytes)
        let iend_start = buf.len() - 12;
        let iend = buf[iend_start..].to_vec();
        buf.truncate(iend_start);
        write_png_chunk(&mut buf, b"teXt", &padding); // lowercase = safe to copy
        buf.extend_from_slice(&iend);
    }

    buf
}

/// Writes PNG bytes to a temporary file that auto-deletes on drop.
pub fn create_temp_png(data: &[u8], name: &str) -> TempFile {
    let dir = std::env::temp_dir().join("vrchat_photo_uploader_tests");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(name);
    let mut file = std::fs::File::create(&path).expect("Failed to create temp PNG file");
    file.write_all(data).expect("Failed to write temp PNG data");
    TempFile { path }
}

/// Creates an in-memory SQLite database with the app's full schema.
pub async fn setup_test_db() -> sqlx::Pool<sqlx::Sqlite> {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory SQLite");

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

/// Reads the `DISCORD_WEBHOOK_URL` environment variable.
pub fn get_test_webhook_url() -> Option<String> {
    std::env::var("DISCORD_WEBHOOK_URL").ok()
}

/// Reads the `DISCORD_FORUM_WEBHOOK_URL` environment variable.
pub fn get_test_forum_webhook_url() -> Option<String> {
    std::env::var("DISCORD_FORUM_WEBHOOK_URL").ok()
}

// --- Internal helpers ---

fn write_png_chunk(buf: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
    let length = data.len() as u32;
    buf.extend_from_slice(&length.to_be_bytes());
    buf.extend_from_slice(chunk_type);
    buf.extend_from_slice(data);

    // CRC32 over chunk_type + data
    let crc = crc32(chunk_type, data);
    buf.extend_from_slice(&crc.to_be_bytes());
}

fn crc32(chunk_type: &[u8], data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in chunk_type.iter().chain(data.iter()) {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    crc ^ 0xFFFF_FFFF
}

fn deflate_bytes(data: &[u8]) -> Vec<u8> {
    use flate2::write::ZlibEncoder;
    use flate2::Compression;

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_minimal_png_is_valid() {
        let png = create_minimal_png();
        // Check PNG signature
        assert_eq!(&png[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
        // Should be parseable by the image crate
        let img = image::load_from_memory(&png).expect("Should parse as valid PNG");
        assert_eq!(img.width(), 1);
        assert_eq!(img.height(), 1);
    }

    #[test]
    fn test_create_png_with_metadata_contains_description() {
        let metadata = r#"{"application":"VRCX","version":2}"#;
        let png = create_png_with_metadata(metadata);
        // Should contain the metadata string in the raw bytes
        let png_str = String::from_utf8_lossy(&png);
        assert!(png_str.contains("Description"));
        assert!(png_str.contains("VRCX"));
        // Should still be a valid PNG
        let img = image::load_from_memory(&png).expect("Should parse as valid PNG");
        assert_eq!(img.width(), 200);
        assert_eq!(img.height(), 200);
    }

    #[test]
    fn test_create_visible_test_png_is_200x200() {
        let png = create_visible_test_png();
        let img = image::load_from_memory(&png).expect("Should parse as valid PNG");
        assert_eq!(img.width(), 200);
        assert_eq!(img.height(), 200);
    }

    #[test]
    fn test_create_test_metadata_produces_valid_json() {
        let json = create_test_metadata("wrld_test123", "Test World", &["Alice", "Bob"], 1705312200);
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("Should be valid JSON");
        assert_eq!(parsed["world"]["id"], "wrld_test123");
        assert_eq!(parsed["world"]["name"], "Test World");
        assert_eq!(parsed["players"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_create_png_of_size_approximately_correct() {
        let target = 10_000;
        let png = create_png_of_size(target);
        // Allow 50% tolerance since compression is unpredictable
        assert!(
            png.len() >= target / 2,
            "PNG too small: {} vs target {}",
            png.len(),
            target
        );
    }

    #[test]
    fn test_create_temp_png_creates_and_deletes() {
        let data = create_minimal_png();
        let path;
        {
            let tmp = create_temp_png(&data, "test_delete.png");
            path = tmp.path.clone();
            assert!(path.exists());
        }
        // After drop, file should be deleted
        assert!(!path.exists());
    }
}
