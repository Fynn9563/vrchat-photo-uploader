# Test Improvements Plan

## Current State

### Backend (Rust) - 6/10
- ✅ Security validation (webhook URLs, XSS prevention, path traversal)
- ✅ Basic image processing workflow
- ✅ Error handling and propagation
- ❌ Missing Discord upload tests
- ❌ Missing background watcher tests
- ❌ Missing AVIF/WebP compression tests
- ❌ Missing metadata extraction tests

### Frontend (TypeScript) - 4/10
- ✅ DOM structure verification
- ✅ Basic form validation
- ❌ Mostly placeholder tests
- ❌ No Tauri command integration tests
- ❌ No actual functionality tests
- ❌ Missing error handling tests

---

## Backend Improvements

### 1. Discord Upload Tests (`src-tauri/src/uploader/`)

```rust
// discord_client.rs tests to add:

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_discord_error_codes() {
        // Test error code 220003 (non-forum webhook)
        let error_json = r#"{"code": 220003, "message": "Thread can only be created from a forum channel"}"#;
        let result = parse_discord_error(error_json);
        assert!(result.contains("forum"));

        // Test error code 40005 (file too large)
        let error_json = r#"{"code": 40005, "message": "Request entity too large"}"#;
        let result = parse_discord_error(error_json);
        assert!(result.contains("large") || result.contains("size"));

        // Test error code 50006 (empty message)
        let error_json = r#"{"code": 50006, "message": "Cannot send empty message"}"#;
        let result = parse_discord_error(error_json);
        assert!(result.contains("empty"));
    }

    #[test]
    fn test_webhook_url_parsing() {
        let valid_url = "https://discord.com/api/webhooks/123456789012345678/abcdef";
        assert!(is_valid_discord_webhook(valid_url));

        let invalid_url = "https://example.com/webhook";
        assert!(!is_valid_discord_webhook(invalid_url));
    }

    #[tokio::test]
    async fn test_upload_with_mock_server() {
        // Use wiremock or similar to mock Discord API
        // Test successful upload
        // Test retry on 429 rate limit
        // Test failure on 413 payload too large
    }
}
```

### 2. Background Watcher Tests (`src-tauri/src/background_watcher.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_image_file() {
        assert!(is_image_file("photo.png"));
        assert!(is_image_file("photo.PNG"));
        assert!(is_image_file("photo.jpg"));
        assert!(is_image_file("photo.jpeg"));
        assert!(is_image_file("photo.webp"));
        assert!(is_image_file("photo.avif"));
        assert!(!is_image_file("document.txt"));
        assert!(!is_image_file("video.mp4"));
    }

    #[test]
    fn test_is_in_ignored_folder() {
        let ignored = vec!["Emoji".to_string(), "Stickers".to_string()];

        assert!(is_in_ignored_folder("/path/to/Emoji/image.png", &ignored));
        assert!(is_in_ignored_folder("/path/to/Stickers/image.png", &ignored));
        assert!(!is_in_ignored_folder("/path/to/Photos/image.png", &ignored));
    }

    #[test]
    fn test_batch_grouping() {
        // Test that files are grouped correctly by time window
        // Test that batch size limits are respected
    }
}
```

### 3. AVIF/WebP Compression Tests (`src-tauri/src/image_processor.rs`)

```rust
#[cfg(test)]
mod compression_tests {
    use super::*;

    #[tokio::test]
    async fn test_avif_compression() {
        let (test_path, png_data) = create_test_image();
        // Create larger test image for meaningful compression test

        let result = compress_image_avif(&test_path, 80).await;
        assert!(result.is_ok());

        let compressed_path = result.unwrap();
        let original_size = std::fs::metadata(&test_path).unwrap().len();
        let compressed_size = std::fs::metadata(&compressed_path).unwrap().len();

        // AVIF should be smaller (for real images)
        // For minimal test images, just verify it produces valid output
        assert!(std::path::Path::new(&compressed_path).exists());

        // Cleanup
        let _ = std::fs::remove_file(&test_path);
        let _ = std::fs::remove_file(&compressed_path);
    }

