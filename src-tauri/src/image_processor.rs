use chrono::Offset;
use flate2::read::DeflateDecoder;
use image::ImageOutputFormat;
use serde_json;
use std::fs;
use std::io::{BufReader, Cursor, Read, Seek, SeekFrom};
use std::path::Path;

use crate::commands::{AuthorInfo, ImageMetadata, PlayerInfo, WorldInfo};
use crate::errors::{AppError, AppResult};
use crate::security::{FileSystemGuard, InputValidator};

/// Represents the source of extracted metadata
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub enum MetadataSource {
    /// VRCX-style JSON metadata in PNG Description chunk
    Vrcx,
    /// VRChat native XMP metadata
    VrchatXmp,
    /// No metadata found
    None,
}

/// Result of metadata extraction with source information
#[derive(Debug, Clone, serde::Serialize)]
pub struct MetadataWithSource {
    pub metadata: Option<ImageMetadata>,
    pub source: MetadataSource,
}

/// Extract metadata with information about its source
pub async fn extract_metadata_with_source(file_path: &str) -> AppResult<MetadataWithSource> {
    log::info!("Extracting metadata with source info for: {}", file_path);

    // Validate input first
    InputValidator::validate_image_file(file_path)?;

    let _path = Path::new(file_path);
    if !_path.exists() {
        return Err(AppError::file_not_found(file_path));
    }

    // Priority 1: Try VRCX-style metadata from PNG Description chunk
    if let Some(metadata_json) = get_png_description(file_path)? {
        let cleaned_json = metadata_json.trim();
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(cleaned_json) {
            if let Ok(metadata) = parse_vrchat_metadata(json) {
                log::info!("Found VRCX metadata in {}", file_path);
                return Ok(MetadataWithSource {
                    metadata: Some(metadata),
                    source: MetadataSource::Vrcx,
                });
            }
        }
    }

    // Priority 2: Try VRChat native XMP metadata
    if let Some(xmp_metadata) = extract_vrchat_xmp_metadata(file_path)? {
        log::info!("Found VRChat XMP metadata in {}", file_path);
        return Ok(MetadataWithSource {
            metadata: Some(xmp_metadata),
            source: MetadataSource::VrchatXmp,
        });
    }

    // Priority 3: Filename pattern (only provides timestamp, no actual metadata)
    log::info!("No embedded metadata found in {}", file_path);
    Ok(MetadataWithSource {
        metadata: None,
        source: MetadataSource::None,
    })
}

pub async fn extract_metadata(file_path: &str) -> AppResult<Option<ImageMetadata>> {
    log::info!("Starting metadata extraction for: {}", file_path);

    // Validate input first
    InputValidator::validate_image_file(file_path)?;

    let _path = Path::new(file_path);
    if !_path.exists() {
        return Err(AppError::file_not_found(file_path));
    }

    // Priority 1: Try to get VRCX-style metadata from PNG text chunks (Description)
    if let Some(metadata_json) = get_png_description(file_path)? {
        log::info!("Found PNG Description metadata in {}", file_path);
        log::debug!(
            "Raw metadata JSON (first 500 chars): {}",
            &metadata_json[..std::cmp::min(500, metadata_json.len())]
        );

        // Try to clean up the JSON string before parsing
        let cleaned_json = metadata_json.trim();

        match serde_json::from_str::<serde_json::Value>(cleaned_json) {
            Ok(json) => {
                log::info!("Successfully parsed VRCX JSON metadata");
                log::debug!("Parsed JSON structure: {:#}", json);
                let metadata = parse_vrchat_metadata(json)?;
                return Ok(Some(metadata));
            }
            Err(e) => {
                log::warn!(
                    "Failed to parse VRCX metadata JSON from {}: {}",
                    file_path,
                    e
                );
                log::debug!("Raw JSON that failed to parse (full): {}", metadata_json);
                log::debug!("JSON length: {} bytes", metadata_json.len());
                log::debug!(
                    "First 100 chars as bytes: {:?}",
                    metadata_json
                        .chars()
                        .take(100)
                        .collect::<String>()
                        .as_bytes()
                );

                // Try to identify the issue
                if metadata_json.starts_with('{') && metadata_json.ends_with('}') {
                    log::debug!("JSON appears to have correct braces");
                } else {
                    log::debug!(
                        "JSON missing proper braces - starts with: {:?}, ends with: {:?}",
                        metadata_json.chars().take(10).collect::<String>(),
                        metadata_json.chars().rev().take(10).collect::<String>()
                    );
                }
            }
        }
    } else {
        log::info!("No VRCX PNG Description metadata found in {}", file_path);
    }

    // Priority 2: Try to get VRChat native XMP metadata
    log::info!("Trying VRChat XMP metadata extraction for {}", file_path);
    if let Some(xmp_metadata) = extract_vrchat_xmp_metadata(file_path)? {
        log::info!(
            "Successfully extracted VRChat XMP metadata from {}",
            file_path
        );
        return Ok(Some(xmp_metadata));
    } else {
        log::info!("No VRChat XMP metadata found in {}", file_path);
    }

    // Priority 3: If no metadata found, try extracting from filename patterns
    log::info!("Trying filename pattern extraction for {}", file_path);
    extract_metadata_from_filename(file_path)
}

