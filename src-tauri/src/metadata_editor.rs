use serde_json;
use std::fs;
use std::path::Path;

use crate::commands::ImageMetadata;
use crate::errors::{AppError, AppResult};
use crate::security::InputValidator;

pub async fn embed_metadata(file_path: &str, metadata: ImageMetadata) -> AppResult<String> {
    // Validate input
    InputValidator::validate_image_file(file_path)?;

    let path = Path::new(file_path);
    if !path.exists() {
        return Err(AppError::file_not_found(file_path));
    }

    // Create VRChat-compatible metadata JSON
    let vrchat_metadata = create_vrchat_metadata_json(&metadata)?;

    // Load the original image
    let img = image::open(path)?;

    // Create output filename with _Modified suffix like Python version
    let parent = path.parent().unwrap_or(Path::new("."));
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let extension = path.extension().unwrap_or_default().to_string_lossy();
    let output_path = parent.join(format!("{}_Modified.{}", stem, extension));

    // Check if output file already exists and try to remove it
    if output_path.exists() {
        log::info!(
            "Output file already exists, attempting to remove: {}",
            output_path.display()
        );
        match std::fs::remove_file(&output_path) {
            Ok(_) => log::info!("Successfully removed existing file"),
            Err(e) => {
                log::error!("Failed to remove existing file: {}", e);
                return Err(AppError::Io(e));
            }
        }
    }

    // Check if parent directory is writable
    if let Err(e) = std::fs::metadata(parent) {
        log::error!("Cannot access parent directory: {}", e);
        return Err(AppError::Io(e));
    }

    log::info!(
        "Attempting to save PNG with metadata to: {}",
        output_path.display()
    );

    // Save PNG with metadata
    save_png_with_metadata(&img, &output_path, &vrchat_metadata)?;

    // Note: We don't preserve file timestamps since we use filename-based timestamps from VRChat naming convention
    log::info!(
        "Embedded metadata in {} -> {}",
        file_path,
        output_path.display()
    );

    Ok(output_path.to_string_lossy().to_string())
}

fn create_vrchat_metadata_json(metadata: &ImageMetadata) -> AppResult<String> {
    let mut json_obj = serde_json::Map::new();

    // Add VRChat metadata structure
    json_obj.insert(
        "application".to_string(),
        serde_json::Value::String("VRChat Photo Uploader".to_string()),
    );
    json_obj.insert("version".to_string(), serde_json::Value::Number(2.into()));
    json_obj.insert(
        "created_at".to_string(),
        serde_json::Value::String(chrono::Utc::now().to_rfc3339()),
    );

    // Add author info
    if let Some(ref author) = metadata.author {
        let mut author_obj = serde_json::Map::new();
        author_obj.insert(
            "displayName".to_string(),
            serde_json::Value::String(author.display_name.clone()),
        );
        author_obj.insert(
            "id".to_string(),
            serde_json::Value::String(author.id.clone()),
        );
        json_obj.insert("author".to_string(), serde_json::Value::Object(author_obj));
    }

    // Add world info
    if let Some(ref world) = metadata.world {
        let mut world_obj = serde_json::Map::new();
        world_obj.insert(
            "name".to_string(),
            serde_json::Value::String(world.name.clone()),
        );
        world_obj.insert(
            "id".to_string(),
            serde_json::Value::String(world.id.clone()),
        );
        world_obj.insert(
            "instanceId".to_string(),
            serde_json::Value::String(world.instance_id.clone()),
        );
        json_obj.insert("world".to_string(), serde_json::Value::Object(world_obj));
    }

    // Add players array
    let players_array: Vec<serde_json::Value> = metadata
        .players
        .iter()
        .map(|player| {
            let mut player_obj = serde_json::Map::new();
            player_obj.insert(
                "displayName".to_string(),
                serde_json::Value::String(player.display_name.clone()),
            );
            player_obj.insert(
                "id".to_string(),
                serde_json::Value::String(player.id.clone()),
            );
            serde_json::Value::Object(player_obj)
        })
        .collect();

    json_obj.insert(
        "players".to_string(),
        serde_json::Value::Array(players_array),
    );

    let json_value = serde_json::Value::Object(json_obj);
    Ok(serde_json::to_string_pretty(&json_value)?)
}

fn save_png_with_metadata(
    img: &image::DynamicImage,
    output_path: &Path,
    metadata_json: &str,
) -> AppResult<()> {
    use std::io::Cursor;

    // Convert image to PNG bytes
    let mut png_data = Vec::new();
    let mut cursor = Cursor::new(&mut png_data);
    img.write_to(&mut cursor, image::ImageOutputFormat::Png)?;

    // Parse PNG and inject metadata
    let modified_png = inject_png_metadata(&png_data, metadata_json)?;

    // Write to output file
    fs::write(output_path, modified_png)?;

    Ok(())
}

