# Changelog

## [4.1.0] - 2026-02-23

### Added
- **Multi-webhook upload** — Upload the same photos to multiple Discord webhooks in a single session. Enable in Settings, then select multiple webhooks from the dropdown. Progress shows which webhook is being processed (e.g., "VRC Photos (2/3)")
- **Discord player tagging (@mentions)** — Map VRChat players to Discord user IDs so they show up as @mentions in your uploads instead of plain text. Manage mappings in the "Discord Player Tagging" section under Manage Webhooks
- **Webhook pinning** — Pin important webhooks to the top of all dropdown lists. Pin icon appears on hover and persists across sessions
- **Multi-webhook auto-upload** — Background auto-upload now supports uploading to multiple webhooks when multi-webhook mode is enabled

### Changed
- **Forum channel is now per-webhook** — Set when adding or editing a webhook, instead of toggling it globally before each upload
- **Redesigned webhook selector** — New custom dropdown that supports both single and multi-select modes
- **Single Thread Mode** — Now only shown when a forum channel webhook is selected
- **Removed "Default Forum Channel mode" setting** — Replaced by "Enable multi-webhook upload" in Settings
- **Removed auto-upload "Forum Channel" toggle** — Forum behavior is now determined from each webhook's own setting
- **Webhook selection remembered** — Your multi-webhook selections are saved and restored on startup

## [4.0.0] - 2026-02-22

### Added
- **Comprehensive test suite** — 483 tests (394 Rust backend + 89 TypeScript frontend)
  - Unit tests for discord client, image grouping, background watcher, config, errors, and metadata editor
  - Integration tests for database CRUD, metadata extraction/embedding, image compression, and security
  - Real Discord webhook tests (text, images, forum threads, compression-to-upload for all 6 formats)
  - Frontend tests for Tauri command mocks, state management, event handling, and UI validation
  - Shared test infrastructure with programmatic PNG generation, VRCX metadata fixtures, and in-memory SQLite
  - CI integration with GitHub Secrets for Discord webhook tests on push

### Changed
- **Migrated from Tauri v1 to Tauri v2** - Major framework upgrade for improved performance, security, and long-term support
- Upgraded all Tauri plugins to v2 (dialog, fs, notification, shell, updater, global-shortcut, os)
- Updated auto-updater to Tauri v2 updater plugin with v1-compatible artifact format for seamless upgrades
- Migrated from Tauri v1 allowlist to v2 capabilities/permissions system
- Updated CI/CD workflows for Tauri v2 (env vars, build action, system dependencies)
- Removed webkit2gtk 4.0 compatibility symlinks (Tauri v2 uses 4.1 natively)

### Fixed
- Fixed auto-updater "Could not fetch a valid release JSON" error
- Preserved localStorage data for existing Windows users during upgrade (`useHttpsScheme: true`)

## [3.1.1] - 2026-02-22

### Fixed
- Fixed auto-updater failing on Windows for users who installed via the NSIS `.exe` setup
- Changed bundle targets from "all" to specific installers so the update manifest correctly maps to NSIS instead of MSI
- App now restarts after installing an update to ensure the installer can replace files

## [3.1.0] - 2025-01-17

### Added
- **AVIF compression format** - Best compression ratio, NASM-optimized in release builds
- **Ignored folders for auto-upload** - Skip specific folders (e.g., Emoji, Stickers, Prints) during background uploads
- **Multi-threaded compression** - All image compression now runs in background threads (non-blocking UI)
- Stop button for background queue uploads
- Background Queue button now only shows when auto-upload is enabled
- Proper "Photo" vs "Photos" pluralization based on image count
- Better Discord error messages (code 220003 for non-forum webhooks, 40005 for large files, etc.)
- Clear error messages when retrying uploads
- Validation for background watcher settings (requires webhook and VRChat folder to be set)

### Changed
- Auto-upload settings are now collapsed/hidden until "Enable background watcher" is checked
- Added AVIF support to background watcher file detection
- Background uploads now properly cancel when auto-upload is disabled mid-upload

### Fixed
- Fixed 40005 "Request entity too large" error not triggering compression fallback
- Fixed error messages not clearing when retrying uploads
- Fixed background upload panel appearing incorrectly on manual upload completion
- Removed unused functions (cleanup)

## [3.0.6] - 2025-01-17

### Fixed
- Fixed background forum uploads
- UI cleanup and improvements

## [3.0.4] - 2025-01-16

### Added
- VRChat native XMP metadata extraction fallback when VRCX metadata is unavailable

## [3.0.3] - 2025-01-15

### Added
- Compression format choice (WebP, WebP Lossless, PNG, JPEG)
- Message splitting for long player lists exceeding Discord's 2000 character limit
- Tiered compression fallback system

### Fixed
- Applied cargo fmt formatting

## [3.0.2] - 2025-01-14

### Fixed
- Various bug fixes and stability improvements

## [3.0.0] - 2025-01-13

### Added
- Smart grouping by world and time window
- Background auto-upload feature with folder watching
- Single thread mode for forum channels
- Merge no metadata option
- User webhook overrides for specific players
- Configurable time windows for grouping
- Default forum mode setting
- Auto-upload settings (delay, batch size, forum channel, etc.)

### Changed
- Complete rewrite with improved architecture
- Better progress tracking and UI feedback
- Enhanced metadata extraction

## [2.1.4] - 2025-01-12

### Fixed
- Release notes comparison with actual last git tag

## [2.1.3] - 2025-01-11

### Fixed
- PowerShell compatibility issue in release notes generation

## [2.1.2] - 2025-01-10

### Changed
- Enhanced release notes with dynamic commit messages and better formatting

## [2.1.1] - 2025-01-09

### Fixed
- Added public key to tauri.conf.json
- Synced all versions to 2.1.0

## [2.0.x] - Previous Releases

### Features
- Discord webhook management
- Forum channel support
- Image compression (WebP/JPEG)
- Drag and drop support
- Real-time upload progress
- Dark/Light theme support
- Global shortcuts
- Image previews on hover
- System notifications
- Auto updates

---

For older releases, see [GitHub Releases](https://github.com/fynn9563/vrchat-photo-uploader/releases).