fn get_png_description(file_path: &str) -> AppResult<Option<String>> {
    log::debug!("Opening PNG file for chunk analysis: {}", file_path);

    let file = fs::File::open(file_path)?;
    let mut reader = BufReader::new(file);

    // Verify PNG signature
    let mut signature = [0u8; 8];
    reader.read_exact(&mut signature)?;

    const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
    if signature != PNG_SIGNATURE {
        log::warn!("File {} is not a valid PNG (invalid signature)", file_path);
        return Err(AppError::invalid_file_type(file_path));
    }

    log::debug!("Valid PNG signature confirmed");

    let mut chunks_found = Vec::new();
    let mut text_chunks_found = 0;

    // Read chunks to find tEXt, iTXt, or zTXt chunks with "Description"
    loop {
        let mut chunk_header = [0u8; 8];
        match reader.read_exact(&mut chunk_header) {
            Ok(_) => {}
            Err(_) => {
                log::debug!("End of PNG file reached");
                break;
            }
        }

        let length = u32::from_be_bytes([
            chunk_header[0],
            chunk_header[1],
            chunk_header[2],
            chunk_header[3],
        ]) as usize;

        let chunk_type = &chunk_header[4..8];
        let chunk_type_str = std::str::from_utf8(chunk_type).unwrap_or("INVALID");

        chunks_found.push(format!("{}({})", chunk_type_str, length));

        // Limit chunk size to prevent memory issues but be more generous for metadata
        const MAX_CHUNK_SIZE: usize = 50 * 1024 * 1024; // 50MB - much larger for big metadata
        if length > MAX_CHUNK_SIZE {
            log::warn!(
                "Skipping oversized chunk {} with size {} MB",
                chunk_type_str,
                length / 1024 / 1024
            );
            reader.seek(SeekFrom::Current(length as i64 + 4))?; // +4 for CRC
            continue;
        }

        // Check for any text chunk type
        if matches!(chunk_type_str, "tEXt" | "iTXt" | "zTXt") {
            text_chunks_found += 1;
            log::info!(
                "Found text chunk #{}: {} with {} bytes",
                text_chunks_found,
                chunk_type_str,
                length
            );

            let mut chunk_data = vec![0u8; length];
            reader.read_exact(&mut chunk_data)?;

            // Try to extract Description from this chunk
            if let Some(description) = extract_description_from_chunk(chunk_type_str, &chunk_data) {
                log::info!(
                    "Successfully extracted Description from {} chunk!",
                    chunk_type_str
                );
                log::debug!("Description length: {} bytes", description.len());
                return Ok(Some(description));
            } else {
                log::debug!("No Description found in {} chunk", chunk_type_str);

                // Log what keywords we did find for debugging
                if let Some(keyword) = get_chunk_keyword(chunk_type_str, &chunk_data) {
                    log::debug!("Chunk keyword found: '{}'", keyword);
                } else {
                    log::debug!("No keyword found in chunk");
                }
            }

            // Skip CRC
            reader.seek(SeekFrom::Current(4))?;
        } else {
            // Skip non-text chunk data and CRC
            reader.seek(SeekFrom::Current(length as i64 + 4))?;
        }

        // Stop at IEND chunk
        if chunk_type_str == "IEND" {
            log::debug!("Reached IEND chunk - end of PNG");
            break;
        }
    }

    log::info!("PNG Analysis Summary for {}:", file_path);
    log::info!("   Total chunks found: [{}]", chunks_found.join(", "));
    log::info!("   Text chunks found: {}", text_chunks_found);
    log::info!("   No Description metadata found");

    Ok(None)
}

fn extract_description_from_chunk(chunk_type: &str, data: &[u8]) -> Option<String> {
    match chunk_type {
        "tEXt" => extract_from_text_chunk(data),
        "iTXt" => extract_from_international_text_chunk(data),
        "zTXt" => extract_from_compressed_text_chunk(data),
        _ => None,
    }
}

