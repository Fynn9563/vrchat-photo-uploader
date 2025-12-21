use crate::commands::{ImageMetadata, PlayerInfo, WorldInfo};
use crate::image_processor;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ImageGroup {
    pub images: Vec<String>,
    pub timestamp: Option<i64>,
    pub group_id: String,
    pub all_players: Vec<PlayerInfo>,
    pub all_worlds: Vec<WorldInfo>,
}

/// Groups images by world and time window
pub async fn group_images_by_metadata(
    file_paths: Vec<String>,
    time_window_minutes: u32,
    group_by_world: bool,
) -> Vec<ImageGroup> {
    let mut image_data: Vec<(String, Option<ImageMetadata>, Option<i64>, String)> = Vec::new();
    let no_time_limit = time_window_minutes == 0;
    let time_window_seconds = if no_time_limit { 1 } else { (time_window_minutes as i64) * 60 };

    // Extract metadata and compute group keys
    for file_path in file_paths {
        log::debug!("Extracting metadata for: {}", file_path);
        let metadata = image_processor::extract_metadata(&file_path)
            .await
            .ok()
            .flatten();
        let timestamp = image_processor::get_timestamp_from_filename(&file_path);

        let group_key = if let Some(ref meta) = metadata {
            create_metadata_key(meta, timestamp, time_window_seconds, no_time_limit, group_by_world)
        } else if no_time_limit {
            "unknown_all".to_string()
        } else if let Some(ts) = timestamp {
            format!("unknown_{}", ts / time_window_seconds)
        } else {
            format!("unknown_{}", file_path)
        };

        image_data.push((file_path, metadata, timestamp, group_key));
    }

    // Group images and collect players and worlds
    let mut groups: HashMap<String, ImageGroup> = HashMap::new();
    let mut group_players: HashMap<String, HashMap<String, PlayerInfo>> = HashMap::new();
    let mut group_worlds: HashMap<String, HashMap<String, WorldInfo>> = HashMap::new();

    for (file_path, metadata, timestamp, group_key) in image_data {
        if let Some(ref meta) = metadata {
            // Merge players using ID as key to avoid duplicates
            let player_map = group_players.entry(group_key.clone()).or_default();
            for player in &meta.players {
                player_map.entry(player.id.clone()).or_insert_with(|| player.clone());
            }

            // Merge worlds using ID as key to avoid duplicates
            if let Some(ref world) = meta.world {
                let world_map = group_worlds.entry(group_key.clone()).or_default();
                world_map.entry(world.id.clone()).or_insert_with(|| world.clone());
            }
        }

        let group = groups.entry(group_key.clone()).or_insert_with(|| ImageGroup {
            images: Vec::new(),
            timestamp,
            group_id: group_key.clone(),
            all_players: Vec::new(),
            all_worlds: Vec::new(),
        });

        group.images.push(file_path);
    }

    // Populate all_players and all_worlds for each group
    for (group_key, group) in groups.iter_mut() {
        if let Some(player_map) = group_players.get(group_key) {
            group.all_players = player_map.values().cloned().collect();
            group.all_players.sort_by(|a, b| a.display_name.cmp(&b.display_name));
        }
        if let Some(world_map) = group_worlds.get(group_key) {
            group.all_worlds = world_map.values().cloned().collect();
            group.all_worlds.sort_by(|a, b| a.name.cmp(&b.name));
        }
    }

    // Sort by timestamp
    let mut group_list: Vec<_> = groups.into_values().collect();
    group_list.sort_by_key(|group| group.timestamp.unwrap_or(0));

    log::info!(
        "Created {} groups from {} images",
        group_list.len(),
        group_list.iter().map(|g| g.images.len()).sum::<usize>()
    );

    group_list
}

/// Creates one group per image (no grouping)
pub async fn create_individual_groups_with_metadata(file_paths: Vec<String>) -> Vec<ImageGroup> {
    let mut groups = Vec::new();

    for (i, file_path) in file_paths.into_iter().enumerate() {
        let metadata = image_processor::extract_metadata(&file_path)
            .await
            .ok()
            .flatten();
        let timestamp = image_processor::get_timestamp_from_filename(&file_path);
        let all_players = metadata.as_ref().map(|m| m.players.clone()).unwrap_or_default();
        let all_worlds = metadata.as_ref()
            .and_then(|m| m.world.clone())
            .map(|w| vec![w])
            .unwrap_or_default();

        groups.push(ImageGroup {
            images: vec![file_path.clone()],
            timestamp,
            group_id: format!(
                "individual_{}_{}",
                i,
                Path::new(&file_path).file_name().unwrap_or_default().to_string_lossy()
            ),
            all_players,
            all_worlds,
        });
    }

    groups.sort_by_key(|group| group.timestamp.unwrap_or(0));
    groups
}

fn create_metadata_key(
    metadata: &ImageMetadata,
    timestamp: Option<i64>,
    time_window_seconds: i64,
    no_time_limit: bool,
    group_by_world: bool,
) -> String {
    let world_part = if group_by_world {
        metadata.world.as_ref().map(|w| w.id.clone()).unwrap_or_else(|| "unknown".to_string())
    } else {
        "any_world".to_string()
    };

    if no_time_limit {
        format!("{}_all", world_part)
    } else {
        format!("{}_t{}", world_part, timestamp.unwrap_or(0) / time_window_seconds)
    }
}