    #[tokio::test]
    async fn test_webp_compression() {
        // Similar to AVIF test
    }

    #[tokio::test]
    async fn test_webp_lossless_compression() {
        // Test lossless mode
    }

    #[tokio::test]
    async fn test_jpeg_compression() {
        // Test JPEG fallback
    }

    #[tokio::test]
    async fn test_tiered_compression_fallback() {
        // Test that compression falls back through tiers
        // Tier 1: Quality 85
        // Tier 2: Quality 70
        // Tier 3: Quality 50 + resize
        // etc.
    }

    #[test]
    fn test_compression_format_selection() {
        assert_eq!(get_compression_format("webp"), CompressionFormat::WebP);
        assert_eq!(get_compression_format("avif"), CompressionFormat::Avif);
        assert_eq!(get_compression_format("png"), CompressionFormat::Png);
        assert_eq!(get_compression_format("jpeg"), CompressionFormat::Jpeg);
    }
}
```

### 4. Metadata Extraction Tests

```rust
#[cfg(test)]
mod metadata_tests {
    use super::*;

    #[tokio::test]
    async fn test_vrcx_metadata_extraction() {
        // Create PNG with embedded VRCX metadata
        // Test extraction of world name, world ID, players, timestamp
    }

    #[tokio::test]
    async fn test_vrchat_native_xmp_extraction() {
        // Create PNG with VRChat native XMP metadata
        // Test fallback when VRCX metadata is missing
    }

    #[tokio::test]
    async fn test_no_metadata_handling() {
        let (test_path, png_data) = create_test_image();
        let mut file = File::create(&test_path).unwrap();
        file.write_all(&png_data).unwrap();

        let result = extract_metadata(&test_path.to_string_lossy()).await;

        // Should return Ok(None) for images without metadata
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        let _ = std::fs::remove_file(&test_path);
    }
}
```

### 5. Image Grouping Tests (`src-tauri/src/uploader/image_groups.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_by_world() {
        let images = vec![
            ImageWithMetadata { world_id: Some("wrld_123".into()), .. },
            ImageWithMetadata { world_id: Some("wrld_123".into()), .. },
            ImageWithMetadata { world_id: Some("wrld_456".into()), .. },
        ];

        let groups = group_images_by_world(&images);
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn test_group_by_time_window() {
        // Test 5 minute window
        // Test 30 minute window
        // Test custom window
    }

    #[test]
    fn test_merge_no_metadata_images() {
        // Test that images without metadata merge into adjacent groups
    }

    #[test]
    fn test_message_length_limits() {
        // Test that messages stay under 2000 chars
        let worlds = generate_many_worlds(50);
        let message = create_worlds_only_message(&worlds, None, 50);
        assert!(message.len() <= 2000);
    }

    #[test]
    fn test_photo_pluralization() {
        let msg1 = create_message(1);
        assert!(msg1.contains("Photo") && !msg1.contains("Photos"));

        let msg2 = create_message(2);
        assert!(msg2.contains("Photos"));
    }
}
```

---

## Frontend Improvements

### 1. Replace Placeholder Tests (`src/test/example.test.ts`)

Delete or replace with actual utility function tests:

```typescript
// utils.test.ts
import { describe, it, expect } from 'vitest';

