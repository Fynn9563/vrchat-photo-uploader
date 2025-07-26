use std::collections::HashMap;
use std::path::Path;
use crate::commands::ImageMetadata;
use crate::image_processor;

#[derive(Debug, Clone)]
pub struct ImageGroup {
    pub images: Vec<String>,
    pub metadata: Option<ImageMetadata>,
    pub timestamp: Option<i64>,
    pub group_id: String,
}

/// Group images by their metadata (world, players, timestamp)
pub async fn group_images_by_metadata(file_paths: Vec<String>, include_player_names: bool) -> Vec<ImageGroup> {

    let mut groups: HashMap<String, ImageGroup> = HashMap::new();
    
    for file_path in file_paths {
        log::debug!("Extracting metadata for: {}", file_path);
        let metadata = image_processor::extract_metadata(&file_path).await.ok().flatten();
        let timestamp = image_processor::get_timestamp_from_filename(&file_path);
        
        if let Some(ref meta) = metadata {
            log::info!("Extracted metadata for {}: world={}, players={}", 
                file_path, 
                meta.world.as_ref().map(|w| &w.name).unwrap_or(&"None".to_string()),
                meta.players.len()
            );
        } else {
            log::info!("No metadata found for: {}", file_path);
        }
        
        let group_key = if let Some(ref meta) = metadata {
            create_metadata_key(meta, timestamp, include_player_names)
        } else {
            // Group unknown metadata by timestamp window (5 minutes)
            if let Some(ts) = timestamp {
                format!("unknown_{}", ts / 300) // 5-minute windows
            } else {
                format!("unknown_{}", file_path) // Individual
            }
        };
        
        groups.entry(group_key.clone())
            .or_insert_with(|| ImageGroup {
                images: Vec::new(),
                metadata: metadata.clone(),
                timestamp,
                group_id: group_key.clone(),
            })
            .images
            .push(file_path);
    }
    
    // Sort groups by timestamp to maintain chronological order
    let mut group_list: Vec<_> = groups.into_values().collect();
    group_list.sort_by_key(|group| group.timestamp.unwrap_or(0));
    
    group_list
}

/// Create individual groups for each image (no grouping by metadata)
pub async fn create_individual_groups_with_metadata(file_paths: Vec<String>) -> Vec<ImageGroup> {
    let mut groups = Vec::new();
    
    for (i, file_path) in file_paths.into_iter().enumerate() {
        log::debug!("Extracting metadata for individual image: {}", file_path);
        let metadata = image_processor::extract_metadata(&file_path).await.ok().flatten();
        let timestamp = image_processor::get_timestamp_from_filename(&file_path);
        
        if let Some(ref meta) = metadata {
            log::info!("Extracted metadata for individual image {}: world={}, players={}", 
                file_path, 
                meta.world.as_ref().map(|w| &w.name).unwrap_or(&"None".to_string()),
                meta.players.len()
            );
        } else {
            log::info!("No metadata found for individual image: {}", file_path);
        }
        
        groups.push(ImageGroup {
            images: vec![file_path.clone()],
            metadata,
            timestamp,
            group_id: format!("individual_{}_{}", i, 
                Path::new(&file_path).file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            ),
        });
    }
    
    // Sort by timestamp to maintain chronological order
    groups.sort_by_key(|group| group.timestamp.unwrap_or(0));
    
    log::info!("Created {} individual groups with metadata", groups.len());
    groups
}

/// Create a unique key for grouping images by metadata
fn create_metadata_key(metadata: &ImageMetadata, timestamp: Option<i64>, include_player_names: bool) -> String {
    let world_id = metadata.world
        .as_ref()
        .map(|w| w.id.clone())
        .unwrap_or_else(|| "unknown".to_string());
    
    let player_part = if include_player_names {
        let mut player_ids: Vec<String> = metadata.players
            .iter()
            .map(|p| p.id.clone())
            .collect();
        player_ids.sort();
        player_ids.join(",")
    } else {
        String::new() // Don't group by players when disabled
    };
    
    let time_window = timestamp.unwrap_or(0) / 300;
    
    format!("{}_{}_t{}", world_id, player_part, time_window)
}

/// Create Discord payload for webhook message
pub fn create_discord_payload(
    metadata: &Option<ImageMetadata>,
    timestamp: Option<i64>,
    is_first_message: bool,
    chunk_index: usize,
    is_forum_post: bool,
    _thread_id: Option<&str>,
    include_player_names: bool,
) -> HashMap<String, String> {
    let mut payload = HashMap::new();
    
    if is_first_message {
        // Always create message content, regardless of metadata
        let content = if let Some(meta) = metadata {
            create_message_content(meta, timestamp, include_player_names)
        } else {
            // Fallback content when no metadata is available
            if let Some(ts) = timestamp {
                format!("📸 VRChat Photo taken at <t:{}:f>", ts)
            } else {
                "📸 VRChat Photo".to_string()
            }
        };
        
        payload.insert("content".to_string(), content);
        
        // ALWAYS set thread_name for forum posts (initial message)
        if is_forum_post {
            let thread_name = if let Some(meta) = metadata {
                create_thread_title(meta)
            } else {
                "📸 VRChat Photos".to_string()
            };
            log::info!("📝 Setting thread_name for forum post: '{}'", thread_name);
            payload.insert("thread_name".to_string(), thread_name);
        }
    } else if chunk_index > 0 {
        // Continuation message
        payload.insert("content".to_string(), format!("📸 [continues... part {}]", chunk_index + 1));
        
        if is_forum_post {
            log::info!("🔗 Forum continuation - thread_id will be added to URL query parameters");
        }
    }
    
    log::debug!("Created Discord payload (form data): {:?}", payload);
    payload
}

/// Create message content with world and player information
fn create_message_content(metadata: &ImageMetadata, timestamp: Option<i64>, include_player_names: bool) -> String {
    let mut content = String::new();
    
    if let Some(world) = &metadata.world {
        let vrchat_link = format!("https://vrchat.com/home/launch?worldId={}", world.id);
        let vrcx_link = format!("https://vrcx.azurewebsites.net/world/{}", world.id);
        
        content.push_str(&format!(
            "📸 Photo taken at **{}** ([VRChat](<{}>), [VRCX](<{}>))",
            world.name, vrchat_link, vrcx_link
        ));
        
        // Add player names if enabled
        if include_player_names && !metadata.players.is_empty() {
            let player_names: Vec<String> = metadata.players
                .iter()
                .map(|p| format!("**{}**", p.display_name))
                .collect();
            content.push_str(&format!(" with {}", player_names.join(", ")));
        }
        
        if let Some(ts) = timestamp {
            content.push_str(&format!(" at <t:{}:f>", ts));
        }
    } else {
        content.push_str("📸 VRChat Photo");
        if let Some(ts) = timestamp {
            content.push_str(&format!(" taken at <t:{}:f>", ts));
        }
    }
    
    content
}

/// Create thread title for forum posts
fn create_thread_title(metadata: &ImageMetadata) -> String {
    if let Some(world) = &metadata.world {
        let title = format!("📸 Photos from {}", world.name);
        
        // Discord thread title limit is 100 characters
        if title.len() > 100 {
            format!("{}...", &title[..97])
        } else {
            title
        }
    } else {
        "📸 VRChat Photos".to_string()
    }
}