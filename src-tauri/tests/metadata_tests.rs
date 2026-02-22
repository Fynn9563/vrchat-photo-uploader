//! Integration tests for PNG metadata extraction and embedding.
//!
//! Uses the crate's `test_helpers` to create PNG fixtures with embedded VRCX metadata,
//! then verifies that `image_processor::extract_metadata()` correctly extracts (or
//! gracefully handles missing) metadata from those files.
//!
//! Also tests `metadata_editor::embed_metadata()` for round-trip embedding and
//! extraction, output file creation, and error handling.

use VRChat_Photo_Uploader::commands::{AuthorInfo, ImageMetadata, PlayerInfo, WorldInfo};
use VRChat_Photo_Uploader::image_processor;
use VRChat_Photo_Uploader::metadata_editor;
use VRChat_Photo_Uploader::test_helpers::*;

// ---------------------------------------------------------------------------
// 1. Extract VRCX metadata with world and players
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_extract_vrcx_metadata_with_world_and_players() {
    let metadata_json = create_test_metadata(
        "wrld_abc123",
        "Cozy Cabin",
        &["Alice", "Bob", "Charlie"],
        1705312200,
    );
    let png_data = create_png_with_metadata(&metadata_json);
    let tmp = create_temp_png(&png_data, "meta_world_players.png");

    let result = image_processor::extract_metadata(&tmp.path_str())
        .await
        .expect("extract_metadata should not return an error");

    let metadata = result.expect("Should have found VRCX metadata");

    // Verify world info
    let world = metadata.world.expect("world should be Some");
    assert_eq!(world.id, "wrld_abc123");
    assert_eq!(world.name, "Cozy Cabin");

    // Verify players
    assert_eq!(metadata.players.len(), 3);
    assert_eq!(metadata.players[0].display_name, "Alice");
    assert_eq!(metadata.players[1].display_name, "Bob");
    assert_eq!(metadata.players[2].display_name, "Charlie");

    // Verify author
    let author = metadata.author.expect("author should be Some");
    assert_eq!(author.display_name, "TestUser");
    assert_eq!(author.id, "usr_test123");
}

// ---------------------------------------------------------------------------
// 2. Extract full metadata structure (all fields populated)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_extract_full_metadata_structure() {
    // Build a fully-populated JSON blob manually so we control every field.
    let json = serde_json::json!({
        "application": "VRCX",
        "version": 2,
        "author": {
            "displayName": "PhotoTaker",
            "id": "usr_photo_taker_42"
        },
        "world": {
            "name": "Neon Arcade",
            "id": "wrld_neon_arcade_99",
            "instanceId": "99999~friends(usr_photo_taker_42)"
        },
        "players": [
            { "displayName": "Player1", "id": "usr_player_1" },
            { "displayName": "Player2", "id": "usr_player_2" }
        ],
        "created_at": "2024-06-15T18:30:00Z"
    });

    let png_data = create_png_with_metadata(&json.to_string());
    let tmp = create_temp_png(&png_data, "meta_full_structure.png");

    let result = image_processor::extract_metadata(&tmp.path_str())
        .await
        .expect("extract_metadata should succeed");

    let metadata = result.expect("Should have found metadata");

    let author = metadata.author.expect("author should be present");
    assert_eq!(author.display_name, "PhotoTaker");
    assert_eq!(author.id, "usr_photo_taker_42");

    let world = metadata.world.expect("world should be present");
    assert_eq!(world.name, "Neon Arcade");
    assert_eq!(world.id, "wrld_neon_arcade_99");
    assert_eq!(world.instance_id, "99999~friends(usr_photo_taker_42)");

    assert_eq!(metadata.players.len(), 2);
    assert_eq!(metadata.players[0].display_name, "Player1");
    assert_eq!(metadata.players[0].id, "usr_player_1");
    assert_eq!(metadata.players[1].display_name, "Player2");
    assert_eq!(metadata.players[1].id, "usr_player_2");
}