fn inject_png_metadata(png_data: &[u8], metadata_json: &str) -> AppResult<Vec<u8>> {
    let mut result = Vec::new();

    // Verify PNG signature
    if png_data.len() < 8 {
        return Err(AppError::invalid_file_type("Invalid PNG file"));
    }

    const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
    if png_data[0..8] != PNG_SIGNATURE {
        return Err(AppError::invalid_file_type("Not a valid PNG file"));
    }

    // Copy PNG signature
    result.extend_from_slice(&png_data[0..8]);

    let mut pos = 8;
    let mut metadata_inserted = false;

    while pos < png_data.len() {
        if pos + 8 > png_data.len() {
            break;
        }

        let length = u32::from_be_bytes([
            png_data[pos],
            png_data[pos + 1],
            png_data[pos + 2],
            png_data[pos + 3],
        ]) as usize;

        let chunk_type = &png_data[pos + 4..pos + 8];
        let chunk_type_str = std::str::from_utf8(chunk_type).unwrap_or("");

        // Insert our metadata chunk after IHDR but before IDAT
        if chunk_type_str == "IDAT" && !metadata_inserted {
            insert_text_chunk(&mut result, "Description", metadata_json)?;
            metadata_inserted = true;
        }

        // Skip existing Description chunks to avoid duplicates
        if (chunk_type_str == "tEXt" || chunk_type_str == "iTXt") && pos + 8 + length <= png_data.len() {
            let chunk_data = &png_data[pos + 8..pos + 8 + length];
            if let Ok(text) = std::str::from_utf8(chunk_data) {
                if text.starts_with("Description\0") {
                    // Skip this chunk
                    pos += 12 + length;
                    continue;
                }
            }
        }

        // Copy the original chunk
        let chunk_end = pos + 12 + length; // 4 length + 4 type + data + 4 CRC
        if chunk_end <= png_data.len() {
            result.extend_from_slice(&png_data[pos..chunk_end]);
        }

        pos = chunk_end;
    }

    // If metadata wasn't inserted yet, add it before the end
    if !metadata_inserted {
        insert_text_chunk(&mut result, "Description", metadata_json)?;
    }

    Ok(result)
}

fn insert_text_chunk(result: &mut Vec<u8>, keyword: &str, text: &str) -> AppResult<()> {
    // Validate keyword length (PNG spec: 1-79 bytes)
    if keyword.is_empty() || keyword.len() > 79 {
        return Err(AppError::validation(
            "keyword",
            "Keyword must be 1-79 bytes",
        ));
    }

    let data = format!("{}\0{}", keyword, text);
    let data_bytes = data.as_bytes();
    let length = data_bytes.len() as u32;

    // Write length
    result.extend_from_slice(&length.to_be_bytes());

    // Write chunk type (tEXt)
    result.extend_from_slice(b"tEXt");

    // Write data
    result.extend_from_slice(data_bytes);

    // Calculate and write CRC
    let crc = calculate_crc(&[b"tEXt", data_bytes].concat());
    result.extend_from_slice(&crc.to_be_bytes());

    Ok(())
}

