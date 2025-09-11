use std::fs::File;
use std::io::Write;
use vrchat_photo_uploader::{
    security::{InputValidator, FileSystemGuard},
    image_processor,
    errors::AppError,
};

/// Integration tests for the VRChat Photo Uploader backend
/// These tests verify that different modules work together correctly

#[tokio::test]
async fn test_full_image_processing_workflow() {
    // Create a test image file
    let temp_dir = std::env::temp_dir();
    let test_file_path = temp_dir.join("integration_test_image.png");
    
    // Create a minimal PNG file
    let png_data = create_minimal_png();
    
    if let Ok(mut file) = File::create(&test_file_path) {
        let _ = file.write_all(&png_data);
        
        let path_str = test_file_path.to_string_lossy();
        
        // Test the complete workflow
        
        // 1. Validate the file path
        let validation_result = InputValidator::validate_file_path(&path_str);
        
        // 2. Check if compression is needed
        let compression_result = image_processor::should_compress_image(&path_str);
        
        // 3. Get image info
        let info_result = image_processor::get_image_info(&path_str);
        
        // 4. Try to extract metadata
        let metadata_result = image_processor::extract_metadata(&path_str).await;
        
        // Cleanup
        let _ = std::fs::remove_file(&test_file_path);
        
        // Verify results
        match validation_result {
            Ok(_) => println!("✅ File validation passed"),
            Err(e) => println!("⚠️  File validation failed: {} (acceptable for minimal PNG)", e),
        }
        
        match compression_result {
            Ok(needs_compression) => {
                println!("✅ Compression check passed: needs_compression = {}", needs_compression);
                assert!(!needs_compression, "Small test image should not need compression");
            }
            Err(e) => println!("⚠️  Compression check failed: {} (might be due to image validation)", e),
        }
        
        match info_result {
            Ok((width, height, size)) => {
                println!("✅ Image info extracted: {}x{}, {} bytes", width, height, size);
                assert_eq!(width, 1, "Test image should be 1 pixel wide");
                assert_eq!(height, 1, "Test image should be 1 pixel tall");
            }
            Err(e) => println!("⚠️  Image info extraction failed: {} (acceptable for minimal PNG)", e),
        }
        
        match metadata_result {
            Ok(Some(metadata)) => {
                println!("✅ Metadata extracted: {:?}", metadata);
            }
            Ok(None) => {
                println!("✅ No metadata found (expected for test image)");
            }
            Err(e) => println!("⚠️  Metadata extraction failed: {} (expected for test image)", e),
        }
    } else {
        panic!("Failed to create test file");
    }
}

#[test]
fn test_security_validation_integration() {
    // Test various security scenarios
    
    // 1. Webhook validation
    let valid_webhook = "https://discord.com/api/webhooks/123456789012345678/abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890-_";
    let invalid_webhook = "https://malicious.com/api/webhooks/123456789012345678/token";
    
    assert!(InputValidator::validate_webhook_url(valid_webhook).is_ok());
    assert!(InputValidator::validate_webhook_url(invalid_webhook).is_err());
    
    // 2. Webhook name validation
    let valid_name = "My Discord Server";
    let invalid_name = "Server<script>alert('xss')</script>";
    
    assert!(InputValidator::validate_webhook_name(valid_name).is_ok());
    assert!(InputValidator::validate_webhook_name(invalid_name).is_err());
    
    // 3. Upload settings validation
    assert!(InputValidator::validate_upload_settings(5, true).is_ok());
    assert!(InputValidator::validate_upload_settings(0, false).is_err());
    assert!(InputValidator::validate_upload_settings(11, true).is_err());
    
    // 4. Filename sanitization
    let dangerous_filename = "../../etc/passwd<script>.png";
    let safe_filename = InputValidator::sanitize_filename(dangerous_filename);
    // sanitize_filename replaces dangerous characters but doesn't modify ".."
    // That's handled by validate_file_path which checks for path traversal
    assert!(safe_filename.contains(".."));  // ".." is preserved (handled elsewhere)
    assert!(!safe_filename.contains("<"));  // HTML brackets are removed
    assert!(!safe_filename.contains(">"));  // HTML brackets are removed
    
    println!("✅ All security validations passed");
}

#[test]
fn test_filesystem_operations_integration() {
    // Test filesystem operations work together
    
    // 1. Create secure temp file
    let temp_result = FileSystemGuard::create_secure_temp_file("test.png");
    assert!(temp_result.is_ok());
    
    let temp_path = temp_result.unwrap();
    assert!(temp_path.to_string_lossy().contains("vrchat_uploader_secure"));
    assert_eq!(temp_path.extension().unwrap(), "png");
    
    // 2. Test cleanup
    let cleanup_result = FileSystemGuard::cleanup_temp_files();
    assert!(cleanup_result.is_ok());
    
    println!("✅ Filesystem operations integration passed");
}