// ---------------------------------------------------------------------------
// 3. No metadata in PNG -> returns Ok(None)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_extract_metadata_no_metadata_returns_none() {
    let png_data = create_minimal_png();
    let tmp = create_temp_png(&png_data, "meta_no_metadata.png");

    let result = image_processor::extract_metadata(&tmp.path_str())
        .await
        .expect("extract_metadata should succeed even without metadata");

    assert!(
        result.is_none(),
        "A plain PNG with no text chunks should yield None"
    );
}

// ---------------------------------------------------------------------------
// 4. Non-PNG file -> returns an error
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_extract_metadata_non_png_file_returns_error() {
    // Write a plain text file with a .txt extension.
    let dir = std::env::temp_dir().join("vrchat_photo_uploader_tests");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("not_an_image.txt");
    std::fs::write(&path, b"hello world").expect("failed to write temp file");

    let path_str = path.to_string_lossy().to_string();
    let result = image_processor::extract_metadata(&path_str).await;

    // Cleanup
    let _ = std::fs::remove_file(&path);

    assert!(
        result.is_err(),
        "extract_metadata should return an error for a non-image file extension"
    );
}

// ---------------------------------------------------------------------------
// 5. Non-existent file -> returns an error
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_extract_metadata_nonexistent_file_returns_error() {
    let result = image_processor::extract_metadata(
        "/tmp/vrchat_photo_uploader_tests/this_file_does_not_exist_at_all.png",
    )
    .await;

    assert!(
        result.is_err(),
        "extract_metadata should return an error for a non-existent file"
    );
}

// ---------------------------------------------------------------------------
// 6. Timestamp extraction from VRChat filename patterns
// ---------------------------------------------------------------------------

#[test]
fn test_timestamp_from_standard_vrchat_filename() {
    // Standard VRChat screenshot pattern: VRChat_YYYY-MM-DD_HH-MM-SS.SSS_WIDTHxHEIGHT.png
    let ts = image_processor::get_timestamp_from_filename(
        "/pictures/VRChat/2024-01-15_14-30-00.123_1920x1080.png",
    );
    assert!(
        ts.is_some(),
        "Should extract timestamp from standard VRChat filename"
    );
    // The exact value depends on the local timezone offset, but it should be
    // somewhere in the ballpark of 2024-01-15 14:30:00 UTC (1705325400).
    let timestamp = ts.unwrap();
    // Allow a generous +/- 24h window to account for any timezone
    assert!(
        (1705225400..=1705425400).contains(&timestamp),
        "Timestamp {timestamp} is outside expected range for 2024-01-15 14:30:00"
    );
}

#[test]
fn test_timestamp_from_filename_without_milliseconds() {
    let ts = image_processor::get_timestamp_from_filename(
        "/screenshots/VRChat_2024-06-20_09-15-30_3840x2160.png",
    );
    assert!(
        ts.is_some(),
        "Should extract timestamp even without sub-second precision"
    );
}

#[test]
fn test_timestamp_from_filename_no_pattern() {
    // A filename that does not match VRChat's naming convention and does not
    // exist on disk (so file creation time fallback also fails).
    let ts = image_processor::get_timestamp_from_filename("/nonexistent/random_photo.png");
    assert!(
        ts.is_none(),
        "Should return None when filename has no date pattern and file does not exist"
    );
}

#[test]
fn test_timestamp_from_filename_only_date_no_time() {
    // Only a date component, no time component separated by underscore.
    let ts = image_processor::get_timestamp_from_filename("/photos/2024-01-15.png");
    // The regex expects YYYY-MM-DD_HH-MM-SS, so a bare date should not match.
    assert!(
        ts.is_none(),
        "A date-only filename should not match the VRChat timestamp pattern"
    );
}