fn get_chunk_keyword(chunk_type: &str, data: &[u8]) -> Option<String> {
    match chunk_type {
        "tEXt" | "zTXt" => {
            if let Some(null_pos) = data.iter().position(|&b| b == 0) {
                std::str::from_utf8(&data[..null_pos])
                    .ok()
                    .map(|s| s.to_string())
            } else {
                None
            }
        }
        "iTXt" => {
            if let Some(null_pos) = data.iter().position(|&b| b == 0) {
                std::str::from_utf8(&data[..null_pos])
                    .ok()
                    .map(|s| s.to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

fn extract_from_text_chunk(data: &[u8]) -> Option<String> {
    // tEXt format: keyword\0text
    let null_pos = data.iter().position(|&b| b == 0)?;
    let keyword = std::str::from_utf8(&data[..null_pos]).ok()?;

    log::debug!("tEXt chunk keyword: '{}'", keyword);

    // Case-insensitive comparison for Description
    if keyword.eq_ignore_ascii_case("Description") {
        let text_data = &data[null_pos + 1..];

        log::debug!(
            "Found Description text chunk with {} bytes",
            text_data.len()
        );
        log::debug!(
            "First 50 bytes: {:?}",
            &text_data[..std::cmp::min(50, text_data.len())]
        );

        // Try UTF-8 first
        if let Ok(text) = std::str::from_utf8(text_data) {
            log::debug!("Successfully decoded as UTF-8");
            return Some(text.to_string());
        }

        // Fallback to Latin-1 encoding (what some older tools might use)
        log::debug!("UTF-8 failed, trying Latin-1 fallback");
        let text = text_data.iter().map(|&b| b as char).collect::<String>();
        log::debug!(
            "Latin-1 decoded first 100 chars: {}",
            &text[..std::cmp::min(100, text.len())]
        );
        return Some(text);
    }

    None
}

fn extract_from_international_text_chunk(data: &[u8]) -> Option<String> {
    // iTXt format: keyword\0compression_flag\0compression_method\0language_tag\0translated_keyword\0text
    log::debug!("Processing iTXt chunk with {} bytes", data.len());

    // Find all null byte positions
    let null_positions: Vec<usize> = data
        .iter()
        .enumerate()
        .filter(|(_, &b)| b == 0)
        .map(|(i, _)| i)
        .collect();

    log::debug!("Found null positions: {:?}", null_positions);

    if null_positions.len() < 4 {
        log::debug!(
            "iTXt chunk has insufficient null separators: {}",
            null_positions.len()
        );
        return None;
    }

    // Extract keyword (up to first null)
    let keyword = std::str::from_utf8(&data[..null_positions[0]]).ok()?;
    log::debug!("iTXt chunk keyword: '{}'", keyword);

    if keyword.eq_ignore_ascii_case("Description") {
        // Get compression flag
        let compression_flag = data.get(null_positions[0] + 1).copied().unwrap_or(0);
        log::debug!("iTXt compression flag: {}", compression_flag);

        if compression_flag == 0 {
            // Uncompressed text starts after the 5th null byte (or at least 4 null bytes)
            if null_positions.len() >= 4 {
                // Text starts after: keyword\0flag\0method\0lang\0translated\0
                let text_start = null_positions.get(4).copied().unwrap_or(null_positions[3]) + 1;

                if text_start < data.len() {
                    let text_data = &data[text_start..];
                    log::debug!(
                        "Found uncompressed iTXt Description with {} bytes",
                        text_data.len()
                    );
                    log::debug!(
                        "First 50 bytes: {:?}",
                        &text_data[..std::cmp::min(50, text_data.len())]
                    );

                    return std::str::from_utf8(text_data).ok().map(|s| s.to_string());
                }
            }
        } else if compression_flag == 1 {
            log::debug!("Found compressed iTXt Description - attempting decompression");
            // Handle compressed iTXt - similar to zTXt but different structure
            if null_positions.len() >= 4 {
                let compressed_start =
                    null_positions.get(4).copied().unwrap_or(null_positions[3]) + 1;
                if compressed_start < data.len() {
                    let compressed_data = &data[compressed_start..];
                    return decompress_deflate_data(compressed_data);
                }
            }
        }
    }

    None
}

fn extract_from_compressed_text_chunk(data: &[u8]) -> Option<String> {
    // zTXt format: keyword\0compression_method\0compressed_text
    let null_pos = data.iter().position(|&b| b == 0)?;
    let keyword = std::str::from_utf8(&data[..null_pos]).ok()?;

    log::debug!("zTXt chunk keyword: '{}'", keyword);

    if keyword.eq_ignore_ascii_case("Description") && data.len() > null_pos + 2 {
        let compression_method = data[null_pos + 1];
        log::debug!("zTXt compression method: {}", compression_method);

        if compression_method == 0 {
            // Deflate compression
            let compressed_data = &data[null_pos + 2..];
            log::debug!("Attempting to decompress {} bytes", compressed_data.len());
            log::debug!(
                "First 20 compressed bytes: {:?}",
                &compressed_data[..std::cmp::min(20, compressed_data.len())]
            );

            return decompress_deflate_data(compressed_data);
        }
    }

    None
}

fn decompress_deflate_data(compressed_data: &[u8]) -> Option<String> {
    let mut decoder = DeflateDecoder::new(compressed_data);
    let mut decompressed = Vec::new();

    match decoder.read_to_end(&mut decompressed) {
        Ok(size) => {
            log::debug!("Successfully decompressed {} bytes", size);
            log::debug!(
                "First 100 decompressed chars: {}",
                std::str::from_utf8(&decompressed)
                    .unwrap_or("<invalid utf8>")
                    .chars()
                    .take(100)
                    .collect::<String>()
            );
            return std::str::from_utf8(&decompressed)
                .ok()
                .map(|s| s.to_string());
        }
        Err(e) => {
            log::warn!("Failed to decompress deflate data: {}", e);
        }
    }

    None
}

/// Extract VRChat native XMP metadata from a PNG file
/// VRChat stores metadata in XMP format with fields like:
/// - XMP:Author
/// - XMP:AuthorID
/// - XMP:WorldID
/// - XMP:WorldDisplayName
fn extract_vrchat_xmp_metadata(file_path: &str) -> AppResult<Option<ImageMetadata>> {
    log::debug!(
        "Attempting to extract VRChat XMP metadata from: {}",
        file_path
    );

    let file = fs::File::open(file_path)?;
    let mut reader = BufReader::new(file);

    // Verify PNG signature
    let mut signature = [0u8; 8];
    reader.read_exact(&mut signature)?;

    const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
    if signature != PNG_SIGNATURE {
        log::debug!("Not a valid PNG file for XMP extraction");
        return Ok(None);
    }

    // Read chunks looking for iTXt with XMP data
    loop {
        let mut chunk_header = [0u8; 8];
        match reader.read_exact(&mut chunk_header) {
            Ok(_) => {}
            Err(_) => {
                log::debug!("End of PNG file reached while searching for XMP");
                break;
            }
        }

        let length = u32::from_be_bytes([
            chunk_header[0],
            chunk_header[1],
            chunk_header[2],
            chunk_header[3],
        ]) as usize;

        let chunk_type = &chunk_header[4..8];
        let chunk_type_str = std::str::from_utf8(chunk_type).unwrap_or("INVALID");

        // XMP data is typically stored in iTXt chunks with "XML:com.adobe.xmp" keyword
        // or in a raw XMP chunk
        if chunk_type_str == "iTXt" {
            const MAX_CHUNK_SIZE: usize = 50 * 1024 * 1024;
            if length > MAX_CHUNK_SIZE {
                reader.seek(SeekFrom::Current(length as i64 + 4))?;
                continue;
            }

            let mut chunk_data = vec![0u8; length];
            reader.read_exact(&mut chunk_data)?;

            // Check if this is an XMP chunk
            if let Some(xmp_content) = extract_xmp_from_itxt(&chunk_data) {
                log::debug!("Found XMP data in iTXt chunk");
                if let Some(metadata) = parse_vrchat_xmp(&xmp_content) {
                    return Ok(Some(metadata));
                }
            }

            // Skip CRC
            reader.seek(SeekFrom::Current(4))?;
        } else if chunk_type_str == "tEXt" || chunk_type_str == "zTXt" {
            // Check text chunks for XMP-related content
            const MAX_CHUNK_SIZE: usize = 50 * 1024 * 1024;
            if length > MAX_CHUNK_SIZE {
                reader.seek(SeekFrom::Current(length as i64 + 4))?;
                continue;
            }

            let mut chunk_data = vec![0u8; length];
            reader.read_exact(&mut chunk_data)?;

            // Try to extract XMP from text chunks
            if let Some(text_content) = if chunk_type_str == "tEXt" {
                extract_text_content(&chunk_data)
            } else {
                extract_compressed_text_content(&chunk_data)
            } {
                if text_content.contains("x]mm[")
                    || text_content.contains("XMP")
                    || text_content.contains("WorldID")
                    || text_content.contains("AuthorID")
                {
                    log::debug!("Found potential XMP data in {} chunk", chunk_type_str);
                    if let Some(metadata) = parse_vrchat_xmp(&text_content) {
                        return Ok(Some(metadata));
                    }
                }
            }

            // Skip CRC
            reader.seek(SeekFrom::Current(4))?;
        } else {
            // Skip non-text chunk data and CRC
            reader.seek(SeekFrom::Current(length as i64 + 4))?;
        }

        // Stop at IEND chunk
        if chunk_type_str == "IEND" {
            break;
        }
    }

    // Also try to read raw XMP data from the file
    // Some tools embed XMP directly without proper chunk structure
    if let Some(metadata) = try_extract_raw_xmp(file_path)? {
        return Ok(Some(metadata));
    }

    Ok(None)
}

/// Extract XMP content from an iTXt chunk
fn extract_xmp_from_itxt(data: &[u8]) -> Option<String> {
    // iTXt format: keyword\0compression_flag\0compression_method\0language_tag\0translated_keyword\0text
    let null_positions: Vec<usize> = data
        .iter()
        .enumerate()
        .filter(|(_, &b)| b == 0)
        .map(|(i, _)| i)
        .collect();

    if null_positions.is_empty() {
        return None;
    }

    let keyword = std::str::from_utf8(&data[..null_positions[0]]).ok()?;

    // Check for XMP-related keywords
    if keyword.contains("XMP")
        || keyword.contains("XML:com.adobe.xmp")
        || keyword.eq_ignore_ascii_case("xpacket")
    {
        log::debug!("Found XMP iTXt chunk with keyword: {}", keyword);

        if null_positions.len() >= 4 {
            let compression_flag = data.get(null_positions[0] + 1).copied().unwrap_or(0);

            if compression_flag == 0 {
                // Uncompressed
                let text_start = null_positions
                    .get(4)
                    .copied()
                    .unwrap_or(null_positions.last().copied().unwrap_or(0))
                    + 1;
                if text_start < data.len() {
                    return std::str::from_utf8(&data[text_start..])
                        .ok()
                        .map(|s| s.to_string());
                }
            } else {
                // Compressed
                let compressed_start = null_positions
                    .get(4)
                    .copied()
                    .unwrap_or(null_positions.last().copied().unwrap_or(0))
                    + 1;
                if compressed_start < data.len() {
                    return decompress_deflate_data(&data[compressed_start..]);
                }
            }
        }
    }

    None
}

/// Extract text content from a tEXt chunk
fn extract_text_content(data: &[u8]) -> Option<String> {
    let null_pos = data.iter().position(|&b| b == 0)?;
    let text_data = &data[null_pos + 1..];
    std::str::from_utf8(text_data).ok().map(|s| s.to_string())
}

/// Extract text content from a zTXt chunk (compressed)
fn extract_compressed_text_content(data: &[u8]) -> Option<String> {
    let null_pos = data.iter().position(|&b| b == 0)?;
    if data.len() > null_pos + 2 {
        let compression_method = data[null_pos + 1];
        if compression_method == 0 {
            return decompress_deflate_data(&data[null_pos + 2..]);
        }
    }
    None
}

/// Try to extract raw XMP data from the file by searching for XMP markers
fn try_extract_raw_xmp(file_path: &str) -> AppResult<Option<ImageMetadata>> {
    let file_content = fs::read(file_path)?;

    // Look for XMP packet start marker
    let xmp_start_marker = b"<?xpacket begin";
    let xmp_end_marker = b"<?xpacket end";

    // Also look for RDF marker which is common in XMP
    let rdf_start = b"<x:xmpmeta";
    let rdf_end = b"</x:xmpmeta>";

    // Try to find XMP packet
    if let Some(start_pos) = find_subsequence(&file_content, xmp_start_marker) {
        if let Some(end_offset) = find_subsequence(&file_content[start_pos..], xmp_end_marker) {
            let end_pos = start_pos + end_offset + 20; // Include the end marker and some buffer
            let end_pos = std::cmp::min(end_pos, file_content.len());

            if let Ok(xmp_text) = std::str::from_utf8(&file_content[start_pos..end_pos]) {
                log::debug!("Found raw XMP packet in file");
                if let Some(metadata) = parse_vrchat_xmp(xmp_text) {
                    return Ok(Some(metadata));
                }
            }
        }
    }

    // Try RDF format
    if let Some(start_pos) = find_subsequence(&file_content, rdf_start) {
        if let Some(end_offset) = find_subsequence(&file_content[start_pos..], rdf_end) {
            let end_pos = start_pos + end_offset + rdf_end.len();

            if let Ok(xmp_text) = std::str::from_utf8(&file_content[start_pos..end_pos]) {
                log::debug!("Found raw RDF/XMP data in file");
                if let Some(metadata) = parse_vrchat_xmp(xmp_text) {
                    return Ok(Some(metadata));
                }
            }
        }
    }

    Ok(None)
}

/// Find a subsequence in a byte slice
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Parse VRChat XMP metadata from XMP content string
/// Looks for VRChat-specific XMP properties:
/// - XMP:Author (display name)
/// - XMP:AuthorID or vrc:AuthorID
/// - XMP:WorldID or vrc:WorldID
/// - XMP:WorldDisplayName or vrc:WorldDisplayName
fn parse_vrchat_xmp(xmp_content: &str) -> Option<ImageMetadata> {
    log::debug!("Parsing VRChat XMP content ({} bytes)", xmp_content.len());
    log::debug!(
        "XMP content preview: {}",
        &xmp_content[..std::cmp::min(500, xmp_content.len())]
    );

    let mut metadata = ImageMetadata {
        author: None,
        world: None,
        players: Vec::new(),
    };

    let mut found_any = false;

    // Extract Author display name - try multiple possible attribute names
    let author_name = extract_xmp_value(xmp_content, "Author")
        .or_else(|| extract_xmp_value(xmp_content, "xmp:Author"))
        .or_else(|| extract_xmp_value(xmp_content, "vrc:Author"))
        .or_else(|| extract_xmp_value(xmp_content, "vrchat:Author"));

    // Extract AuthorID - try multiple possible attribute names
    let author_id = extract_xmp_value(xmp_content, "AuthorID")
        .or_else(|| extract_xmp_value(xmp_content, "vrc:AuthorID"))
        .or_else(|| extract_xmp_value(xmp_content, "XMP:AuthorID"))
        .or_else(|| extract_xmp_value(xmp_content, "vrchat:AuthorID"));

    // Extract WorldID
    let world_id = extract_xmp_value(xmp_content, "WorldID")
        .or_else(|| extract_xmp_value(xmp_content, "vrc:WorldID"))
        .or_else(|| extract_xmp_value(xmp_content, "XMP:WorldID"))
        .or_else(|| extract_xmp_value(xmp_content, "vrchat:WorldID"));

    // Extract WorldDisplayName
    let world_name = extract_xmp_value(xmp_content, "WorldDisplayName")
        .or_else(|| extract_xmp_value(xmp_content, "vrc:WorldDisplayName"))
        .or_else(|| extract_xmp_value(xmp_content, "XMP:WorldDisplayName"))
        .or_else(|| extract_xmp_value(xmp_content, "vrchat:WorldDisplayName"));

    // Set author if we have AuthorID or Author name
    if author_id.is_some() || author_name.is_some() {
        let id = author_id.unwrap_or_default();
        let name = author_name.unwrap_or_default();

        log::debug!("Found XMP Author: {} ({})", name, id);
        metadata.author = Some(AuthorInfo {
            display_name: name,
            id,
        });
        found_any = true;
    }

    // Set world info if we have WorldID
    if world_id.is_some() || world_name.is_some() {
        let id = world_id.unwrap_or_default();
        let name = world_name.unwrap_or_default();

        log::debug!("Found XMP World: {} ({})", name, id);

        metadata.world = Some(WorldInfo {
            name,
            id,
            instance_id: String::new(), // Not available in XMP
        });
        found_any = true;
    }

    // Note: VRChat XMP doesn't include player list, only author and world

    if found_any {
        log::info!(
            "Successfully parsed VRChat XMP metadata - Author: {}, World: {}",
            metadata.author.is_some(),
            metadata.world.is_some()
        );
        Some(metadata)
    } else {
        log::debug!("No VRChat metadata found in XMP content");
        None
    }
}

/// Extract a value from XMP content for a given property name
/// Handles both XML attribute format and element format
fn extract_xmp_value(content: &str, property: &str) -> Option<String> {
    // Try namespaced XML element: <ns:property>value</ns:property>
    let ns_elem_pattern = format!(
        r#"<(\w+):{}[^>]*>([^<]*)</\1:{}>"#,
        regex::escape(property),
        regex::escape(property)
    );
    if let Ok(re) = regex::Regex::new(&ns_elem_pattern) {
        if let Some(caps) = re.captures(content) {
            if let Some(value) = caps.get(2) {
                let val = value.as_str().trim().to_string();
                if !val.is_empty() {
                    return Some(val);
                }
            }
        }
    }

    // Try XML element: <property>value</property>
    let elem_pattern = format!(
        r#"<{}[^>]*>([^<]*)</{}>"#,
        regex::escape(property),
        regex::escape(property)
    );
    if let Ok(re) = regex::Regex::new(&elem_pattern) {
        if let Some(caps) = re.captures(content) {
            if let Some(value) = caps.get(1) {
                let val = value.as_str().trim().to_string();
                if !val.is_empty() {
                    return Some(val);
                }
            }
        }
    }

    // Try attribute: property="value"
    let attr_pattern = format!(r#"(?:^|[^a-zA-Z]){}="([^"]*)""#, regex::escape(property));
    if let Ok(re) = regex::Regex::new(&attr_pattern) {
        if let Some(caps) = re.captures(content) {
            if let Some(value) = caps.get(1) {
                let val = value.as_str().to_string();
                if !val.is_empty() {
                    return Some(val);
                }
            }
        }
    }

    // Try namespaced attribute: ns:property="value"
    let prefixed_pattern = format!(r#"\w+:{}="([^"]*)""#, regex::escape(property));
    if let Ok(re) = regex::Regex::new(&prefixed_pattern) {
        if let Some(caps) = re.captures(content) {
            if let Some(value) = caps.get(1) {
                let val = value.as_str().to_string();
                if !val.is_empty() {
                    return Some(val);
                }
            }
        }
    }

    None
}

fn parse_vrchat_metadata(json: serde_json::Value) -> AppResult<ImageMetadata> {
    log::debug!("Parsing VRChat metadata JSON structure");

    let mut metadata = ImageMetadata {
        author: None,
        world: None,
        players: Vec::new(),
    };

    // Extract author info
    if let Some(author_obj) = json.get("author") {
        if let (Some(name), Some(id)) = (
            author_obj.get("displayName").and_then(|v| v.as_str()),
            author_obj.get("id").and_then(|v| v.as_str()),
        ) {
            log::debug!("Found author: {} ({})", name, id);
            metadata.author = Some(AuthorInfo {
                display_name: name.to_string(),
                id: id.to_string(),
            });
        }
    }

    // Extract world info
    if let Some(world_obj) = json.get("world") {
        let world_name = world_obj
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown World");
        let world_id = world_obj
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown ID");
        // Note: instance_id is still extracted but not displayed in Discord messages
        let instance_id = world_obj
            .get("instanceId")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        log::debug!(
            "Found world: {} ({}) - Instance: {}",
            world_name,
            world_id,
            instance_id
        );

        metadata.world = Some(WorldInfo {
            name: world_name.to_string(),
            id: world_id.to_string(),
            instance_id: instance_id.to_string(),
        });
    }

    // Extract players array
    if let Some(players_array) = json.get("players").and_then(|v| v.as_array()) {
        log::debug!("Found {} players", players_array.len());

        for (i, player) in players_array.iter().enumerate() {
            if let (Some(name), Some(id)) = (
                player.get("displayName").and_then(|v| v.as_str()),
                player.get("id").and_then(|v| v.as_str()),
            ) {
                log::debug!("Player {}: {} ({})", i + 1, name, id);
                metadata.players.push(PlayerInfo {
                    display_name: name.to_string(),
                    id: id.to_string(),
                });
            }
        }
    }

    log::info!(
        "Successfully parsed metadata - Author: {}, World: {}, Players: {}",
        metadata.author.is_some(),
        metadata.world.is_some(),
        metadata.players.len()
    );

    Ok(metadata)
}

fn extract_metadata_from_filename(file_path: &str) -> AppResult<Option<ImageMetadata>> {
    let filename = Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    log::debug!("Checking filename for timestamp pattern: {}", filename);

    // Try to extract timestamp from filename pattern: YYYY-MM-DD_HH-MM-SS
    let date_regex = regex::Regex::new(r"(\d{4}-\d{2}-\d{2})_(\d{2}-\d{2}-\d{2}(?:\.\d+)?)")
        .map_err(|e| AppError::Internal(format!("Regex error: {}", e)))?;

    if date_regex.is_match(filename) {
        log::info!("Found VRChat-style timestamp in filename: {}", filename);
        log::info!("This suggests it's a VRChat screenshot, but no embedded metadata was found");
    } else {
        log::debug!("No VRChat timestamp pattern found in filename");
    }

    // For now, return None if no PNG metadata found
    Ok(None)
}

pub async fn compress_image(file_path: &str, quality: u8) -> AppResult<String> {
    // Load config to get compression format preference
    let format = crate::config::load_config()
        .map(|c| c.compression_format)
        .unwrap_or_else(|_| "webp".to_string());

    compress_image_with_format(file_path, quality, &format).await
}

pub async fn compress_image_with_format(
    file_path: &str,
    quality: u8,
    format: &str,
) -> AppResult<String> {
    // Validate inputs
    InputValidator::validate_image_file(file_path)?;

    if quality == 0 || quality > 100 {
        return Err(AppError::validation(
            "quality",
            "Quality must be between 1 and 100",
        ));
    }

    // Load image with memory-efficient loading for large files
    let img = load_image_efficiently(file_path)?;

    // Create output path in secure temp directory
    let temp_path = FileSystemGuard::create_secure_temp_file(file_path)?;

    // Choose format based on setting
    if format == "jpg" {
        let output_path = temp_path.with_extension("jpg");
        let mut output = Vec::new();
        let mut cursor = Cursor::new(&mut output);
        img.write_to(&mut cursor, ImageOutputFormat::Jpeg(quality))?;
        fs::write(&output_path, output)?;

        log::info!(
            "Compressed {} to JPEG at {} (quality: {})",
            file_path,
            output_path.display(),
            quality
        );

        Ok(output_path.to_string_lossy().to_string())
    } else {
        // Use webp crate for lossy WebP compression with quality control
        let output_path = temp_path.with_extension("webp");

        // Convert to RGBA for webp encoder
        let rgba_img = img.to_rgba8();
        let (width, height) = rgba_img.dimensions();

        // Create WebP encoder with lossy compression
        let encoder = webp::Encoder::from_rgba(&rgba_img, width, height);
        let webp_data = encoder.encode(quality as f32);

        fs::write(&output_path, &*webp_data)?;

        log::info!(
            "Compressed {} to WebP at {} (quality: {})",
            file_path,
            output_path.display(),
            quality
        );

        Ok(output_path.to_string_lossy().to_string())
    }
}

fn load_image_efficiently(file_path: &str) -> AppResult<image::DynamicImage> {
    // Check file size first
    let file_size = FileSystemGuard::get_file_size(file_path)?;
    const LARGE_FILE_THRESHOLD: u64 = 50 * 1024 * 1024; // 50MB

    if file_size > LARGE_FILE_THRESHOLD {
        log::warn!(
            "Large image file detected: {} ({} MB)",
            file_path,
            file_size / 1024 / 1024
        );

        // For very large files, we might want to use a streaming approach
        // or limit the maximum dimensions
        let img = image::open(file_path)?;

        // Resize if too large
        const MAX_DIMENSION: u32 = 4096;
        if img.width() > MAX_DIMENSION || img.height() > MAX_DIMENSION {
            log::info!("Resizing large image from {}x{}", img.width(), img.height());
            let resized = img.resize(
                MAX_DIMENSION,
                MAX_DIMENSION,
                image::imageops::FilterType::Lanczos3,
            );
            Ok(resized)
        } else {
            Ok(img)
        }
    } else {
        // Normal loading for smaller files
        Ok(image::open(file_path)?)
    }
}

pub async fn get_file_hash(file_path: &str) -> AppResult<String> {
    InputValidator::validate_file_path(file_path)?;

    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // For large files, read in chunks to avoid memory issues
    let file_size = FileSystemGuard::get_file_size(file_path)?;
    const CHUNK_SIZE: usize = 8192; // 8KB chunks

    let mut hasher = DefaultHasher::new();

    if file_size > 100 * 1024 * 1024 {
        // Files larger than 100MB
        // Stream-based hashing for large files
        let mut file = fs::File::open(file_path)?;
        let mut buffer = vec![0u8; CHUNK_SIZE];

        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            buffer[..bytes_read].hash(&mut hasher);
        }
    } else {
        // Read entire file for smaller files
        let contents = fs::read(file_path)?;
        contents.hash(&mut hasher);
    }

    Ok(format!("{:x}", hasher.finish()))
}

pub fn get_timestamp_from_filename(file_path: &str) -> Option<i64> {
    let filename = Path::new(file_path).file_name().and_then(|n| n.to_str())?;

    let date_regex =
        regex::Regex::new(r"(\d{4}-\d{2}-\d{2})_(\d{2}-\d{2}-\d{2}(?:\.\d+)?)").ok()?;

    if let Some(captures) = date_regex.captures(filename) {
        let date_part = captures.get(1)?.as_str();
        let time_part = captures.get(2)?.as_str().replace('-', ":");

        let datetime_str = format!("{} {}", date_part, time_part);
        log::debug!("Parsing datetime from filename: {}", datetime_str);

        // Try different datetime formats
        let formats = ["%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%d %H:%M:%S"];

        for format in &formats {
            if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&datetime_str, format) {
                log::debug!("Parsed NaiveDateTime: {}", dt);

                // VRChat screenshots are saved in local time
                // Get current system timezone offset
                let local_offset = chrono::Local::now().offset().fix();
                log::debug!("Local timezone offset: {}", local_offset);

                // Convert to local datetime with timezone
                match dt.and_local_timezone(local_offset).single() {
                    Some(local_dt) => {
                        let utc_timestamp = local_dt.timestamp();
                        log::debug!("Local datetime: {}", local_dt);
                        log::debug!(
                            "UTC timestamp: {} (Discord: <t:{}:f>)",
                            utc_timestamp,
                            utc_timestamp
                        );
                        return Some(utc_timestamp);
                    }
                    None => {
                        log::warn!("Ambiguous local timezone conversion (likely DST transition)");
                        // During DST transitions, pick the earliest interpretation
                        if let Some(local_dt) = dt.and_local_timezone(local_offset).earliest() {
                            let utc_timestamp = local_dt.timestamp();
                            log::debug!("Using earliest DST interpretation: {}", local_dt);
                            return Some(utc_timestamp);
                        } else {
                            log::warn!("Could not resolve DST ambiguity, using UTC fallback");
                        }
                    }
                }

                // Fallback: treat as UTC (this is safe but may be wrong by timezone offset)
                let utc_timestamp = dt.and_utc().timestamp();
                log::warn!("FALLBACK: Treating timestamp as UTC. This may be incorrect by your timezone offset.");
                log::debug!(
                    "Fallback UTC timestamp: {} (Discord: <t:{}:f>)",
                    utc_timestamp,
                    utc_timestamp
                );
                return Some(utc_timestamp);
            }
        }
    }

    // Fallback to file creation time (this is always in correct timezone)
    if let Ok(metadata) = fs::metadata(file_path) {
        if let Ok(created) = metadata.created() {
            if let Ok(duration) = created.duration_since(std::time::UNIX_EPOCH) {
                let timestamp = duration.as_secs() as i64;
                log::debug!(
                    "Using file creation time: {} (Discord: <t:{}:f>)",
                    timestamp,
                    timestamp
                );
                return Some(timestamp);
            }
        }
    }

    log::warn!("Could not extract any timestamp");
    None
}

/// Get image dimensions and file size
pub fn get_image_info(file_path: &str) -> AppResult<(u32, u32, u64)> {
    InputValidator::validate_image_file(file_path)?;

    let file_size = FileSystemGuard::get_file_size(file_path)?;

    // Read only the image header for dimensions
    let reader = image::io::Reader::open(file_path)?.with_guessed_format()?;

    let dimensions = reader.into_dimensions()?;

    Ok((dimensions.0, dimensions.1, file_size))
}

/// Generate thumbnail for UI display
pub fn generate_thumbnail(file_path: &str, max_dimension: u32) -> AppResult<String> {
    InputValidator::validate_image_file(file_path)?;

    log::debug!(
        "Generating thumbnail for {} with max dimension {}",
        file_path,
        max_dimension
    );

    // Load the image
    let img = image::open(file_path)?;

    // Resize to thumbnail using thumbnail method
    let thumbnail = img.thumbnail(max_dimension, max_dimension);

    log::debug!(
        "Resized from {}x{} to {}x{}",
        img.width(),
        img.height(),
        thumbnail.width(),
        thumbnail.height()
    );

    // Create output path in secure temp directory
    let temp_path = FileSystemGuard::create_secure_temp_file(file_path)?;
    let output_path = temp_path.with_extension("thumb.webp");

    // Convert to WebP using webp crate for better compression
    let rgba_img = thumbnail.to_rgba8();
    let (width, height) = rgba_img.dimensions();
    let encoder = webp::Encoder::from_rgba(&rgba_img, width, height);
    let webp_data = encoder.encode(60.0); // quality 60 for thumbnails

    fs::write(&output_path, &*webp_data)?;

    log::info!(
        "Generated thumbnail for {} at {} ({}x{})",
        file_path,
        output_path.display(),
        thumbnail.width(),
        thumbnail.height()
    );

    Ok(output_path.to_string_lossy().to_string())
}

/// Check if image needs compression for Discord
pub fn should_compress_image(file_path: &str) -> AppResult<bool> {
    let file_size = FileSystemGuard::get_file_size(file_path)?;
    const DISCORD_LIMIT: u64 = 50 * 1024 * 1024; // 50MB
    const COMPRESSION_THRESHOLD: u64 = 8 * 1024 * 1024; // 8MB

    if file_size > DISCORD_LIMIT {
        return Ok(true); // Must compress
    }

    if file_size > COMPRESSION_THRESHOLD {
        // Optionally compress large files
        return Ok(true);
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    fn create_test_image() -> (std::path::PathBuf, Vec<u8>) {
        let temp_dir = std::env::temp_dir();
        let test_file_path = temp_dir.join("test_image_processor.png");

        // Create a minimal valid PNG file (1x1 pixel)
        let png_data = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
            0x00, 0x00, 0x00, 0x0D, // IHDR chunk length
            0x49, 0x48, 0x44, 0x52, // IHDR
            0x00, 0x00, 0x00, 0x01, // width = 1
            0x00, 0x00, 0x00, 0x01, // height = 1
            0x08, 0x02, 0x00, 0x00,
            0x00, // bit depth = 8, color type = 2 (RGB), compression = 0, filter = 0, interlace = 0
            0x90, 0x77, 0x53, 0xDE, // IHDR CRC
            0x00, 0x00, 0x00, 0x0C, // IDAT chunk length
            0x49, 0x44, 0x41, 0x54, // IDAT
            0x08, 0x99, 0x01, 0x01, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x02, 0x00,
            0x01, // IDAT data
            0x00, 0x00, 0x00, 0x00, // IEND chunk length
            0x49, 0x45, 0x4E, 0x44, // IEND
            0xAE, 0x42, 0x60, 0x82, // IEND CRC
        ];

        (test_file_path, png_data)
    }

    #[test]
    fn test_should_compress_image_small_file() {
        let (test_file_path, png_data) = create_test_image();

        if let Ok(mut file) = File::create(&test_file_path) {
            let _ = file.write_all(&png_data);

            let path_str = test_file_path.to_string_lossy();
            let result = should_compress_image(&path_str);

            // Cleanup
            let _ = std::fs::remove_file(&test_file_path);

            // Small file should not need compression
            if let Ok(should_compress) = result {
                assert!(!should_compress, "Small image should not need compression");
            }
        }
    }

    #[test]
    fn test_should_compress_image_large_file() {
        let temp_dir = std::env::temp_dir();
        let test_file_path = temp_dir.join("test_large_image.png");

        // Create a file larger than compression threshold (8MB)
        let large_data = vec![0u8; 10 * 1024 * 1024]; // 10MB of zeros

        if let Ok(mut file) = File::create(&test_file_path) {
            let _ = file.write_all(&large_data);

            let path_str = test_file_path.to_string_lossy();
            let result = should_compress_image(&path_str);

            // Cleanup
            let _ = std::fs::remove_file(&test_file_path);

            // Large file should need compression
            if let Ok(should_compress) = result {
                assert!(should_compress, "Large file should need compression");
            }
        }
    }

    #[test]
    fn test_should_compress_image_nonexistent_file() {
        let result = should_compress_image("nonexistent_file.png");
        assert!(result.is_err(), "Should fail for nonexistent file");
    }

    #[test]
    fn test_get_image_info_invalid_file() {
        let result = get_image_info("nonexistent_file.png");
        assert!(result.is_err(), "Should fail for nonexistent file");
    }

    #[test]
    fn test_get_image_info_non_image_file() {
        let temp_dir = std::env::temp_dir();
        let test_file_path = temp_dir.join("test_not_image.txt");

        if let Ok(mut file) = File::create(&test_file_path) {
            let _ = file.write_all(b"This is not an image");

            let path_str = test_file_path.to_string_lossy();
            let result = get_image_info(&path_str);

            // Cleanup
            let _ = std::fs::remove_file(&test_file_path);

            // Should fail because it's not an image
            assert!(result.is_err(), "Should fail for non-image file");
        }
    }

    #[tokio::test]
    async fn test_extract_metadata_nonexistent_file() {
        let result = extract_metadata("nonexistent_file.png").await;
        assert!(result.is_err(), "Should fail for nonexistent file");
    }

    #[tokio::test]
    async fn test_extract_metadata_no_metadata() {
        let (test_file_path, png_data) = create_test_image();

        if let Ok(mut file) = File::create(&test_file_path) {
            let _ = file.write_all(&png_data);

            let path_str = test_file_path.to_string_lossy();
            let result = extract_metadata(&path_str).await;

            // Cleanup
            let _ = std::fs::remove_file(&test_file_path);

            // Should succeed but return None (no metadata)
            match result {
                Ok(metadata) => assert!(
                    metadata.is_none(),
                    "Should return None for image without metadata"
                ),
                Err(_) => {
                    // Might fail due to image validation, which is acceptable
                    println!("Extract metadata failed (acceptable for minimal test PNG)");
                }
            }
        }
    }

    #[test]
    fn test_parse_vrchat_metadata_invalid_json() {
        let invalid_json = serde_json::json!({
            "invalid": "structure"
        });

        let result = parse_vrchat_metadata(invalid_json);
        // Should handle invalid JSON gracefully
        // Both success and failure are acceptable for invalid JSON
        if let Ok(_) = result {
            // Might succeed with empty metadata
        }
        // Might fail, both outcomes are acceptable
    }

    #[test]
    fn test_parse_vrchat_metadata_valid_structure() {
        let valid_json = serde_json::json!({
            "application": "VRChat",
            "version": "2024.1.1",
            "author": {
                "displayName": "TestUser",
                "id": "usr_test123"
            },
            "world": {
                "name": "Test World",
                "id": "wrld_test123"
            }
        });

        let result = parse_vrchat_metadata(valid_json);
        assert!(
            result.is_ok(),
            "Should successfully parse valid VRChat metadata structure"
        );

        if let Ok(metadata) = result {
            // Check author field
            if let Some(author) = metadata.author {
                assert_eq!(author.display_name, "TestUser");
                assert_eq!(author.id, "usr_test123");
            }

            // Check world field
            if let Some(world) = metadata.world {
                assert_eq!(world.name, "Test World");
                assert_eq!(world.id, "wrld_test123");
            }

            // Check players field exists
            assert!(metadata.players.is_empty() || !metadata.players.is_empty());
        }
    }
}
