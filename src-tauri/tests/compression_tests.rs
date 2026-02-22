//! Integration tests for image compression and processing functions.
//!
//! These tests exercise the public image processing API: compression to
//! various formats, image info retrieval, thumbnail generation, and the
//! should-compress heuristic.

use VRChat_Photo_Uploader::{
    image_processor,
    test_helpers::{create_minimal_png, create_png_of_size, create_temp_png},
};

// ---------------------------------------------------------------------------
// should_compress_image
// ---------------------------------------------------------------------------

#[test]
fn test_should_compress_small_file_returns_false() {
    let png = create_minimal_png();
    let tmp = create_temp_png(&png, "compress_small.png");

    let result = image_processor::should_compress_image(&tmp.path_str())
        .expect("should_compress_image should succeed for a valid file");
    assert!(
        !result,
        "A tiny PNG should not exceed the compression threshold"
    );
}

#[test]
fn test_should_compress_large_file_returns_true() {
    // The compression threshold is 8 MB; create a ~9 MB PNG.
    let png = create_png_of_size(9 * 1024 * 1024);
    let tmp = create_temp_png(&png, "compress_large.png");

    let result = image_processor::should_compress_image(&tmp.path_str())
        .expect("should_compress_image should succeed for a valid file");
    assert!(
        result,
        "A file larger than 8 MB should be flagged for compression"
    );
}

// ---------------------------------------------------------------------------
// compress_image_with_format — WebP
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_compress_to_webp_produces_valid_output() {
    let png = create_minimal_png();
    let tmp = create_temp_png(&png, "compress_webp_src.png");

    let output_path =
        image_processor::compress_image_with_format(&tmp.path_str(), 80, "webp", None)
            .await
            .expect("WebP compression should succeed");

    // Output file must exist and have a .webp extension.
    let output = std::path::Path::new(&output_path);
    assert!(output.exists(), "WebP output file should exist");
    assert_eq!(
        output.extension().and_then(|e| e.to_str()),
        Some("webp"),
        "Output should have .webp extension"
    );

    // The output should be non-empty.
    let output_size = std::fs::metadata(&output_path)
        .expect("metadata read")
        .len();
    assert!(output_size > 0, "WebP output should not be empty");

    // Cleanup
    let _ = std::fs::remove_file(&output_path);
}

#[tokio::test]
async fn test_compress_to_webp_likely_smaller_than_source() {
    // Use a larger source so compression has something to work with.
    let png = create_png_of_size(50_000);
    let tmp = create_temp_png(&png, "compress_webp_size.png");

    let source_size = std::fs::metadata(tmp.path_str())
        .expect("source metadata")
        .len();

    let output_path =
        image_processor::compress_image_with_format(&tmp.path_str(), 75, "webp", None)
            .await
            .expect("WebP compression should succeed");

    let output_size = std::fs::metadata(&output_path)
        .expect("output metadata")
        .len();

    assert!(
        output_size < source_size,
        "WebP output ({output_size} bytes) should be smaller than PNG source ({source_size} bytes)",
    );

    let _ = std::fs::remove_file(&output_path);
}

// ---------------------------------------------------------------------------
// compress_image_with_format — AVIF
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_compress_to_avif_produces_valid_output() {
    let png = create_minimal_png();
    let tmp = create_temp_png(&png, "compress_avif_src.png");

    let output_path =
        image_processor::compress_image_with_format(&tmp.path_str(), 70, "avif", None)
            .await
            .expect("AVIF compression should succeed");

    let output = std::path::Path::new(&output_path);
    assert!(output.exists(), "AVIF output file should exist");
    assert_eq!(
        output.extension().and_then(|e| e.to_str()),
        Some("avif"),
        "Output should have .avif extension"
    );

    let output_size = std::fs::metadata(&output_path)
        .expect("metadata read")
        .len();
    assert!(output_size > 0, "AVIF output should not be empty");

    let _ = std::fs::remove_file(&output_path);
}

// ---------------------------------------------------------------------------
// get_image_info
// ---------------------------------------------------------------------------

#[test]
fn test_get_image_info_returns_correct_values_for_minimal_png() {
    let png = create_minimal_png();
    let tmp = create_temp_png(&png, "info_minimal.png");

    let (width, height, file_size) = image_processor::get_image_info(&tmp.path_str())
        .expect("get_image_info should succeed for a valid PNG");

    assert_eq!(width, 1, "Minimal PNG width should be 1");
    assert_eq!(height, 1, "Minimal PNG height should be 1");
    assert_eq!(
        file_size,
        png.len() as u64,
        "Reported file size should match the bytes written"
    );
}