/// Creates Discord payload. Returns (main_payload, overflow_messages)
pub fn create_discord_payload(
    all_worlds: &[WorldInfo],
    all_players: &[PlayerInfo],
    timestamp: Option<i64>,
    is_first_message: bool,
    chunk_index: usize,
    is_forum_post: bool,
    _thread_id: Option<&str>,
    include_player_names: bool,
) -> (HashMap<String, String>, Vec<String>) {
    let mut payload = HashMap::new();
    let mut overflow_messages = Vec::new();

    if is_first_message {
        // Create content with worlds, timestamp, and as many players as fit
        let (content, remaining_players, had_players_in_main) = create_message_content_with_players(
            all_worlds,
            all_players,
            timestamp,
            include_player_names
        );
        payload.insert("content".to_string(), content);

        if is_forum_post {
            let thread_name = create_thread_title(all_worlds);
            payload.insert("thread_name".to_string(), thread_name);
        }

        // Create overflow messages for remaining players
        if !remaining_players.is_empty() {
            overflow_messages = create_overflow_player_messages(&remaining_players, had_players_in_main);
        }
    } else if chunk_index > 0 {
        // No text for continuation chunks - just upload the images silently
    }

    (payload, overflow_messages)
}

/// Creates message with worlds, timestamp, and as many players as fit
fn create_message_content_with_players(
    all_worlds: &[WorldInfo],
    all_players: &[PlayerInfo],
    timestamp: Option<i64>,
    include_player_names: bool,
) -> (String, Vec<PlayerInfo>, bool) {
    const MAX_LENGTH: usize = 1900; // Leave buffer for Discord's 2000 char limit
    let mut content = String::new();
    let mut remaining_players: Vec<PlayerInfo> = Vec::new();
    let mut had_players_in_main = false;

    if !all_worlds.is_empty() {
        content.push_str("📸 Photos taken at ");

        let world_parts: Vec<String> = all_worlds
            .iter()
            .map(|world| {
                let vrchat_link = format!("https://vrchat.com/home/launch?worldId={}", world.id);
                let vrcx_link = format!("https://vrcx.azurewebsites.net/world/{}", world.id);
                format!("**{}** ([VRChat](<{}>), [VRCX](<{}>))", world.name, vrchat_link, vrcx_link)
            })
            .collect();

        content.push_str(&world_parts.join(", "));

        if let Some(ts) = timestamp {
            content.push_str(&format!(" at <t:{}:f>", ts));
        }

        // Add players if requested
        if include_player_names && !all_players.is_empty() {
            // Check if we can fit at least "with " + one player name
            let first_player = format!("**{}**", all_players[0].display_name);
            let with_prefix = " with ";

            if content.len() + with_prefix.len() + first_player.len() <= MAX_LENGTH {
                content.push_str(with_prefix);
                content.push_str(&first_player);
                had_players_in_main = true;

                let mut players_added = 1;
                for player in all_players.iter().skip(1) {
                    let player_str = format!("**{}**", player.display_name);
                    let addition = format!(", {}", player_str);

                    if content.len() + addition.len() > MAX_LENGTH {
                        // Can't fit more players, save remaining
                        remaining_players = all_players[players_added..].to_vec();
                        // End with comma to indicate continuation
                        content.push(',');
                        log::info!("First message has {} players, {} overflow to next message(s)",
                            players_added, remaining_players.len());
                        break;
                    }
                    content.push_str(&addition);
                    players_added += 1;
                }
            } else {
                // Can't fit any players, all go to overflow
                remaining_players = all_players.to_vec();
                log::info!("No players fit in first message, all {} go to overflow", remaining_players.len());
            }
        }
    } else {
        content.push_str("📸 Photos");
        if let Some(ts) = timestamp {
            content.push_str(&format!(" taken at <t:{}:f>", ts));
        }
    }

    log::debug!("Final message content length: {} chars", content.len());

    (content, remaining_players, had_players_in_main)
}

/// Creates overflow messages for remaining players
fn create_overflow_player_messages(remaining_players: &[PlayerInfo], had_players_in_main: bool) -> Vec<String> {
    const MAX_LENGTH: usize = 1900; // Leave buffer for Discord's 2000 char limit
    let mut messages = Vec::new();

    // If no players were in the main message, start with "with "
    let mut current = if !had_players_in_main {
        String::from("with ")
    } else {
        String::new()
    };
    let prefix_len = current.len();

    for player in remaining_players.iter() {
        let player_str = format!("**{}**", player.display_name);
        let separator = if current.len() > prefix_len { ", " } else { "" };
        let addition = format!("{}{}", separator, player_str);

        if current.len() > prefix_len && current.len() + addition.len() > MAX_LENGTH {
            // Current message is full, end with comma and start new one
            current.push(',');
            messages.push(current);
            current = player_str;
        } else {
            current.push_str(&addition);
        }
    }

    // Don't forget the last message (no trailing comma on final message)
    if current.len() > prefix_len || (!had_players_in_main && !current.is_empty()) {
        messages.push(current);
    }

    log::info!("Created {} overflow message(s) for {} remaining players",
        messages.len(), remaining_players.len());
    messages
}

fn create_thread_title(all_worlds: &[WorldInfo]) -> String {
    if !all_worlds.is_empty() {
        let world_names: Vec<&str> = all_worlds.iter().map(|w| w.name.as_str()).collect();
        let title = format!("📸 Photos from {}", world_names.join(", "));
        if title.len() > 100 {
            format!("{}...", &title[..97])
        } else {
            title
        }
    } else {
        "📸 Photos".to_string()
    }
}
