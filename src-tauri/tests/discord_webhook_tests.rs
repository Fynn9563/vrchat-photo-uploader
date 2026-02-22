//! Integration tests for Discord webhook functionality.
//!
//! These tests require real Discord webhook URLs and are marked `#[ignore]`.
//! Run them with:
//!   cargo test --test discord_webhook_tests -- --ignored
//!
//! Required environment variables:
//!   DISCORD_WEBHOOK_URL       - a webhook pointing to a normal text channel
//!   DISCORD_FORUM_WEBHOOK_URL - a webhook pointing to a forum channel
//!
//! You can set these via shell exports or a `.env` file in the `src-tauri/` directory.

use std::sync::Once;

use serial_test::serial;

use VRChat_Photo_Uploader::commands::{PlayerInfo, WorldInfo};
use VRChat_Photo_Uploader::image_processor;
use VRChat_Photo_Uploader::test_helpers::{
    create_png_with_metadata, create_temp_png, create_test_metadata, create_visible_test_png,
};
use VRChat_Photo_Uploader::uploader::discord_client::{
    extract_thread_id, DiscordClient, UploadPayload,
};
use VRChat_Photo_Uploader::uploader::image_groups::create_discord_payload;

// ---------------------------------------------------------------------------
// Load .env file once (safe to call from multiple tests)
// ---------------------------------------------------------------------------

static INIT_ENV: Once = Once::new();

fn load_env() {
    INIT_ENV.call_once(|| {
        dotenvy::dotenv().ok();
    });
}

// ---------------------------------------------------------------------------
// Helper: construct WorldInfo / PlayerInfo without needing Serialize
// ---------------------------------------------------------------------------

fn make_world(name: &str, id: &str) -> WorldInfo {
    WorldInfo {
        name: name.to_string(),
        id: id.to_string(),
        instance_id: String::new(),
    }
}

fn make_player(name: &str) -> PlayerInfo {
    PlayerInfo {
        display_name: name.to_string(),
        id: format!("usr_{}", name.to_lowercase().replace(' ', "_")),
    }
}

// ===========================================================================
// Normal channel tests
// ===========================================================================

/// Test 1 - Send a text-only message to a normal channel webhook.
#[tokio::test]
#[ignore]
#[serial]
async fn test_send_text_message() {
    load_env();
    let webhook_url = match std::env::var("DISCORD_WEBHOOK_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("Skipping: DISCORD_WEBHOOK_URL not set");
            return;
        }
    };

    let client = DiscordClient::new();
    let result = client
        .send_text_message(&webhook_url, "Integration test: text-only message", None)
        .await;
    assert!(
        result.is_ok(),
        "Failed to send text message: {:?}",
        result.err()
    );
}

/// Test 2 - Upload a single PNG image to a normal channel webhook.
#[tokio::test]
#[ignore]
#[serial]
async fn test_send_single_image() {
    load_env();
    let webhook_url = match std::env::var("DISCORD_WEBHOOK_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("Skipping: DISCORD_WEBHOOK_URL not set");
            return;
        }
    };

    let png_data = create_visible_test_png();
    let temp_file = create_temp_png(&png_data, "single_image_test.png");

    let mut payload = UploadPayload::new();
    payload.add_text_field(
        "content".to_string(),
        "[Test 2] Single image upload (PNG 200x200)".to_string(),
    );
    payload
        .add_file(&temp_file.path_str(), "file0".to_string())
        .await
        .expect("Failed to add file to payload");

    let client = DiscordClient::new();
    let result = client
        .send_webhook_with_thread_id(&webhook_url, &payload, None)
        .await;
    assert!(
        result.is_ok(),
        "Failed to send single image: {:?}",
        result.err()
    );
}

/// Test 3 - Upload three PNG images in a single payload to a normal channel webhook.
#[tokio::test]
#[ignore]
#[serial]
async fn test_send_multiple_images() {
    load_env();
    let webhook_url = match std::env::var("DISCORD_WEBHOOK_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("Skipping: DISCORD_WEBHOOK_URL not set");
            return;
        }
    };

    let png_data = create_visible_test_png();
    let temp_files = [
        create_temp_png(&png_data, "multi_image_test_0.png"),
        create_temp_png(&png_data, "multi_image_test_1.png"),
        create_temp_png(&png_data, "multi_image_test_2.png"),
    ];

    let mut payload = UploadPayload::new();
    payload.add_text_field(
        "content".to_string(),
        "[Test 3] Multiple images upload (3x PNG 200x200)".to_string(),
    );
    for (i, temp_file) in temp_files.iter().enumerate() {
        payload
            .add_file(&temp_file.path_str(), format!("file{i}"))
            .await
            .expect("Failed to add file to payload");
    }

    let client = DiscordClient::new();
    let result = client
        .send_webhook_with_thread_id(&webhook_url, &payload, None)
        .await;
    assert!(
        result.is_ok(),
        "Failed to send multiple images: {:?}",
        result.err()
    );
}

