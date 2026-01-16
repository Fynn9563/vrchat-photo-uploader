# Changelog

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