// ---------------------------------------------------------------------------
// 7. PNG without VRCX metadata but with other text chunks
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_extract_metadata_png_with_non_vrcx_text_chunks() {
    // Build a PNG that has a tEXt chunk with keyword "Comment" instead of "Description".
    // This simulates a PNG exported from a generic image editor.
    let mut buf = Vec::new();

    // PNG signature
    buf.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    // IHDR chunk: 1x1 pixel, 8-bit RGB
    let ihdr_data: [u8; 13] = [
        0, 0, 0, 1, // width
        0, 0, 0, 1, // height
        8, 2, 0, 0, 0, // 8-bit RGB
    ];
    write_png_chunk(&mut buf, b"IHDR", &ihdr_data);

    // tEXt chunk with keyword "Comment" (not "Description")
    let mut text_data = Vec::new();
    text_data.extend_from_slice(b"Comment");
    text_data.push(0); // null separator
    text_data.extend_from_slice(b"This is a generic comment, not VRCX metadata");
    write_png_chunk(&mut buf, b"tEXt", &text_data);

    // Another tEXt chunk with keyword "Software"
    let mut sw_data = Vec::new();
    sw_data.extend_from_slice(b"Software");
    sw_data.push(0);
    sw_data.extend_from_slice(b"GIMP 2.10");
    write_png_chunk(&mut buf, b"tEXt", &sw_data);

    // IDAT chunk
    let raw_scanline: [u8; 4] = [0, 255, 255, 255];
    let compressed = deflate_bytes(&raw_scanline);
    write_png_chunk(&mut buf, b"IDAT", &compressed);

    // IEND
    write_png_chunk(&mut buf, b"IEND", &[]);

    let tmp = create_temp_png(&buf, "meta_other_text_chunks.png");

    let result = image_processor::extract_metadata(&tmp.path_str())
        .await
        .expect("extract_metadata should succeed");

    assert!(
        result.is_none(),
        "PNG with non-Description text chunks should yield None metadata"
    );
}

// ---------------------------------------------------------------------------
// Helpers re-exported from the crate are used above.  The two below are thin
// wrappers that delegate to internal `test_helpers` functions that are NOT
// public (they are used only to build the custom PNG in test 7).
// ---------------------------------------------------------------------------

/// Write a PNG chunk (length + type + data + CRC32) into `buf`.
fn write_png_chunk(buf: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
    let length = data.len() as u32;
    buf.extend_from_slice(&length.to_be_bytes());
    buf.extend_from_slice(chunk_type);
    buf.extend_from_slice(data);
    let crc = crc32_compute(chunk_type, data);
    buf.extend_from_slice(&crc.to_be_bytes());
}