/// Test 4 - Upload a PNG that contains VRCX metadata and include a text payload
///          built by `create_discord_payload`.
#[tokio::test]
#[ignore]
#[serial]
async fn test_send_image_with_metadata_message() {
    load_env();
    let webhook_url = match std::env::var("DISCORD_WEBHOOK_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("Skipping: DISCORD_WEBHOOK_URL not set");
            return;
        }
    };

    let metadata_json = create_test_metadata(
        "wrld_integration_test",
        "Integration Test World",
        &["Alice", "Bob"],
        1705312200,
    );
    let png_data = create_png_with_metadata(&metadata_json);
    let temp_file = create_temp_png(&png_data, "metadata_image_test.png");

    let worlds = vec![make_world(
        "Integration Test World",
        "wrld_integration_test",
    )];
    let players = vec![make_player("Alice"), make_player("Bob")];

    let no_mappings = std::collections::HashMap::new();
    let (text_fields, _overflow) = create_discord_payload(
        &worlds,
        &players,
        Some(1705312200),
        true,  // is_first_message
        0,     // chunk_index
        false, // is_forum_post
        None,  // thread_id
        true,  // include_player_names
        1,     // image_count
        &no_mappings,
    );

    let mut payload = UploadPayload::new();
    for (key, value) in &text_fields {
        payload.add_text_field(key.clone(), value.clone());
    }
    payload
        .add_file(&temp_file.path_str(), "file0".to_string())
        .await
        .expect("Failed to add file to payload");

    let client = DiscordClient::new();
    let result = client
        .send_webhook_with_thread_id(&webhook_url, &payload, None)
        .await;
    assert!(
        result.is_ok(),
        "Failed to send image with metadata message: {:?}",
        result.err()
    );
}

/// Test 5 - Build a payload with a large player list via `create_discord_payload`
///          and verify the message (plus any overflow) can be sent.
#[tokio::test]
#[ignore]
#[serial]
async fn test_send_message_with_player_list() {
    load_env();
    let webhook_url = match std::env::var("DISCORD_WEBHOOK_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("Skipping: DISCORD_WEBHOOK_URL not set");
            return;
        }
    };

    let worlds = vec![make_world(
        "Player List Test World",
        "wrld_player_list_test",
    )];
    let players: Vec<PlayerInfo> = (0..30)
        .map(|i| make_player(&format!("TestPlayer_{i:02}")))
        .collect();

    let no_mappings = std::collections::HashMap::new();
    let (text_fields, overflow_messages) = create_discord_payload(
        &worlds,
        &players,
        Some(1705312200),
        true,  // is_first_message
        0,     // chunk_index
        false, // is_forum_post
        None,  // thread_id
        true,  // include_player_names
        3,     // image_count
        &no_mappings,
    );

    let client = DiscordClient::new();

    // Send the main message (text-only, no images needed for this test)
    let content = text_fields
        .get("content")
        .expect("Payload should contain 'content' field");
    let result = client.send_text_message(&webhook_url, content, None).await;
    assert!(
        result.is_ok(),
        "Failed to send main player list message: {:?}",
        result.err()
    );

    // Send any overflow messages
    for (i, overflow_msg) in overflow_messages.iter().enumerate() {
        let result = client
            .send_text_message(&webhook_url, overflow_msg, None)
            .await;
        assert!(
            result.is_ok(),
            "Failed to send overflow message {}: {:?}",
            i,
            result.err()
        );
    }
}

// ===========================================================================
// Forum channel tests
// ===========================================================================

/// Test 6 - Create a new forum thread using `send_forum_text_message` and
///          extract the thread_id from the response.
#[tokio::test]
#[ignore]
#[serial]
async fn test_forum_create_thread() {
    load_env();
    let webhook_url = match std::env::var("DISCORD_FORUM_WEBHOOK_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("Skipping: DISCORD_FORUM_WEBHOOK_URL not set");
            return;
        }
    };

    let client = DiscordClient::new();
    let result = client
        .send_forum_text_message(
            &webhook_url,
            "Integration test: creating a forum thread",
            Some("Test Thread"),
        )
        .await;
    assert!(
        result.is_ok(),
        "Failed to create forum thread: {:?}",
        result.err()
    );

    let response = result.unwrap();
    let thread_id = extract_thread_id(&response);
    assert!(
        thread_id.is_some(),
        "Should be able to extract thread_id from forum response. Response: {}",
        &response[..std::cmp::min(300, response.len())]
    );
}