describe('Utility Functions', () => {
  describe('formatFileSize', () => {
    it('should format bytes correctly', () => {
      expect(formatFileSize(0)).toBe('0 B');
      expect(formatFileSize(1024)).toBe('1 KB');
      expect(formatFileSize(1048576)).toBe('1 MB');
      expect(formatFileSize(8388608)).toBe('8 MB');
    });
  });

  describe('formatTimestamp', () => {
    it('should format Discord timestamps', () => {
      const timestamp = 1705500000;
      const formatted = formatTimestamp(timestamp);
      expect(formatted).toContain('<t:');
    });
  });

  describe('validateWebhookUrl', () => {
    it('should validate Discord webhook URLs', () => {
      expect(validateWebhookUrl('https://discord.com/api/webhooks/123/abc')).toBe(true);
      expect(validateWebhookUrl('https://discordapp.com/api/webhooks/123/abc')).toBe(true);
      expect(validateWebhookUrl('https://example.com/webhook')).toBe(false);
      expect(validateWebhookUrl('not-a-url')).toBe(false);
    });
  });
});
```

### 2. Tauri Command Mock Tests (`src/test/tauri.test.ts`)

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mockTauri } from './setup';

describe('Tauri Command Integration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('Webhook Commands', () => {
    it('should load webhooks from backend', async () => {
      const mockWebhooks = [
        { id: 1, name: 'Test Server', url: 'https://discord.com/api/webhooks/123/abc', is_forum: false }
      ];
      mockTauri.invoke.mockResolvedValue(mockWebhooks);

      const result = await invoke('get_webhooks');

      expect(mockTauri.invoke).toHaveBeenCalledWith('get_webhooks');
      expect(result).toEqual(mockWebhooks);
    });

    it('should add a new webhook', async () => {
      mockTauri.invoke.mockResolvedValue({ id: 2, name: 'New Webhook', url: '...', is_forum: true });

      const result = await invoke('add_webhook', {
        name: 'New Webhook',
        url: 'https://discord.com/api/webhooks/456/def',
        isForum: true
      });

      expect(result.id).toBe(2);
    });

    it('should handle webhook validation errors', async () => {
      mockTauri.invoke.mockRejectedValue(new Error('Invalid webhook URL'));

      await expect(invoke('add_webhook', { name: 'Bad', url: 'invalid' }))
        .rejects.toThrow('Invalid webhook URL');
    });
  });

  describe('Upload Commands', () => {
    it('should start upload with correct parameters', async () => {
      mockTauri.invoke.mockResolvedValue({ success: true, uploaded: 5 });

      const result = await invoke('start_upload', {
        filePaths: ['/path/to/image1.png', '/path/to/image2.png'],
        webhookId: 1,
        maxImages: 10,
        groupByMetadata: true,
        isForumChannel: false
      });

      expect(mockTauri.invoke).toHaveBeenCalledWith('start_upload', expect.any(Object));
      expect(result.success).toBe(true);
    });

    it('should handle upload cancellation', async () => {
      mockTauri.invoke.mockResolvedValue({ cancelled: true });

      const result = await invoke('cancel_upload');
      expect(result.cancelled).toBe(true);
    });
  });

  describe('Config Commands', () => {
    it('should load config from backend', async () => {
      const mockConfig = {
        theme: 'dark',
        notifications_enabled: true,
        compression_format: 'avif',
        default_forum_mode: false
      };
      mockTauri.invoke.mockResolvedValue(mockConfig);

      const result = await invoke('get_config');
      expect(result.compression_format).toBe('avif');
    });

    it('should save config changes', async () => {
      mockTauri.invoke.mockResolvedValue(true);

      await invoke('save_config', { key: 'compression_format', value: 'webp' });

      expect(mockTauri.invoke).toHaveBeenCalledWith('save_config', {
        key: 'compression_format',
        value: 'webp'
      });
    });
  });
});
```

### 3. State Management Tests (`src/test/state.test.ts`)