#[test]
fn test_get_image_info_returns_correct_dimensions_for_larger_png() {
    // create_png_of_size uses width=100 and computes height from the target size.
    // We just need a PNG large enough so that the dimensions are 100 x N where N >= 1.
    let png = create_png_of_size(10_000);
    let tmp = create_temp_png(&png, "info_larger.png");

    let (width, height, file_size) =
        image_processor::get_image_info(&tmp.path_str()).expect("get_image_info should succeed");

    assert_eq!(
        width, 100,
        "Width should be 100 as set by create_png_of_size"
    );
    assert!(height >= 1, "Height should be at least 1");
    assert!(file_size > 0, "File size should be positive");
}

#[test]
fn test_get_image_info_detects_png_format() {
    let png = create_minimal_png();
    let tmp = create_temp_png(&png, "info_format.png");

    // get_image_info returns (width, height, size). The file we wrote is a valid
    // PNG, so the reader should be able to guess the format successfully (the
    // function internally uses `with_guessed_format`). We just verify it does
    // not error out on a well-formed PNG.
    let result = image_processor::get_image_info(&tmp.path_str());
    assert!(
        result.is_ok(),
        "get_image_info should succeed for a valid PNG file"
    );
}

// ---------------------------------------------------------------------------
// generate_thumbnail
// ---------------------------------------------------------------------------

#[test]
fn test_generate_thumbnail_produces_smaller_output() {
    // Create a source PNG with enough pixels to make a meaningful thumbnail.
    let png = create_png_of_size(50_000);
    let tmp = create_temp_png(&png, "thumb_src.png");

    let source_size = std::fs::metadata(tmp.path_str())
        .expect("source metadata")
        .len();

    let thumb_path = image_processor::generate_thumbnail(&tmp.path_str(), 64)
        .expect("generate_thumbnail should succeed");

    let thumb_size = std::fs::metadata(&thumb_path)
        .expect("thumb metadata")
        .len();

    assert!(thumb_size > 0, "Thumbnail should not be empty");
    assert!(
        thumb_size < source_size,
        "Thumbnail ({thumb_size} bytes) should be smaller than source ({source_size} bytes)",
    );

    let _ = std::fs::remove_file(&thumb_path);
}

#[test]
fn test_generate_thumbnail_respects_max_dimension() {
    let png = create_png_of_size(50_000);
    let tmp = create_temp_png(&png, "thumb_dim.png");

    let max_dim: u32 = 32;
    let thumb_path = image_processor::generate_thumbnail(&tmp.path_str(), max_dim)
        .expect("generate_thumbnail should succeed");

    // Load the thumbnail and verify its dimensions are within the limit.
    let thumb_img =
        image::open(&thumb_path).expect("Thumbnail should be loadable by the image crate");
    assert!(
        thumb_img.width() <= max_dim && thumb_img.height() <= max_dim,
        "Thumbnail dimensions ({}x{}) should not exceed max_dimension ({})",
        thumb_img.width(),
        thumb_img.height(),
        max_dim,
    );

    let _ = std::fs::remove_file(&thumb_path);
}

// ---------------------------------------------------------------------------
// compress_image_with_format — scale factor
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_compress_with_scale_reduces_dimensions() {
    let png = create_png_of_size(50_000);
    let tmp = create_temp_png(&png, "compress_scale_src.png");

    let (orig_w, orig_h, _) = image_processor::get_image_info(&tmp.path_str())
        .expect("get_image_info should succeed on the source");

    // Compress with a 0.5 scale factor to WebP.
    let output_path =
        image_processor::compress_image_with_format(&tmp.path_str(), 80, "webp", Some(0.5))
            .await
            .expect("Scaled WebP compression should succeed");

    // Load the output and check dimensions.
    let output_img =
        image::open(&output_path).expect("Output should be loadable by the image crate");

    let expected_w = (orig_w as f32 * 0.5) as u32;
    let expected_h = (orig_h as f32 * 0.5) as u32;

    assert_eq!(
        output_img.width(),
        expected_w,
        "Scaled width should be half of original ({orig_w} -> {expected_w})",
    );
    assert_eq!(
        output_img.height(),
        expected_h,
        "Scaled height should be half of original ({orig_h} -> {expected_h})",
    );

    let _ = std::fs::remove_file(&output_path);
}

#[tokio::test]
async fn test_compress_with_scale_one_preserves_dimensions() {
    let png = create_png_of_size(10_000);
    let tmp = create_temp_png(&png, "compress_scale1_src.png");

    let (orig_w, orig_h, _) =
        image_processor::get_image_info(&tmp.path_str()).expect("get_image_info should succeed");

    // Scale 1.0 should leave dimensions unchanged.
    let output_path =
        image_processor::compress_image_with_format(&tmp.path_str(), 80, "webp", Some(1.0))
            .await
            .expect("Compression with scale 1.0 should succeed");

    let output_img =
        image::open(&output_path).expect("Output should be loadable by the image crate");

    assert_eq!(
        output_img.width(),
        orig_w,
        "Scale 1.0 should preserve width"
    );
    assert_eq!(
        output_img.height(),
        orig_h,
        "Scale 1.0 should preserve height"
    );

    let _ = std::fs::remove_file(&output_path);
}