/// Test 7 - Create a forum thread, then upload an image into that thread.
#[tokio::test]
#[ignore]
#[serial]
async fn test_forum_upload_image_to_thread() {
    load_env();
    let webhook_url = match std::env::var("DISCORD_FORUM_WEBHOOK_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("Skipping: DISCORD_FORUM_WEBHOOK_URL not set");
            return;
        }
    };

    let client = DiscordClient::new();

    // Step 1: Create thread
    let thread_response = client
        .send_forum_text_message(
            &webhook_url,
            "Integration test: forum image upload thread",
            Some("Image Upload Test"),
        )
        .await
        .expect("Failed to create forum thread");

    let thread_id = extract_thread_id(&thread_response)
        .expect("Failed to extract thread_id from forum response");

    // Step 2: Upload image into the thread
    let png_data = create_visible_test_png();
    let temp_file = create_temp_png(&png_data, "forum_image_test.png");

    let mut payload = UploadPayload::new();
    payload.add_text_field(
        "content".to_string(),
        "[Test 7] Forum thread image upload".to_string(),
    );
    payload
        .add_file(&temp_file.path_str(), "file0".to_string())
        .await
        .expect("Failed to add file to payload");

    let result = client
        .send_webhook_with_thread_id(&webhook_url, &payload, Some(&thread_id))
        .await;
    assert!(
        result.is_ok(),
        "Failed to upload image to forum thread: {:?}",
        result.err()
    );
}

/// Test 8 - Full forum workflow: create thread, extract thread_id, upload two
///          images, then send an overflow text message into the same thread.
#[tokio::test]
#[ignore]
#[serial]
async fn test_forum_full_workflow() {
    load_env();
    let webhook_url = match std::env::var("DISCORD_FORUM_WEBHOOK_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("Skipping: DISCORD_FORUM_WEBHOOK_URL not set");
            return;
        }
    };

    let client = DiscordClient::new();

    // Step 1: Create forum thread
    let thread_response = client
        .send_forum_text_message(
            &webhook_url,
            "Integration test: full forum workflow",
            Some("Full Workflow Test"),
        )
        .await
        .expect("Failed to create forum thread for full workflow");

    let thread_id =
        extract_thread_id(&thread_response).expect("Failed to extract thread_id for full workflow");

    // Step 2: Upload first image
    let png_data = create_visible_test_png();
    let temp_file_1 = create_temp_png(&png_data, "forum_workflow_1.png");
    let temp_file_2 = create_temp_png(&png_data, "forum_workflow_2.png");

    let mut payload1 = UploadPayload::new();
    payload1.add_text_field(
        "content".to_string(),
        "[Test 8] Forum workflow - image 1/2".to_string(),
    );
    payload1
        .add_file(&temp_file_1.path_str(), "file0".to_string())
        .await
        .expect("Failed to add first file");

    let result1 = client
        .send_webhook_with_thread_id(&webhook_url, &payload1, Some(&thread_id))
        .await;
    assert!(
        result1.is_ok(),
        "Failed to upload first image in forum workflow: {:?}",
        result1.err()
    );

    // Step 3: Upload second image
    let mut payload2 = UploadPayload::new();
    payload2.add_text_field(
        "content".to_string(),
        "[Test 8] Forum workflow - image 2/2".to_string(),
    );
    payload2
        .add_file(&temp_file_2.path_str(), "file0".to_string())
        .await
        .expect("Failed to add second file");

    let result2 = client
        .send_webhook_with_thread_id(&webhook_url, &payload2, Some(&thread_id))
        .await;
    assert!(
        result2.is_ok(),
        "Failed to upload second image in forum workflow: {:?}",
        result2.err()
    );

    // Step 4: Send overflow text message into the thread
    let result3 = client
        .send_text_message(
            &webhook_url,
            "Integration test: overflow text in forum thread",
            Some(&thread_id),
        )
        .await;
    assert!(
        result3.is_ok(),
        "Failed to send overflow text in forum thread: {:?}",
        result3.err()
    );
}

// ===========================================================================
// Error handling tests
// ===========================================================================

/// Test 9 - Sending to a known-invalid webhook URL should produce an error.
#[tokio::test]
#[ignore]
#[serial]
async fn test_invalid_webhook_url() {
    load_env();
    // No env var needed - we intentionally use a bogus URL.
    let invalid_url = "https://discord.com/api/webhooks/0/invalid";

    let client = DiscordClient::new();
    let result = client
        .send_text_message(invalid_url, "This should fail", None)
        .await;
    assert!(
        result.is_err(),
        "Sending to an invalid webhook should return an error"
    );
}

// ===========================================================================
// Rate limiting tests
// ===========================================================================