/// Compute CRC32 over chunk_type + data (PNG specification CRC).
fn crc32_compute(chunk_type: &[u8], data: &[u8]) -> u32 {
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

/// Deflate-compress bytes (zlib wrapper), matching the PNG IDAT encoding.
fn deflate_bytes(data: &[u8]) -> Vec<u8> {
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

// ===========================================================================
// Metadata embedding tests (metadata_editor::embed_metadata)
// ===========================================================================

/// Helper: build a full ImageMetadata struct for embedding tests.
fn make_test_metadata() -> ImageMetadata {
    ImageMetadata {
        author: Some(AuthorInfo {
            display_name: "EmbedTestUser".to_string(),
            id: "usr_embed_test".to_string(),
        }),
        world: Some(WorldInfo {
            name: "Embed Test World".to_string(),
            id: "wrld_embed_test".to_string(),
            instance_id: "54321~private(usr_embed_test)".to_string(),
        }),
        players: vec![
            PlayerInfo {
                display_name: "Player_A".to_string(),
                id: "usr_player_a".to_string(),
            },
            PlayerInfo {
                display_name: "Player_B".to_string(),
                id: "usr_player_b".to_string(),
            },
        ],
    }
}

// ---------------------------------------------------------------------------
// 8. Round-trip: embed metadata then extract it back
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_embed_metadata_round_trip() {
    let png_data = create_visible_test_png();
    let tmp = create_temp_png(&png_data, "embed_roundtrip.png");

    let metadata = make_test_metadata();
    let output_path = metadata_editor::embed_metadata(&tmp.path_str(), metadata)
        .await
        .expect("embed_metadata should succeed");

    // Verify output file exists
    assert!(
        std::path::Path::new(&output_path).exists(),
        "Output file should exist"
    );

    // Extract metadata from the output file
    let extracted = image_processor::extract_metadata(&output_path)
        .await
        .expect("extract_metadata should succeed on embedded file");

    let meta = extracted.expect("Should find embedded metadata");

    // Verify author
    let author = meta.author.expect("author should be present");
    assert_eq!(author.display_name, "EmbedTestUser");
    assert_eq!(author.id, "usr_embed_test");

    // Verify world
    let world = meta.world.expect("world should be present");
    assert_eq!(world.name, "Embed Test World");
    assert_eq!(world.id, "wrld_embed_test");

    // Verify players
    assert_eq!(meta.players.len(), 2);
    assert_eq!(meta.players[0].display_name, "Player_A");
    assert_eq!(meta.players[1].display_name, "Player_B");

    // Cleanup
    let _ = std::fs::remove_file(&output_path);
}

// ---------------------------------------------------------------------------
// 9. Output file has _Modified suffix
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_embed_metadata_creates_modified_file() {
    let png_data = create_visible_test_png();
    let tmp = create_temp_png(&png_data, "embed_suffix_test.png");

    let metadata = ImageMetadata {
        author: None,
        world: None,
        players: vec![],
    };

    let output_path = metadata_editor::embed_metadata(&tmp.path_str(), metadata)
        .await
        .expect("embed_metadata should succeed");

    assert!(
        output_path.contains("embed_suffix_test_Modified.png"),
        "Output should have _Modified suffix, got: {output_path}"
    );
    assert!(
        std::path::Path::new(&output_path).exists(),
        "Modified file should exist on disk"
    );

    // Cleanup
    let _ = std::fs::remove_file(&output_path);
}

// ---------------------------------------------------------------------------
// 10. Embed into nonexistent file returns error
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_embed_metadata_nonexistent_file() {
    let metadata = make_test_metadata();
    let result = metadata_editor::embed_metadata(
        "/tmp/vrchat_photo_uploader_tests/this_does_not_exist.png",
        metadata,
    )
    .await;

    assert!(
        result.is_err(),
        "embed_metadata should fail on nonexistent file"
    );
}

// ---------------------------------------------------------------------------
// 11. Output is a valid, loadable PNG with correct dimensions
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_embed_metadata_preserves_image() {
    let png_data = create_visible_test_png();
    let tmp = create_temp_png(&png_data, "embed_preserves.png");

    let metadata = make_test_metadata();
    let output_path = metadata_editor::embed_metadata(&tmp.path_str(), metadata)
        .await
        .expect("embed_metadata should succeed");

    // Load and verify the output image
    let img = image::open(&output_path).expect("Output should be a valid image");
    assert_eq!(img.width(), 200, "Width should be preserved");
    assert_eq!(img.height(), 200, "Height should be preserved");

    // Cleanup
    let _ = std::fs::remove_file(&output_path);
}

// ---------------------------------------------------------------------------
// 12. Embed with special/unicode characters
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_embed_metadata_with_unicode() {
    let png_data = create_visible_test_png();
    let tmp = create_temp_png(&png_data, "embed_unicode.png");

    let metadata = ImageMetadata {
        author: Some(AuthorInfo {
            display_name: "ユーザー★".to_string(),
            id: "usr_unicode".to_string(),
        }),
        world: Some(WorldInfo {
            name: "日本語ワールド 🌸".to_string(),
            id: "wrld_jp".to_string(),
            instance_id: "42~friends".to_string(),
        }),
        players: vec![
            PlayerInfo {
                display_name: "Ñoño".to_string(),
                id: "usr_nono".to_string(),
            },
            PlayerInfo {
                display_name: "O'Brien".to_string(),
                id: "usr_obrien".to_string(),
            },
        ],
    };

    let output_path = metadata_editor::embed_metadata(&tmp.path_str(), metadata)
        .await
        .expect("embed_metadata should handle unicode");

    // Extract and verify unicode data survives the round-trip
    let extracted = image_processor::extract_metadata(&output_path)
        .await
        .expect("extract should succeed")
        .expect("should find metadata");

    let author = extracted.author.expect("author should exist");
    assert_eq!(author.display_name, "ユーザー★");

    let world = extracted.world.expect("world should exist");
    assert_eq!(world.name, "日本語ワールド 🌸");

    assert_eq!(extracted.players[0].display_name, "Ñoño");
    assert_eq!(extracted.players[1].display_name, "O'Brien");

    // Cleanup
    let _ = std::fs::remove_file(&output_path);
}

// ---------------------------------------------------------------------------
// 13. Embed minimal metadata (no author, no world, empty players)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_embed_metadata_minimal() {
    let png_data = create_visible_test_png();
    let tmp = create_temp_png(&png_data, "embed_minimal.png");

    let metadata = ImageMetadata {
        author: None,
        world: None,
        players: vec![],
    };

    let output_path = metadata_editor::embed_metadata(&tmp.path_str(), metadata)
        .await
        .expect("embed_metadata should succeed with minimal metadata");

    // Should still produce a valid PNG
    let img = image::open(&output_path).expect("Output should be a valid image");
    assert_eq!(img.width(), 200);

    // Cleanup
    let _ = std::fs::remove_file(&output_path);
}

// ---------------------------------------------------------------------------
// 14. Embed overwrites existing VRCX metadata
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_embed_metadata_overwrites_existing() {
    // Create a PNG that already has VRCX metadata
    let original_json = create_test_metadata(
        "wrld_original",
        "Original World",
        &["OldPlayer"],
        1705312200,
    );
    let png_data = create_png_with_metadata(&original_json);
    let tmp = create_temp_png(&png_data, "embed_overwrite.png");

    // Verify original metadata is there
    let original_extracted = image_processor::extract_metadata(&tmp.path_str())
        .await
        .expect("should succeed")
        .expect("should find original metadata");
    assert_eq!(original_extracted.world.unwrap().name, "Original World");

    // Now embed new metadata
    let new_metadata = ImageMetadata {
        author: Some(AuthorInfo {
            display_name: "NewAuthor".to_string(),
            id: "usr_new".to_string(),
        }),
        world: Some(WorldInfo {
            name: "Replacement World".to_string(),
            id: "wrld_replacement".to_string(),
            instance_id: "99~public".to_string(),
        }),
        players: vec![PlayerInfo {
            display_name: "NewPlayer".to_string(),
            id: "usr_newplayer".to_string(),
        }],
    };

    let output_path = metadata_editor::embed_metadata(&tmp.path_str(), new_metadata)
        .await
        .expect("embed_metadata should succeed");

    // Extract from the modified file and verify new metadata replaced old
    let extracted = image_processor::extract_metadata(&output_path)
        .await
        .expect("should succeed")
        .expect("should find new metadata");

    let world = extracted.world.expect("world should be present");
    assert_eq!(world.name, "Replacement World");
    assert_eq!(world.id, "wrld_replacement");

    let author = extracted.author.expect("author should be present");
    assert_eq!(author.display_name, "NewAuthor");

    assert_eq!(extracted.players.len(), 1);
    assert_eq!(extracted.players[0].display_name, "NewPlayer");

    // Cleanup
    let _ = std::fs::remove_file(&output_path);
}

// ---------------------------------------------------------------------------
// 15. Embed with many players (large metadata)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_embed_metadata_many_players() {
    let png_data = create_visible_test_png();
    let tmp = create_temp_png(&png_data, "embed_many_players.png");

    let players: Vec<PlayerInfo> = (0..50)
        .map(|i| PlayerInfo {
            display_name: format!("Player_{i:03}"),
            id: format!("usr_player_{i:03}"),
        })
        .collect();

    let metadata = ImageMetadata {
        author: Some(AuthorInfo {
            display_name: "Host".to_string(),
            id: "usr_host".to_string(),
        }),
        world: Some(WorldInfo {
            name: "Crowded World".to_string(),
            id: "wrld_crowded".to_string(),
            instance_id: "1~public".to_string(),
        }),
        players,
    };

    let output_path = metadata_editor::embed_metadata(&tmp.path_str(), metadata)
        .await
        .expect("Should handle large player lists");

    // Verify all 50 players survived the round-trip
    let extracted = image_processor::extract_metadata(&output_path)
        .await
        .expect("should succeed")
        .expect("should find metadata");

    assert_eq!(
        extracted.players.len(),
        50,
        "All 50 players should be preserved"
    );
    assert_eq!(extracted.players[0].display_name, "Player_000");
    assert_eq!(extracted.players[49].display_name, "Player_049");

    // Cleanup
    let _ = std::fs::remove_file(&output_path);
}