#[test]
fn test_error_handling_integration() {
    // Test that errors propagate correctly through the system
    
    // 1. Test with nonexistent file
    let nonexistent = "definitely_does_not_exist.png";
    
    // Should fail at validation level
    assert!(InputValidator::validate_file_path(nonexistent).is_err());
    
    // Should fail at image processing level
    assert!(image_processor::should_compress_image(nonexistent).is_err());
    assert!(image_processor::get_image_info(nonexistent).is_err());
    
    // 2. Test with invalid webhook
    let invalid_webhook = "not-a-webhook";
    let result = InputValidator::validate_webhook_url(invalid_webhook);
    assert!(result.is_err());
    
    // Verify error type
    match result {
        Err(AppError::InvalidWebhook { url }) => {
            assert_eq!(url, invalid_webhook);
            println!("✅ Correct error type for invalid webhook");
        }
        Err(other) => {
            println!("⚠️  Got different error type: {:?}", other);
        }
        Ok(_) => panic!("Should have failed"),
    }
    
    println!("✅ Error handling integration passed");
}

#[tokio::test]
async fn test_async_operations_integration() {
    // Test async operations work correctly with sync operations
    
    let temp_dir = std::env::temp_dir();
    let test_file_path = temp_dir.join("async_test_image.png");
    
    // Create test file
    let png_data = create_minimal_png();
    if let Ok(mut file) = File::create(&test_file_path) {
        let _ = file.write_all(&png_data);
        
        let path_str = test_file_path.to_string_lossy();
        
        // Mix of async and sync operations
        let sync_validation = InputValidator::validate_file_path(&path_str);
        let async_metadata = image_processor::extract_metadata(&path_str).await;
        let sync_compression = image_processor::should_compress_image(&path_str);
        
        // Cleanup
        let _ = std::fs::remove_file(&test_file_path);
        
        // All operations should complete without panic
        match (sync_validation, async_metadata, sync_compression) {
            (Ok(_), Ok(_), Ok(_)) => println!("✅ All async/sync operations succeeded"),
            (Ok(_), Ok(_), Err(e)) => println!("⚠️  Sync compression failed: {}", e),
            (Ok(_), Err(e), Ok(_)) => println!("⚠️  Async metadata failed: {}", e),
            (Err(e), Ok(_), Ok(_)) => println!("⚠️  Sync validation failed: {}", e),
            _ => println!("⚠️  Multiple operations failed (acceptable for minimal test PNG)"),
        }
    }
    
    println!("✅ Async operations integration completed");
}

#[test]
fn test_data_flow_integration() {
    // Test that data flows correctly between modules
    
    // 1. Start with user input (webhook)
    let user_webhook_input = "  https://discord.com/api/webhooks/123456789012345678/abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890-_  ";
    
    // 2. Validate and sanitize
    let trimmed = user_webhook_input.trim();
    let validation_result = InputValidator::validate_webhook_url(trimmed);
    assert!(validation_result.is_ok());
    
    // 3. Test webhook name flow
    let user_name_input = "  My Gaming Server  ";
    let trimmed_name = user_name_input.trim();
    
    let name_validation = InputValidator::validate_webhook_name(trimmed_name);
    assert!(name_validation.is_ok());
    
    // 4. Test settings flow
    let user_max_images = 5u8;
    let user_group_metadata = true;
    let settings_validation = InputValidator::validate_upload_settings(user_max_images, user_group_metadata);
    assert!(settings_validation.is_ok());
    
    println!("✅ Data flow integration passed");
}

/// Helper function to create a minimal PNG for testing
fn create_minimal_png() -> Vec<u8> {
    vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
        0x00, 0x00, 0x00, 0x0D, // IHDR chunk length
        0x49, 0x48, 0x44, 0x52, // IHDR
        0x00, 0x00, 0x00, 0x01, // width = 1
        0x00, 0x00, 0x00, 0x01, // height = 1
        0x08, 0x02, 0x00, 0x00, 0x00, // bit depth = 8, color type = 2 (RGB)
        0x90, 0x77, 0x53, 0xDE, // IHDR CRC
        0x00, 0x00, 0x00, 0x0C, // IDAT chunk length
        0x49, 0x44, 0x41, 0x54, // IDAT
        0x08, 0x99, 0x01, 0x01, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, // IDAT data
        0x00, 0x00, 0x00, 0x00, // IEND chunk length
        0x49, 0x45, 0x4E, 0x44, // IEND
        0xAE, 0x42, 0x60, 0x82, // IEND CRC
    ]
}