fn calculate_crc(data: &[u8]) -> u32 {
    // Standard PNG CRC calculation
    const CRC_TABLE: [u32; 256] = [
        0x00000000, 0x77073096, 0xee0e612c, 0x990951ba, 0x076dc419, 0x706af48f, 0xe963a535,
        0x9e6495a3, 0x0edb8832, 0x79dcb8a4, 0xe0d5e91e, 0x97d2d988, 0x09b64c2b, 0x7eb17cbd,
        0xe7b82d07, 0x90bf1d91, 0x1db71064, 0x6ab020f2, 0xf3b97148, 0x84be41de, 0x1adad47d,
        0x6ddde4eb, 0xf4d4b551, 0x83d385c7, 0x136c9856, 0x646ba8c0, 0xfd62f97a, 0x8a65c9ec,
        0x14015c4f, 0x63066cd9, 0xfa0f3d63, 0x8d080df5, 0x3b6e20c8, 0x4c69105e, 0xd56041e4,
        0xa2677172, 0x3c03e4d1, 0x4b04d447, 0xd20d85fd, 0xa50ab56b, 0x35b5a8fa, 0x42b2986c,
        0xdbbbc9d6, 0xacbcf940, 0x32d86ce3, 0x45df5c75, 0xdcd60dcf, 0xabd13d59, 0x26d930ac,
        0x51de003a, 0xc8d75180, 0xbfd06116, 0x21b4f4b5, 0x56b3c423, 0xcfba9599, 0xb8bda50f,
        0x2802b89e, 0x5f058808, 0xc60cd9b2, 0xb10be924, 0x2f6f7c87, 0x58684c11, 0xc1611dab,
        0xb6662d3d, 0x76dc4190, 0x01db7106, 0x98d220bc, 0xefd5102a, 0x71b18589, 0x06b6b51f,
        0x9fbfe4a5, 0xe8b8d433, 0x7807c9a2, 0x0f00f934, 0x9609a88e, 0xe10e9818, 0x7f6a0dbb,
        0x086d3d2d, 0x91646c97, 0xe6635c01, 0x6b6b51f4, 0x1c6c6162, 0x856530d8, 0xf262004e,
        0x6c0695ed, 0x1b01a57b, 0x8208f4c1, 0xf50fc457, 0x65b0d9c6, 0x12b7e950, 0x8bbeb8ea,
        0xfcb9887c, 0x62dd1ddf, 0x15da2d49, 0x8cd37cf3, 0xfbd44c65, 0x4db26158, 0x3ab551ce,
        0xa3bc0074, 0xd4bb30e2, 0x4adfa541, 0x3dd895d7, 0xa4d1c46d, 0xd3d6f4fb, 0x4369e96a,
        0x346ed9fc, 0xad678846, 0xda60b8d0, 0x44042d73, 0x33031de5, 0xaa0a4c5f, 0xdd0d7cc9,
        0x5005713c, 0x270241aa, 0xbe0b1010, 0xc90c2086, 0x5768b525, 0x206f85b3, 0xb966d409,
        0xce61e49f, 0x5edef90e, 0x29d9c998, 0xb0d09822, 0xc7d7a8b4, 0x59b33d17, 0x2eb40d81,
        0xb7bd5c3b, 0xc0ba6cad, 0xedb88320, 0x9abfb3b6, 0x03b6e20c, 0x74b1d29a, 0xead54739,
        0x9dd277af, 0x04db2615, 0x73dc1683, 0xe3630b12, 0x94643b84, 0x0d6d6a3e, 0x7a6a5aa8,
        0xe40ecf0b, 0x9309ff9d, 0x0a00ae27, 0x7d079eb1, 0xf00f9344, 0x8708a3d2, 0x1e01f268,
        0x6906c2fe, 0xf762575d, 0x806567cb, 0x196c3671, 0x6e6b06e7, 0xfed41b76, 0x89d32be0,
        0x10da7a5a, 0x67dd4acc, 0xf9b9df6f, 0x8ebeeff9, 0x17b7be43, 0x60b08ed5, 0xd6d6a3e8,
        0xa1d1937e, 0x38d8c2c4, 0x4fdff252, 0xd1bb67f1, 0xa6bc5767, 0x3fb506dd, 0x48b2364b,
        0xd80d2bda, 0xaf0a1b4c, 0x36034af6, 0x41047a60, 0xdf60efc3, 0xa867df55, 0x316e8eef,
        0x4669be79, 0xcb61b38c, 0xbc66831a, 0x256fd2a0, 0x5268e236, 0xcc0c7795, 0xbb0b4703,
        0x220216b9, 0x5505262f, 0xc5ba3bbe, 0xb2bd0b28, 0x2bb45a92, 0x5cb36a04, 0xc2d7ffa7,
        0xb5d0cf31, 0x2cd99e8b, 0x5bdeae1d, 0x9b64c2b0, 0xec63f226, 0x756aa39c, 0x026d930a,
        0x9c0906a9, 0xeb0e363f, 0x72076785, 0x05005713, 0x95bf4a82, 0xe2b87a14, 0x7bb12bae,
        0x0cb61b38, 0x92d28e9b, 0xe5d5be0d, 0x7cdcefb7, 0x0bdbdf21, 0x86d3d2d4, 0xf1d4e242,
        0x68ddb3f8, 0x1fda836e, 0x81be16cd, 0xf6b9265b, 0x6fb077e1, 0x18b74777, 0x88085ae6,
        0xff0f6a70, 0x66063bca, 0x11010b5c, 0x8f659eff, 0xf862ae69, 0x616bffd3, 0x166ccf45,
        0xa00ae278, 0xd70dd2ee, 0x4e048354, 0x3903b3c2, 0xa7672661, 0xd06016f7, 0x4969474d,
        0x3e6e77db, 0xaed16a4a, 0xd9d65adc, 0x40df0b66, 0x37d83bf0, 0xa9bcae53, 0xdebb9ec5,
        0x47b2cf7f, 0x30b5ffe9, 0xbdbdf21c, 0xcabac28a, 0x53b39330, 0x24b4a3a6, 0xbad03605,
        0xcdd70693, 0x54de5729, 0x23d967bf, 0xb3667a2e, 0xc4614ab8, 0x5d681b02, 0x2a6f2b94,
        0xb40bbe37, 0xc30c8ea1, 0x5a05df1b, 0x2d02ef8d,
    ];

    let mut crc = 0xffffffff;
    for &byte in data {
        let table_index = ((crc ^ byte as u32) & 0xff) as usize;
        crc = CRC_TABLE[table_index] ^ (crc >> 8);
    }
    crc ^ 0xffffffff
}