```typescript
import { describe, it, expect, beforeEach } from 'vitest';

// Import or recreate AppState class
class AppState {
  webhooks: Webhook[] = [];
  uploadQueue: QueueItem[] = [];
  selectedWebhookId: number | null = null;
  isUploading: boolean = false;
  // ... rest of state
}

describe('AppState', () => {
  let state: AppState;

  beforeEach(() => {
    state = new AppState();
  });

  describe('Upload Queue Management', () => {
    it('should add items to queue with correct initial state', () => {
      state.addToQueue({ filePath: '/test/image.png', filename: 'image.png' });

      const item = state.uploadQueue[0];
      expect(item.status).toBe('queued');
      expect(item.selected).toBe(true);
      expect(item.progress).toBe(0);
      expect(item.retryCount).toBe(0);
    });

    it('should not add duplicate files to queue', () => {
      state.addToQueue({ filePath: '/test/image.png', filename: 'image.png' });
      state.addToQueue({ filePath: '/test/image.png', filename: 'image.png' });

      expect(state.uploadQueue.length).toBe(1);
    });

    it('should update item status correctly', () => {
      state.addToQueue({ filePath: '/test/image.png', filename: 'image.png' });
      const itemId = state.uploadQueue[0].id;

      state.updateItemStatus(itemId, 'uploading', 50);

      expect(state.uploadQueue[0].status).toBe('uploading');
      expect(state.uploadQueue[0].progress).toBe(50);
    });

    it('should handle failed items correctly', () => {
      state.addToQueue({ filePath: '/test/image.png', filename: 'image.png' });
      const itemId = state.uploadQueue[0].id;

      state.markItemFailed(itemId, 'Upload timeout');

      expect(state.uploadQueue[0].status).toBe('failed');
      expect(state.uploadQueue[0].error).toBe('Upload timeout');
    });

    it('should get only selected items', () => {
      state.addToQueue({ filePath: '/test/image1.png', filename: 'image1.png' });
      state.addToQueue({ filePath: '/test/image2.png', filename: 'image2.png' });

      state.uploadQueue[0].selected = false;

      const selected = state.getSelectedItems();
      expect(selected.length).toBe(1);
      expect(selected[0].filename).toBe('image2.png');
    });

    it('should get failed items for retry', () => {
      state.addToQueue({ filePath: '/test/image1.png', filename: 'image1.png' });
      state.addToQueue({ filePath: '/test/image2.png', filename: 'image2.png' });

      state.uploadQueue[0].status = 'failed';
      state.uploadQueue[1].status = 'completed';

      const failed = state.getFailedItems();
      expect(failed.length).toBe(1);
    });
  });

  describe('Webhook Management', () => {
    it('should select webhook by id', () => {
      state.webhooks = [
        { id: 1, name: 'Server 1', url: '...', is_forum: false },
        { id: 2, name: 'Server 2', url: '...', is_forum: true }
      ];

      state.selectWebhook(2);

      expect(state.selectedWebhookId).toBe(2);
      expect(state.getSelectedWebhook()?.name).toBe('Server 2');
    });
  });
});
```

### 4. Error Handling Tests (`src/test/errors.test.ts`)

```typescript
import { describe, it, expect, vi } from 'vitest';

describe('Error Handling', () => {
  describe('Discord API Errors', () => {
    it('should display user-friendly message for rate limit', () => {
      const error = { code: 429, message: 'You are being rate limited' };
      const userMessage = formatDiscordError(error);
      expect(userMessage).toContain('rate limit');
      expect(userMessage).toContain('wait');
    });

    it('should display user-friendly message for file too large', () => {
      const error = { code: 40005, message: 'Request entity too large' };
      const userMessage = formatDiscordError(error);
      expect(userMessage).toContain('file');
      expect(userMessage).toContain('large');
    });

    it('should display user-friendly message for non-forum webhook', () => {
      const error = { code: 220003, message: 'Thread can only be created...' };
      const userMessage = formatDiscordError(error);
      expect(userMessage).toContain('forum');
    });
  });

  describe('Network Errors', () => {
    it('should handle timeout errors', () => {
      const error = new Error('ETIMEDOUT');
      const userMessage = formatNetworkError(error);
      expect(userMessage).toContain('timed out');
    });

    it('should handle connection refused', () => {
      const error = new Error('ECONNREFUSED');
      const userMessage = formatNetworkError(error);
      expect(userMessage).toContain('connect');
    });
  });
});
```

### 5. Background Upload UI Tests (`src/test/background-upload.test.ts`)

