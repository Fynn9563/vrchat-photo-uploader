# Test Coverage Summary

## Current State: 483 total tests

### Backend (Rust) — 394 tests
- **170 unit tests** (`cargo test --lib`)
- **52 integration tests** (`cargo test --tests`, excluding ignored)
- **16 Discord webhook tests** (`cargo test --test discord_webhook_tests -- --ignored`)
- **170 binary tests** (mirror of unit tests via `--bin`)

### Frontend (TypeScript) — 89 tests
- **19 Tauri command mock tests**
- **20 state management tests**
- **14 event handling tests**
- **36 UI/form validation tests** (existing)

---

## Backend Test Breakdown

### Unit Tests (in-module `#[cfg(test)]`)

| Module | Tests | What's covered |
|--------|-------|----------------|
| `uploader/discord_client.rs` | 41 | Error code parsing, retry logic, thread ID extraction, backoff calculation, webhook ID extraction, retry-after parsing, payload construction |
| `uploader/image_groups.rs` | 33 | Discord payload creation, metadata keys, thread titles, player list overflow, world messages, compact/split formatting |
| `background_watcher.rs` | 17 | `is_image_file` (extensions, case sensitivity), `is_in_ignored_folder` (matching, case insensitive, partial names) |
| `config.rs` | 17 | Default values, serde roundtrip, missing optional fields, Config/AppConfig conversion, validation rules |
| `errors.rs` | 17 | `is_retryable()`/`is_permanent()` classification, helper constructors, Display trait, Into\<String\> |
| `metadata_editor.rs` | 17 | JSON serialization (full/minimal/no-author/no-world/unicode), PNG chunk injection (valid/invalid/replace), tEXt chunk writing (valid/empty/long keyword), CRC32 calculation |
| `test_helpers.rs` | 6 | PNG generation validity, metadata embedding, visible PNG, temp file cleanup |

### Integration Tests (`src-tauri/tests/`)

| Test file | Tests | What's covered |
|-----------|-------|----------------|
| `database_tests.rs` | 16 | Webhook CRUD, duplicate constraints, usage tracking, upload history, sessions, `is_file_processed`, user overrides |
| `metadata_tests.rs` | 18 | VRCX metadata extraction, full structure, no-metadata handling, non-PNG files, VRChat filename timestamps, embed round-trip, `_Modified` suffix, embed preserves image, unicode metadata, overwrite existing, many players |
| `compression_tests.rs` | 12 | `should_compress_image`, WebP/AVIF encoding, `get_image_info`, thumbnails, scale factor |
| `discord_webhook_tests.rs` | 16 | Real Discord API: text messages, single/multi image upload, metadata payloads, player lists, forum threads, error handling, rate limiting, compression→upload (webp, lossless_webp, jpg, avif, png, png_smart) |
| `integration_tests.rs` | 6 | Existing security/validation tests |

### Discord Webhook Tests (`#[ignore]`)

Require env vars — set via `.env` file in `src-tauri/` or shell exports:
```
DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/...
DISCORD_FORUM_WEBHOOK_URL=https://discord.com/api/webhooks/...
```

Run with: `cd src-tauri && cargo test --test discord_webhook_tests -- --ignored --nocapture`

| Test | Channel | Status |
|------|---------|--------|
| `test_send_text_message` | Normal | Passing |
| `test_send_single_image` | Normal | Passing |
| `test_send_multiple_images` | Normal | Passing |
| `test_send_image_with_metadata_message` | Normal | Passing |
| `test_send_message_with_player_list` | Normal | Passing |
| `test_forum_create_thread` | Forum | Requires forum webhook |
| `test_forum_upload_image_to_thread` | Forum | Requires forum webhook |
| `test_forum_full_workflow` | Forum | Requires forum webhook |
| `test_invalid_webhook_url` | N/A | Passing |
| `test_rapid_requests_handled` | Normal | Passing |
| `test_upload_compressed_webp` | Normal | Passing |
| `test_upload_compressed_lossless_webp` | Normal | Passing |
| `test_upload_compressed_jpg` | Normal | Passing |
| `test_upload_compressed_avif` | Normal | Passing |
| `test_upload_compressed_png` | Normal | Passing |
| `test_upload_compressed_png_smart` | Normal | Passing |

---

## Frontend Test Breakdown

| Test file | Tests | What's covered |
|-----------|-------|----------------|
| `tauri-commands.test.ts` | 19 | `invoke()` mocks for all Tauri commands (webhooks, uploads, config, image processing, updates, overrides, error handling) |
| `state-management.test.ts` | 20 | Upload queue (add/remove/clear/select/deselect/filter/status/progress/error/retry), webhook management, progress state, session transitions |
| `event-handling.test.ts` | 14 | Listener registration, event emission, upload progress payloads (active/completed/failed), drag-drop, listener cleanup |
| `ui.test.ts` | 36 | DOM structure, webhook modal, upload settings, file input, drag-drop events, toast container, AppState class, form validation |

---

## Test Infrastructure

### Shared Fixtures (`src-tauri/src/test_helpers.rs`)
- `create_minimal_png()` — valid 1x1 white PNG
- `create_visible_test_png()` — 200x200 colorful gradient PNG (visible in Discord)
- `create_png_with_metadata(json)` — 200x200 PNG with VRCX-style `tEXt` Description chunk
- `create_test_metadata(world_id, world_name, players, timestamp)` — VRCX-compatible JSON
- `create_png_of_size(bytes)` — PNG padded to a target size (for compression tests)
- `create_temp_png(data, name)` — auto-deleting temp file
- `setup_test_db()` — in-memory SQLite with full schema
- `get_test_webhook_url()` / `get_test_forum_webhook_url()` — env var readers

### Dev Dependencies
```toml
[dev-dependencies]
serial_test = "3"
dotenvy = "0.15"
```

### CI Integration (`.github/workflows/test.yaml`)
- `backend-tests` job: `cargo test --lib` + `cargo test --test '*'`
- `frontend-tests` job: `pnpm test:run --coverage`
- `discord-integration-tests` job: runs `--ignored` tests on push/dispatch only, uses GitHub Secrets
- `lint-and-format` job: `cargo fmt --check` + `cargo clippy` + `tsc --noEmit`

---

## Running Tests

```bash
# All Rust unit tests
cd src-tauri && cargo test --lib

# All Rust integration tests (excludes #[ignore])
cd src-tauri && cargo test --tests

# Discord webhook tests (requires .env or env vars)
cd src-tauri && cargo test --test discord_webhook_tests -- --ignored --nocapture

# All frontend tests
pnpm test:run

# Everything except Discord tests
cd src-tauri && cargo test && cd .. && pnpm test:run

# Clippy (all targets including tests)
cd src-tauri && cargo clippy --all-targets
```