/// Test 10 - Send three text messages in rapid succession and verify the
///           built-in rate limiter prevents 429 errors.
#[tokio::test]
#[ignore]
#[serial]
async fn test_rapid_requests_handled() {
    load_env();
    let webhook_url = match std::env::var("DISCORD_WEBHOOK_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("Skipping: DISCORD_WEBHOOK_URL not set");
            return;
        }
    };

    let client = DiscordClient::new();

    for i in 0..3 {
        let result = client
            .send_text_message(
                &webhook_url,
                &format!("Integration test: rapid request {}/3", i + 1),
                None,
            )
            .await;
        assert!(
            result.is_ok(),
            "Rapid request {} failed (possible 429): {:?}",
            i + 1,
            result.err()
        );
    }
}

// ===========================================================================
// Compression → Upload tests
// ===========================================================================

/// Helper: compress a PNG with the given format, then upload the result to Discord.
async fn compress_and_upload(webhook_url: &str, format: &str, test_name: &str) {
    let png_data = create_visible_test_png();
    let tmp = create_temp_png(&png_data, &format!("compress_{format}_upload.png"));

    let compressed_path =
        image_processor::compress_image_with_format(&tmp.path_str(), 80, format, None)
            .await
            .unwrap_or_else(|e| panic!("{test_name}: compression to {format} failed: {e:?}"));

    let compressed_size = std::fs::metadata(&compressed_path)
        .map(|m| m.len())
        .unwrap_or(0);

    let mut payload = UploadPayload::new();
    payload.add_text_field(
        "content".to_string(),
        format!(
            "[Compression Test] Format: **{format}** | Quality: 80 | Size: {compressed_size} bytes"
        ),
    );
    payload
        .add_file(&compressed_path, "file0".to_string())
        .await
        .unwrap_or_else(|e| panic!("{test_name}: failed to add compressed file: {e:?}"));

    let client = DiscordClient::new();
    let result = client
        .send_webhook_with_thread_id(webhook_url, &payload, None)
        .await;
    assert!(
        result.is_ok(),
        "{test_name}: upload of {format} file failed: {:?}",
        result.err()
    );

    // Cleanup compressed output
    let _ = std::fs::remove_file(&compressed_path);
}

/// Test 11 - Compress to lossy WebP, then upload.
#[tokio::test]
#[ignore]
#[serial]
async fn test_upload_compressed_webp() {
    load_env();
    let webhook_url = match std::env::var("DISCORD_WEBHOOK_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("Skipping: DISCORD_WEBHOOK_URL not set");
            return;
        }
    };
    compress_and_upload(&webhook_url, "webp", "test_upload_compressed_webp").await;
}

/// Test 12 - Compress to lossless WebP, then upload.
#[tokio::test]
#[ignore]
#[serial]
async fn test_upload_compressed_lossless_webp() {
    load_env();
    let webhook_url = match std::env::var("DISCORD_WEBHOOK_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("Skipping: DISCORD_WEBHOOK_URL not set");
            return;
        }
    };
    compress_and_upload(
        &webhook_url,
        "lossless_webp",
        "test_upload_compressed_lossless_webp",
    )
    .await;
}

/// Test 13 - Compress to JPEG, then upload.
#[tokio::test]
#[ignore]
#[serial]
async fn test_upload_compressed_jpg() {
    load_env();
    let webhook_url = match std::env::var("DISCORD_WEBHOOK_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("Skipping: DISCORD_WEBHOOK_URL not set");
            return;
        }
    };
    compress_and_upload(&webhook_url, "jpg", "test_upload_compressed_jpg").await;
}

/// Test 14 - Compress to AVIF, then upload.
#[tokio::test]
#[ignore]
#[serial]
async fn test_upload_compressed_avif() {
    load_env();
    let webhook_url = match std::env::var("DISCORD_WEBHOOK_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("Skipping: DISCORD_WEBHOOK_URL not set");
            return;
        }
    };
    compress_and_upload(&webhook_url, "avif", "test_upload_compressed_avif").await;
}

/// Test 15 - Compress to PNG (re-encode), then upload.
#[tokio::test]
#[ignore]
#[serial]
async fn test_upload_compressed_png() {
    load_env();
    let webhook_url = match std::env::var("DISCORD_WEBHOOK_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("Skipping: DISCORD_WEBHOOK_URL not set");
            return;
        }
    };
    compress_and_upload(&webhook_url, "png", "test_upload_compressed_png").await;
}

/// Test 16 - Compress with smart PNG mode, then upload.
#[tokio::test]
#[ignore]
#[serial]
async fn test_upload_compressed_png_smart() {
    load_env();
    let webhook_url = match std::env::var("DISCORD_WEBHOOK_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("Skipping: DISCORD_WEBHOOK_URL not set");
            return;
        }
    };
    compress_and_upload(
        &webhook_url,
        "png_smart",
        "test_upload_compressed_png_smart",
    )
    .await;
}