```typescript
import { describe, it, expect, beforeEach, vi } from 'vitest';

describe('Background Upload UI', () => {
  beforeEach(() => {
    document.body.innerHTML = `
      <div id="backgroundQueuePanel" class="hidden">
        <div id="backgroundQueueItems"></div>
        <button id="stopBackgroundUpload">Stop</button>
        <span id="backgroundProgress">0 / 0</span>
      </div>
      <input type="checkbox" id="enableBackgroundWatcher" />
      <div id="autoUploadSettings" class="hidden">
        <input type="text" id="vrchatFolder" />
        <input type="number" id="uploadDelay" value="30" />
        <input type="text" id="ignoredFolders" />
      </div>
    `;
  });

  it('should show background queue panel only when auto-upload is enabled', () => {
    const panel = document.getElementById('backgroundQueuePanel');
    const checkbox = document.getElementById('enableBackgroundWatcher') as HTMLInputElement;

    expect(panel?.classList.contains('hidden')).toBe(true);

    checkbox.checked = true;
    checkbox.dispatchEvent(new Event('change'));

    // Panel should become visible when there are uploads
    // (actual visibility depends on upload state)
  });

  it('should collapse auto-upload settings when disabled', () => {
    const settings = document.getElementById('autoUploadSettings');
    const checkbox = document.getElementById('enableBackgroundWatcher') as HTMLInputElement;

    expect(settings?.classList.contains('hidden')).toBe(true);

    checkbox.checked = true;
    checkbox.dispatchEvent(new Event('change'));
    settings?.classList.remove('hidden');

    expect(settings?.classList.contains('hidden')).toBe(false);
  });

  it('should parse ignored folders correctly', () => {
    const input = document.getElementById('ignoredFolders') as HTMLInputElement;
    input.value = 'Emoji, Stickers, Prints';

    const folders = parseIgnoredFolders(input.value);
    expect(folders).toEqual(['Emoji', 'Stickers', 'Prints']);
  });

  it('should validate VRChat folder path', () => {
    const input = document.getElementById('vrchatFolder') as HTMLInputElement;

    input.value = 'C:\\Users\\User\\Pictures\\VRChat';
    expect(isValidPath(input.value)).toBe(true);

    input.value = '';
    expect(isValidPath(input.value)).toBe(false);
  });
});
```

---

## Test Infrastructure Improvements

### 1. Add Test Coverage Reporting

**Rust (Cargo.toml):**
```toml
[dev-dependencies]
# ... existing deps
cargo-tarpaulin = "0.27"  # For coverage
```

**TypeScript (package.json):**
```json
{
  "scripts": {
    "test": "vitest",
    "test:coverage": "vitest --coverage",
    "test:ui": "vitest --ui"
  },
  "devDependencies": {
    "@vitest/coverage-v8": "^1.0.0",
    "@vitest/ui": "^1.0.0"
  }
}
```

### 2. Add CI Test Requirements

**GitHub Actions (.github/workflows/test.yaml):**
```yaml
- name: Run Rust tests
  run: cargo test --all-features
  working-directory: src-tauri

- name: Run Frontend tests
  run: npm test -- --run

- name: Check coverage threshold
  run: |
    # Fail if coverage drops below 60%
    npm run test:coverage -- --coverage.thresholds.lines=60
```

### 3. Add Mock Server for Discord API Tests

```rust
// tests/mock_discord.rs
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

pub async fn setup_mock_discord() -> MockServer {
    let mock_server = MockServer::start().await;

    // Mock successful webhook post
    Mock::given(method("POST"))
        .and(path_regex(r"/api/webhooks/\d+/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "123456789",
            "type": 0,
            "content": "Test message"
        })))
        .mount(&mock_server)
        .await;

    mock_server
}
```

---

## Priority Order

1. **High Priority** (Core functionality)
   - Discord upload tests with error handling
   - AVIF/WebP compression tests
   - Background watcher file detection tests

2. **Medium Priority** (User experience)
   - Tauri command integration tests
   - State management tests
   - Error message formatting tests

3. **Low Priority** (Polish)
   - Coverage reporting
   - UI interaction tests
   - Performance benchmarks

---

## Notes

- Consider using `wiremock` or `httpmock` for mocking Discord API in Rust tests
- Frontend tests should use `@tauri-apps/api` mocks from `./setup.ts`
- Real VRChat photos with metadata would be ideal for metadata extraction tests (add to `tests/fixtures/`)
- Keep test images small to avoid bloating the repo
