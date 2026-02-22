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
// Update signature and implementation
pub async fn group_images_by_metadata(
    file_paths: Vec<String>,
    time_window_minutes: u32,
    group_by_world: bool,
    merge_no_metadata: bool,
    app_handle: tauri::AppHandle,
    session_id: String,
) -> Vec<ImageGroup> {
    let mut image_data: Vec<(String, Option<ImageMetadata>, Option<i64>, String)> = Vec::new();
    let no_time_limit = time_window_minutes == 0;
    let time_window_seconds = if no_time_limit {
        1
    } else {
        (time_window_minutes as i64) * 60
    };

    // Parallel metadata extraction
    // Use a semaphore to limit concurrency
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::sync::Mutex;
    use tauri::Emitter;
    use tokio::sync::Semaphore;

    let max_concurrent = std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(4)
        .min(16);
    let semaphore = Arc::new(Semaphore::new(max_concurrent));
    let results_mutex = Arc::new(Mutex::new(Vec::with_capacity(file_paths.len())));

    let total_files = file_paths.len();
    let completed_counter = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();

    for (index, file_path) in file_paths.into_iter().enumerate() {
        let sem = semaphore.clone();
        let results = results_mutex.clone();
        let completed = completed_counter.clone();
        let app_handle = app_handle.clone();
        let session_id = session_id.clone();

        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            log::debug!("Extracting metadata for: {file_path}");

            let metadata = image_processor::extract_metadata(&file_path)
                .await
                .ok()
                .flatten();
            let timestamp = image_processor::get_timestamp_from_filename(&file_path);

            let mut guard = results.lock().unwrap();
            guard.push((index, file_path, metadata, timestamp));

            // Emit progress
            let done = completed.fetch_add(1, Ordering::SeqCst) + 1;
            // Emit batch updates to avoid flooding event loop for 5000 items
            if done % 5 == 0 || done == total_files {
                app_handle.emit("upload-progress", serde_json::json!({
                    "session_id": session_id,
                    "total_images": total_files,
                    "completed": 0, // Uploads completed
                    "current_progress": (done as f64 / total_files as f64) * 100.0,
                    "session_status": format!("Preparing images... {}/{}", done, total_files),
                    // We can also send a custom event if main listener expects distinct fields
                 })).ok();
            }
        }));
    }

    // Wait for all
    for handle in handles {
        if let Err(e) = handle.await {
            log::error!("Metadata extraction task failed: {e}");
        }
    }

    // Extract results and sort by original index to maintain order
    let mut collected_results = match Arc::try_unwrap(results_mutex) {
        Ok(mutex) => mutex.into_inner().unwrap(),
        Err(arc) => arc.lock().unwrap().clone(), // Fallback clone
    };
    collected_results.sort_by_key(|r| r.0);

    let mut last_valid_group_key: Option<String> = None;

    // Sequential grouping logic (must be sequential for context)
    for (_index, file_path, metadata, timestamp) in collected_results {
        let group_key = if let Some(ref meta) = metadata {
            let key = create_metadata_key(
                meta,
                timestamp,
                time_window_seconds,
                no_time_limit,
                group_by_world,
            );
            // Update last valid key if we found metadata
            if merge_no_metadata {
                last_valid_group_key = Some(key.clone());
            }
            key
        } else if merge_no_metadata && last_valid_group_key.is_some() {
            // If merging is enabled and we have a previous group, use it!
            let key = last_valid_group_key.as_ref().unwrap().clone();
            log::info!("Merging no-metadata file {file_path} into previous group: {key}");
            key
        } else if no_time_limit {
            "unknown_all".to_string()
        } else if let Some(ts) = timestamp {
            format!("unknown_{}", ts / time_window_seconds)
        } else {
            format!("unknown_{file_path}")
        };

        image_data.push((file_path, metadata, timestamp, group_key));
    }

    log::info!(
        "Grouping {} images (window: {}m, world: {}, merge_no_meta: {})",
        image_data.len(),
        time_window_minutes,
        group_by_world,
        merge_no_metadata
    );

    // Group images and collect players and worlds
    let mut groups: HashMap<String, ImageGroup> = HashMap::new();
    let mut group_players: HashMap<String, HashMap<String, PlayerInfo>> = HashMap::new();
    let mut group_worlds: HashMap<String, HashMap<String, WorldInfo>> = HashMap::new();

    for (file_path, metadata, timestamp, group_key) in image_data {
        if let Some(ref meta) = metadata {
            // Merge players using ID as key to avoid duplicates
            let player_map = group_players.entry(group_key.clone()).or_default();
            for player in &meta.players {
                player_map
                    .entry(player.id.clone())
                    .or_insert_with(|| player.clone());
            }

            // Merge worlds using ID as key to avoid duplicates
            if let Some(ref world) = meta.world {
                let world_map = group_worlds.entry(group_key.clone()).or_default();
                world_map
                    .entry(world.id.clone())
                    .or_insert_with(|| world.clone());
            }
        }

        let group = groups
            .entry(group_key.clone())
            .or_insert_with(|| ImageGroup {
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
            group
                .all_players
                .sort_by(|a, b| a.display_name.cmp(&b.display_name));
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
        let all_players = metadata
            .as_ref()
            .map(|m| m.players.clone())
            .unwrap_or_default();
        let all_worlds = metadata
            .as_ref()
            .and_then(|m| m.world.clone())
            .map(|w| vec![w])
            .unwrap_or_default();

        groups.push(ImageGroup {
            images: vec![file_path.clone()],
            timestamp,
            group_id: format!(
                "individual_{}_{}",
                i,
                Path::new(&file_path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
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
        metadata
            .world
            .as_ref()
            .map(|w| w.id.clone())
            .unwrap_or_else(|| "unknown".to_string())
    } else {
        "any_world".to_string()
    };

    if no_time_limit {
        format!("{world_part}_all")
    } else {
        format!(
            "{}_t{}",
            world_part,
            timestamp.unwrap_or(0) / time_window_seconds
        )
    }
}

/// Format a player for Discord: returns `<@discord_id>` if mapped, else `**PlayerName**`
fn format_player_for_discord(
    player: &PlayerInfo,
    discord_mappings: &HashMap<String, String>,
) -> String {
    // Check by VRChat user ID first (more reliable), then by display name
    // Keys in the map are lowercased for case-insensitive matching
    if let Some(discord_id) = discord_mappings
        .get(&player.id.to_lowercase())
        .or_else(|| discord_mappings.get(&player.display_name.to_lowercase()))
    {
        format!("<@{discord_id}>")
    } else {
        format!("**{}**", player.display_name)
    }
}

/// Creates Discord payload. Returns (main_payload, overflow_messages)
#[allow(clippy::too_many_arguments)]
pub fn create_discord_payload(
    all_worlds: &[WorldInfo],
    all_players: &[PlayerInfo],
    timestamp: Option<i64>,
    is_first_message: bool,
    chunk_index: usize,
    is_forum_post: bool,
    _thread_id: Option<&str>,
    include_player_names: bool,
    image_count: usize,
    discord_mappings: &HashMap<String, String>,
) -> (HashMap<String, String>, Vec<String>) {
    let mut payload = HashMap::new();
    let mut overflow_messages = Vec::new();

    if is_first_message {
        // Create content with worlds, timestamp, and as many players as fit
        let (content, remaining_players, had_players_in_main) = create_message_content_with_players(
            all_worlds,
            all_players,
            timestamp,
            include_player_names,
            image_count,
            discord_mappings,
        );
        payload.insert("content".to_string(), content);

        if is_forum_post {
            let thread_name = create_thread_title(all_worlds, image_count);
            payload.insert("thread_name".to_string(), thread_name);
        }

        // Create overflow messages for remaining players
        if !remaining_players.is_empty() {
            overflow_messages = create_overflow_player_messages(
                &remaining_players,
                had_players_in_main,
                discord_mappings,
            );
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
    image_count: usize,
    discord_mappings: &HashMap<String, String>,
) -> (String, Vec<PlayerInfo>, bool) {
    const MAX_LENGTH: usize = 1900;
    let mut content = String::new();
    let mut remaining_players: Vec<PlayerInfo> = Vec::new();
    let mut had_players_in_main = false;

    // Use singular "Photo" for 1 image, plural "Photos" for multiple
    let photo_word = if image_count == 1 { "Photo" } else { "Photos" };

    if !all_worlds.is_empty() {
        content.push_str(&format!("📸 {photo_word} taken at "));

        let world_parts: Vec<String> = all_worlds
            .iter()
            .map(|world| {
                let vrchat_link = format!("https://vrchat.com/home/launch?worldId={}", world.id);
                let vrcx_link = format!("https://vrcx.azurewebsites.net/world/{}", world.id);
                format!(
                    "**{}** ([VRChat](<{}>), [VRCX](<{}>))",
                    world.name, vrchat_link, vrcx_link
                )
            })
            .collect();

        content.push_str(&world_parts.join(", "));

        if let Some(ts) = timestamp {
            content.push_str(&format!(" at <t:{ts}:f>"));
        }

        // Add players if requested
        if include_player_names && !all_players.is_empty() {
            // Check if we can fit at least "with " + one player name
            let first_player = format_player_for_discord(&all_players[0], discord_mappings);
            let with_prefix = " with ";

            if content.len() + with_prefix.len() + first_player.len() <= MAX_LENGTH {
                content.push_str(with_prefix);
                content.push_str(&first_player);
                had_players_in_main = true;

                let mut players_added = 1;
                for player in all_players.iter().skip(1) {
                    let player_str = format_player_for_discord(player, discord_mappings);
                    let addition = format!(", {player_str}");

                    if content.len() + addition.len() > MAX_LENGTH {
                        // Can't fit more players, save remaining
                        remaining_players = all_players[players_added..].to_vec();
                        // End with comma to indicate continuation
                        content.push(',');
                        log::info!(
                            "First message has {} players, {} overflow to next message(s)",
                            players_added,
                            remaining_players.len()
                        );
                        break;
                    }
                    content.push_str(&addition);
                    players_added += 1;
                }
            } else {
                // Can't fit any players, all go to overflow
                remaining_players = all_players.to_vec();
                log::info!(
                    "No players fit in first message, all {} go to overflow",
                    remaining_players.len()
                );
            }
        }
    } else {
        content.push_str(&format!("📸 {photo_word}"));
        if let Some(ts) = timestamp {
            content.push_str(&format!(" taken at <t:{ts}:f>"));
        }
    }

    log::debug!("Final message content length: {} chars", content.len());

    (content, remaining_players, had_players_in_main)
}

/// Creates overflow messages for remaining players
fn create_overflow_player_messages(
    remaining_players: &[PlayerInfo],
    had_players_in_main: bool,
    discord_mappings: &HashMap<String, String>,
) -> Vec<String> {
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
        let player_str = format_player_for_discord(player, discord_mappings);
        let separator = if current.len() > prefix_len { ", " } else { "" };
        let addition = format!("{separator}{player_str}");

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

    log::info!(
        "Created {} overflow message(s) for {} remaining players",
        messages.len(),
        remaining_players.len()
    );
    messages
}

fn create_thread_title(all_worlds: &[WorldInfo], image_count: usize) -> String {
    let photo_word = if image_count == 1 { "Photo" } else { "Photos" };
    if !all_worlds.is_empty() {
        let world_names: Vec<&str> = all_worlds.iter().map(|w| w.name.as_str()).collect();
        let title = format!("📸 {} from {}", photo_word, world_names.join(", "));
        if title.len() > 100 {
            format!("{}...", &title[..97])
        } else {
            title
        }
    } else {
        format!("📸 {photo_word}")
    }
}

/// Creates a message with just worlds (no players) - used for first retry when combined message is too long
pub fn create_worlds_only_message(
    all_worlds: &[WorldInfo],
    timestamp: Option<i64>,
    image_count: usize,
) -> String {
    let photo_word = if image_count == 1 { "Photo" } else { "Photos" };
    if all_worlds.is_empty() {
        let mut content = format!("📸 {photo_word}");
        if let Some(ts) = timestamp {
            content.push_str(&format!(" taken at <t:{ts}:f>"));
        }
        return content;
    }

    let mut content = format!("📸 {photo_word} taken at ");

    let world_parts: Vec<String> = all_worlds
        .iter()
        .map(|world| {
            let vrchat_link = format!("https://vrchat.com/home/launch?worldId={}", world.id);
            let vrcx_link = format!("https://vrcx.azurewebsites.net/world/{}", world.id);
            format!(
                "**{}** ([VRChat](<{}>), [VRCX](<{}>))",
                world.name, vrchat_link, vrcx_link
            )
        })
        .collect();

    content.push_str(&world_parts.join(", "));

    if let Some(ts) = timestamp {
        content.push_str(&format!(" at <t:{ts}:f>"));
    }

    content
}

/// Creates a compact world summary (names only) and separate links messages
/// Returns (summary_message, link_messages) - used when there are many worlds
pub fn create_compact_world_messages(
    all_worlds: &[WorldInfo],
    image_count: usize,
) -> (String, Vec<String>) {
    const MAX_LENGTH: usize = 1900;
    let photo_word = if image_count == 1 { "Photo" } else { "Photos" };

    if all_worlds.is_empty() {
        return (format!("📸 {photo_word}"), vec![]);
    }

    // Build summary message with world names (bullet list)
    let mut summary = format!("📸 {} from {} worlds:\n", photo_word, all_worlds.len());
    for world in all_worlds.iter() {
        summary.push_str(&format!("• {}\n", world.name));
    }

    // Build links messages (chunked to fit Discord limit)
    let mut link_messages = Vec::new();
    let mut current_links = String::from("World Links:\n");
    let prefix_len = current_links.len();

    for world in all_worlds.iter() {
        let vrchat_link = format!("https://vrchat.com/home/launch?worldId={}", world.id);
        let vrcx_link = format!("https://vrcx.azurewebsites.net/world/{}", world.id);
        let link_line = format!("• [VRChat](<{vrchat_link}>) | [VRCX](<{vrcx_link}>)\n");

        if current_links.len() + link_line.len() > MAX_LENGTH {
            // Current message full, save and start new one
            link_messages.push(current_links.trim_end().to_string());
            current_links = link_line;
        } else {
            current_links.push_str(&link_line);
        }
    }

    // Don't forget the last links message
    if current_links.len() > prefix_len || !current_links.is_empty() {
        link_messages.push(current_links.trim_end().to_string());
    }

    log::info!(
        "Created compact world summary and {} link message(s) for {} worlds",
        link_messages.len(),
        all_worlds.len()
    );

    (summary.trim_end().to_string(), link_messages)
}

/// Creates player messages that fit within Discord's limit (used when combined message is too long)
pub fn create_split_player_messages(
    all_players: &[PlayerInfo],
    discord_mappings: &HashMap<String, String>,
) -> Vec<String> {
    const MAX_LENGTH: usize = 1900;
    let mut messages = Vec::new();

    if all_players.is_empty() {
        return messages;
    }

    let mut current = String::from("with ");
    let prefix_len = current.len();

    for player in all_players.iter() {
        let player_str = format_player_for_discord(player, discord_mappings);
        let separator = if current.len() > prefix_len { ", " } else { "" };
        let addition = format!("{separator}{player_str}");

        if current.len() > prefix_len && current.len() + addition.len() > MAX_LENGTH {
            // Current message is full, end with comma and start new one
            current.push(',');
            messages.push(current);
            current = format_player_for_discord(player, discord_mappings);
        } else {
            current.push_str(&addition);
        }
    }

    // Don't forget the last message
    if current.len() > prefix_len {
        messages.push(current);
    } else if current == "with " && !all_players.is_empty() {
        // Edge case: first player name alone
        messages.push(format!(
            "with {}",
            format_player_for_discord(&all_players[0], discord_mappings)
        ));
    }

    log::info!(
        "Created {} split player message(s) for {} players",
        messages.len(),
        all_players.len()
    );
    messages
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{ImageMetadata, PlayerInfo, WorldInfo};

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

    fn make_metadata(world_name: &str, world_id: &str) -> ImageMetadata {
        ImageMetadata {
            author: None,
            world: Some(make_world(world_name, world_id)),
            players: vec![],
        }
    }

    // --- create_discord_payload tests ---

    #[test]
    fn test_payload_first_message_with_world() {
        let worlds = vec![make_world("Test World", "wrld_123")];
        let players = vec![];
        let no_mappings = HashMap::new();
        let (payload, overflow) = create_discord_payload(
            &worlds,
            &players,
            Some(1705312200),
            true,
            0,
            false,
            None,
            false,
            3,
            &no_mappings,
        );
        let content = payload.get("content").unwrap();
        assert!(content.contains("Photos taken at"));
        assert!(content.contains("Test World"));
        assert!(content.contains("<t:1705312200:f>"));
        assert!(overflow.is_empty());
    }

    #[test]
    fn test_payload_first_message_no_world() {
        let no_mappings = HashMap::new();
        let (payload, _) = create_discord_payload(
            &[],
            &[],
            Some(1705312200),
            true,
            0,
            false,
            None,
            false,
            5,
            &no_mappings,
        );
        let content = payload.get("content").unwrap();
        assert!(content.contains("Photos"));
        assert!(content.contains("<t:1705312200:f>"));
    }

    #[test]
    fn test_payload_continuation_chunk_empty() {
        let worlds = vec![make_world("W", "wrld_1")];
        let no_mappings = HashMap::new();
        let (payload, _) = create_discord_payload(
            &worlds,
            &[],
            None,
            false,
            1,
            false,
            None,
            false,
            2,
            &no_mappings,
        );
        // Continuation chunks should have no content
        assert!(!payload.contains_key("content"));
    }

    #[test]
    fn test_payload_forum_adds_thread_name() {
        let worlds = vec![make_world("My World", "wrld_456")];
        let no_mappings = HashMap::new();
        let (payload, _) = create_discord_payload(
            &worlds,
            &[],
            None,
            true,
            0,
            true,
            None,
            false,
            2,
            &no_mappings,
        );
        assert!(payload.contains_key("thread_name"));
        let thread_name = payload.get("thread_name").unwrap();
        assert!(thread_name.contains("My World"));
    }

    #[test]
    fn test_payload_singular_photo() {
        let no_mappings = HashMap::new();
        let (payload, _) =
            create_discord_payload(&[], &[], None, true, 0, false, None, false, 1, &no_mappings);
        let content = payload.get("content").unwrap();
        assert!(content.contains("Photo"));
        assert!(!content.contains("Photos"));
    }

    #[test]
    fn test_payload_plural_photos() {
        let no_mappings = HashMap::new();
        let (payload, _) =
            create_discord_payload(&[], &[], None, true, 0, false, None, false, 2, &no_mappings);
        let content = payload.get("content").unwrap();
        assert!(content.contains("Photos"));
    }

    #[test]
    fn test_payload_with_players() {
        let worlds = vec![make_world("W", "wrld_1")];
        let players = vec![make_player("Alice"), make_player("Bob")];
        let no_mappings = HashMap::new();
        let (payload, overflow) = create_discord_payload(
            &worlds,
            &players,
            None,
            true,
            0,
            false,
            None,
            true,
            2,
            &no_mappings,
        );
        let content = payload.get("content").unwrap();
        assert!(content.contains("Alice"));
        assert!(content.contains("Bob"));
        assert!(overflow.is_empty());
    }

    #[test]
    fn test_payload_without_player_names_flag() {
        let worlds = vec![make_world("W", "wrld_1")];
        let players = vec![make_player("Alice")];
        let no_mappings = HashMap::new();
        let (payload, _) = create_discord_payload(
            &worlds,
            &players,
            None,
            true,
            0,
            false,
            None,
            false,
            2,
            &no_mappings,
        );
        let content = payload.get("content").unwrap();
        assert!(!content.contains("Alice"));
    }

    // --- create_metadata_key tests ---

    #[test]
    fn test_metadata_key_with_world_and_time() {
        let meta = make_metadata("W", "wrld_abc");
        let key = create_metadata_key(&meta, Some(3600), 3600, false, true);
        assert_eq!(key, "wrld_abc_t1");
    }

    #[test]
    fn test_metadata_key_no_time_limit() {
        let meta = make_metadata("W", "wrld_abc");
        let key = create_metadata_key(&meta, Some(3600), 3600, true, true);
        assert_eq!(key, "wrld_abc_all");
    }

    #[test]
    fn test_metadata_key_no_world_grouping() {
        let meta = make_metadata("W", "wrld_abc");
        let key = create_metadata_key(&meta, Some(3600), 3600, false, false);
        assert_eq!(key, "any_world_t1");
    }

    #[test]
    fn test_metadata_key_no_world_no_time() {
        let meta = make_metadata("W", "wrld_abc");
        let key = create_metadata_key(&meta, Some(3600), 3600, true, false);
        assert_eq!(key, "any_world_all");
    }

    #[test]
    fn test_metadata_key_no_world_in_metadata() {
        let mut meta = make_metadata("W", "wrld_abc");
        meta.world = None;
        let key = create_metadata_key(&meta, Some(3600), 3600, false, true);
        assert_eq!(key, "unknown_t1");
    }

    // --- create_thread_title tests ---

    #[test]
    fn test_thread_title_single_world() {
        let worlds = vec![make_world("Cool Place", "wrld_1")];
        let title = create_thread_title(&worlds, 5);
        assert!(title.contains("Cool Place"));
        assert!(title.contains("Photos"));
    }

    #[test]
    fn test_thread_title_multiple_worlds() {
        let worlds = vec![
            make_world("World A", "wrld_a"),
            make_world("World B", "wrld_b"),
        ];
        let title = create_thread_title(&worlds, 3);
        assert!(title.contains("World A"));
        assert!(title.contains("World B"));
    }

    #[test]
    fn test_thread_title_truncated_at_100() {
        let worlds = vec![
            make_world("A Very Long World Name That Takes Up Space", "wrld_1"),
            make_world("Another Long World Name To Push Over Limit", "wrld_2"),
        ];
        let title = create_thread_title(&worlds, 5);
        assert!(
            title.len() <= 100,
            "Title should be at most 100 chars: len={}",
            title.len()
        );
    }

    #[test]
    fn test_thread_title_no_worlds() {
        let title = create_thread_title(&[], 3);
        assert!(title.contains("Photos"));
    }

    #[test]
    fn test_thread_title_single_photo() {
        let title = create_thread_title(&[], 1);
        assert!(title.contains("Photo"));
        assert!(!title.contains("Photos"));
    }

    // --- create_message_content_with_players tests ---

    #[test]
    fn test_content_with_players_fits() {
        let worlds = vec![make_world("W", "wrld_1")];
        let players = vec![make_player("Alice"), make_player("Bob")];
        let no_mappings = HashMap::new();
        let (content, remaining, had_players) =
            create_message_content_with_players(&worlds, &players, None, true, 2, &no_mappings);
        assert!(content.contains("Alice"));
        assert!(content.contains("Bob"));
        assert!(remaining.is_empty());
        assert!(had_players);
    }

    #[test]
    fn test_content_no_players_when_disabled() {
        let worlds = vec![make_world("W", "wrld_1")];
        let players = vec![make_player("Alice")];
        let no_mappings = HashMap::new();
        let (content, remaining, had_players) =
            create_message_content_with_players(&worlds, &players, None, false, 2, &no_mappings);
        assert!(!content.contains("Alice"));
        assert!(remaining.is_empty());
        assert!(!had_players);
    }

    #[test]
    fn test_content_overflows_players() {
        let worlds = vec![make_world("W", "wrld_1")];
        // Create enough players to overflow 1900 chars
        let players: Vec<PlayerInfo> = (0..200)
            .map(|i| make_player(&format!("Player_{i:04}")))
            .collect();
        let no_mappings = HashMap::new();
        let (content, remaining, had_players) =
            create_message_content_with_players(&worlds, &players, None, true, 5, &no_mappings);
        assert!(content.len() <= 1901, "Content too long: {}", content.len());
        assert!(!remaining.is_empty(), "Should have overflow players");
        assert!(had_players);
    }

    // --- create_overflow_player_messages tests ---

    #[test]
    fn test_overflow_single_message() {
        let players = vec![make_player("Alice"), make_player("Bob")];
        let no_mappings = HashMap::new();
        let msgs = create_overflow_player_messages(&players, true, &no_mappings);
        assert_eq!(msgs.len(), 1);
        assert!(msgs[0].contains("Alice"));
        assert!(msgs[0].contains("Bob"));
    }

    #[test]
    fn test_overflow_with_prefix_when_no_main_players() {
        let players = vec![make_player("Alice")];
        let no_mappings = HashMap::new();
        let msgs = create_overflow_player_messages(&players, false, &no_mappings);
        assert_eq!(msgs.len(), 1);
        assert!(msgs[0].starts_with("with "));
    }

    #[test]
    fn test_overflow_multiple_messages() {
        // Create enough players to need multiple messages (>1900 chars each)
        let players: Vec<PlayerInfo> = (0..300)
            .map(|i| make_player(&format!("LongPlayerName_{i:04}")))
            .collect();
        let no_mappings = HashMap::new();
        let msgs = create_overflow_player_messages(&players, true, &no_mappings);
        assert!(
            msgs.len() > 1,
            "Should need multiple messages for {} players",
            players.len()
        );
        for msg in &msgs {
            assert!(msg.len() <= 1901, "Message too long: {}", msg.len());
        }
    }

    // --- create_worlds_only_message tests ---

    #[test]
    fn test_worlds_only_with_worlds() {
        let worlds = vec![make_world("Cool Place", "wrld_1")];
        let msg = create_worlds_only_message(&worlds, Some(12345), 3);
        assert!(msg.contains("Cool Place"));
        assert!(msg.contains("<t:12345:f>"));
    }

    #[test]
    fn test_worlds_only_no_worlds() {
        let msg = create_worlds_only_message(&[], Some(12345), 2);
        assert!(msg.contains("Photos"));
        assert!(msg.contains("<t:12345:f>"));
    }

    #[test]
    fn test_worlds_only_no_timestamp() {
        let worlds = vec![make_world("W", "wrld_1")];
        let msg = create_worlds_only_message(&worlds, None, 1);
        assert!(!msg.contains("<t:"));
    }

    // --- create_compact_world_messages tests ---

    #[test]
    fn test_compact_worlds_empty() {
        let (summary, links) = create_compact_world_messages(&[], 2);
        assert!(summary.contains("Photos"));
        assert!(links.is_empty());
    }

    #[test]
    fn test_compact_worlds_with_worlds() {
        let worlds = vec![
            make_world("World A", "wrld_a"),
            make_world("World B", "wrld_b"),
        ];
        let (summary, links) = create_compact_world_messages(&worlds, 3);
        assert!(summary.contains("World A"));
        assert!(summary.contains("World B"));
        assert!(summary.contains("2 worlds"));
        assert!(!links.is_empty());
    }

    // --- create_split_player_messages tests ---

    #[test]
    fn test_split_players_empty() {
        let no_mappings = HashMap::new();
        let msgs = create_split_player_messages(&[], &no_mappings);
        assert!(msgs.is_empty());
    }

    #[test]
    fn test_split_players_single() {
        let players = vec![make_player("Alice")];
        let no_mappings = HashMap::new();
        let msgs = create_split_player_messages(&players, &no_mappings);
        assert_eq!(msgs.len(), 1);
        assert!(msgs[0].contains("with "));
        assert!(msgs[0].contains("Alice"));
    }

    #[test]
    fn test_split_players_multiple() {
        let players = vec![
            make_player("Alice"),
            make_player("Bob"),
            make_player("Charlie"),
        ];
        let no_mappings = HashMap::new();
        let msgs = create_split_player_messages(&players, &no_mappings);
        assert_eq!(msgs.len(), 1);
        assert!(msgs[0].contains("Alice"));
        assert!(msgs[0].contains("Bob"));
        assert!(msgs[0].contains("Charlie"));
    }

    #[test]
    fn test_split_players_overflow() {
        let players: Vec<PlayerInfo> = (0..300)
            .map(|i| make_player(&format!("LongName_{i:04}")))
            .collect();
        let no_mappings = HashMap::new();
        let msgs = create_split_player_messages(&players, &no_mappings);
        assert!(msgs.len() > 1);
        for msg in &msgs {
            assert!(msg.len() <= 1901, "Message too long: {}", msg.len());
        }
    }

    // --- format_player_for_discord tests ---

    #[test]
    fn test_format_player_unmapped() {
        let player = make_player("Alice");
        let no_mappings = HashMap::new();
        let result = format_player_for_discord(&player, &no_mappings);
        assert_eq!(result, "**Alice**");
    }

    #[test]
    fn test_format_player_mapped_by_id() {
        let player = make_player("Alice");
        let mut mappings = HashMap::new();
        mappings.insert(player.id.clone(), "123456789".to_string());
        let result = format_player_for_discord(&player, &mappings);
        assert_eq!(result, "<@123456789>");
    }

    #[test]
    fn test_format_player_mapped_by_display_name() {
        let player = make_player("Alice");
        let mut mappings = HashMap::new();
        // Keys are lowercased (matching how the upload pipeline builds the map)
        mappings.insert("alice".to_string(), "987654321".to_string());
        let result = format_player_for_discord(&player, &mappings);
        assert_eq!(result, "<@987654321>");
    }

    #[test]
    fn test_format_player_id_takes_priority_over_name() {
        let player = make_player("Alice");
        let mut mappings = HashMap::new();
        mappings.insert(player.id.clone(), "111111111".to_string());
        mappings.insert("alice".to_string(), "222222222".to_string());
        let result = format_player_for_discord(&player, &mappings);
        // ID mapping should take priority
        assert_eq!(result, "<@111111111>");
    }

    #[test]
    fn test_format_player_case_insensitive() {
        let player = make_player("Alice"); // display_name = "Alice"
        let mut mappings = HashMap::new();
        // Lowercase key matches uppercase display name
        mappings.insert("alice".to_string(), "555555555".to_string());
        let result = format_player_for_discord(&player, &mappings);
        assert_eq!(result, "<@555555555>");
    }

    #[test]
    fn test_payload_with_discord_mappings() {
        let worlds = vec![make_world("W", "wrld_1")];
        let players = vec![make_player("Alice"), make_player("Bob")];
        let mut mappings = HashMap::new();
        mappings.insert("usr_alice".to_string(), "123456789".to_string());
        let (payload, _) = create_discord_payload(
            &worlds, &players, None, true, 0, false, None, true, 2, &mappings,
        );
        let content = payload.get("content").unwrap();
        assert!(
            content.contains("<@123456789>"),
            "Alice should be tagged: {content}"
        );
        assert!(content.contains("**Bob**"), "Bob should be bold: {content}");
    }
}
