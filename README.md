# VRChat Photo Uploader

![Version](https://img.shields.io/github/v/release/fynn9563/vrchat-photo-uploader)
![License](https://img.shields.io/github/license/fynn9563/vrchat-photo-uploader?branch=master)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux-brightgreen)
![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri-blue)

A powerful desktop application for uploading VRChat photos to Discord with intelligent grouping, automatic metadata extraction, and seamless integration.

> **Disclaimer**: This application is not affiliated with, endorsed by, or sponsored by VRChat Inc. or Discord Inc. VRChat and Discord are trademarks of their respective owners. This is an independent community tool.

## Features

### Smart Grouping
- **Group by World** - Automatically groups photos taken in the same VRChat world
- **Group by Time** - Configurable time windows (5 min to 2 hours, or custom)
- **Combined Grouping** - Use both world and time-based grouping together
- **Individual Mode** - Upload each photo as a separate message
- **Merge No Metadata** - Optionally merge photos without metadata into adjacent groups

### Metadata Extraction
- **Automatic Detection** - Reads embedded VRCX metadata from PNG files
- **VRChat Native Support** - Falls back to VRChat's native XMP metadata when VRCX data is unavailable
- **World Information** - Extracts world name, ID, and instance details
- **Player Lists** - Captures all players present when the photo was taken
- **Timestamps** - Preserves original photo timestamps in Discord posts
- **VRChat & VRCX Links** - Automatically generates clickable world links

### Discord Integration
- **Multiple Webhooks** - Save and manage multiple Discord webhooks
- **Multi-Webhook Upload** - Upload the same photos to multiple webhooks in a single session
- **Webhook Pinning** - Pin important webhooks to the top of all dropdown lists
- **Forum Channel Support** - Per-webhook forum channel setting for threaded posts
- **Single Thread Mode** - Post all groups to a single forum thread
- **Smart Message Formatting** - Automatically handles Discord's 2000 character limit
- **Batched Uploads** - Configurable images per message (1-10)
- **Player Name Lists** - Optional player names in post content
- **Discord @Mentions** - Map VRChat players to Discord user IDs for automatic tagging
- **User Webhook Overrides** - Redirect specific players' photos to different webhooks

### Background Auto-Upload
- **Folder Watching** - Automatically detects new photos in your VRChat folder
- **Configurable Delay** - Wait for additional photos before uploading
- **Batch Processing** - Upload multiple photos at once
- **Independent Settings** - Separate configuration for background uploads
- **Stop Control** - Cancel background uploads at any time

### Image Processing
- **Automatic Compression** - Compresses only when Discord's file size limit requires it
- **Multiple Formats** - WebP (lossy/lossless), AVIF, PNG, and JPEG support
- **Tiered Fallback** - Progressively increases compression if uploads fail
- **Resolution Scaling** - Automatic downscaling for extremely large images
- **Intelligent Chunking** - Splits large uploads to stay under Discord limits

### User Experience
- **Drag & Drop** - Simply drag photos into the app
- **Real-time Progress** - Live upload progress with ETA
- **Dark/Light Theme** - Follows system preference or manual selection
- **Global Shortcuts** - `Ctrl+Shift+U` to open file picker from anywhere
- **Image Previews** - `Ctrl+Hover` for instant thumbnail previews
- **Upload Notifications** - Optional system notifications on completion
- **Auto Updates** - Built-in update checker

### Tools
- **Metadata Editor** - View, edit, and embed metadata into PNG files
- **Discord Player Tagging** - Map VRChat players to Discord @mentions
- **VRChat Folder Quick Access** - One-click access to your photos folder
- **Background Queue Monitor** - View and control background uploads

## Installation

### Windows
Download from the [Releases page](https://github.com/fynn9563/vrchat-photo-uploader/releases):
- **Installer**: `VRChat-Photo-Uploader_{version}_x64-setup.exe` - Recommended for most users (NSIS)

### Linux
- **AppImage**: `VRChat-Photo-Uploader_{version}_amd64.AppImage` - Portable, works on most distros
- **Debian/Ubuntu**: `VRChat-Photo-Uploader_{version}_amd64.deb` - Native package
- **RPM**: `VRChat-Photo-Uploader-{version}-1.x86_64.rpm` - Fedora/openSUSE

## Quick Start

1. **Add a Webhook**
   - In Discord: Server Settings > Integrations > Webhooks > New Webhook
   - Copy the webhook URL
   - In the app: Click **Manage** > paste URL > **Add Webhook**

2. **Configure Settings**
   - When adding a webhook, enable **Forum Channel** if it targets a Discord forum
   - Enable **Smart Grouping** for automatic organization
   - Adjust **Images per Message** (default: 10)
   - Optionally enable **Multi-Webhook Upload** in Settings to upload to multiple webhooks at once

3. **Upload Photos**
   - Drag & drop photos or click to browse
   - Review the queue and adjust selections
   - Click **Start Upload**

## Settings

### Upload Settings
| Setting | Description |
|---------|-------------|
| Images per Message | Maximum attachments per Discord message (1-10) |
| Smart Grouping | Automatically organizes photos by metadata |
| Group by World | Groups photos from the same VRChat world |
| Group by Time | Groups photos within a time window |
| Include Player Names | Lists players in the post content |
| Single Thread Mode | Posts all groups to a single forum thread (shown for forum webhooks) |
| Merge No Metadata | Includes photos without metadata in adjacent groups |

### Webhook Settings
| Setting | Description |
|---------|-------------|
| Forum Channel | Per-webhook toggle — creates threaded posts in forum channels |
| Pin Webhook | Pin a webhook to the top of all dropdown lists |
| Multi-Webhook Upload | Upload to multiple webhooks in a single session |
| Discord Player Tagging | Map VRChat players to Discord user IDs for @mentions |

### Background Auto-Upload Settings
| Setting | Description |
|---------|-------------|
| Enable Background Watcher | Automatically upload new photos |
| VRChat Folder | Path to watch for new photos |
| Upload Delay | Seconds to wait for additional photos |
| Batch Size | Maximum images per message |
| Target Webhooks | Select one or more webhooks for auto-uploads |

### Preferences
| Setting | Description |
|---------|-------------|
| Theme | Dark, Light, or Auto (follows system) |
| Notifications | Show system notifications on upload completion |
| Global Shortcuts | Enable `Ctrl+Shift+U` system-wide shortcut |
| Image Previews | Enable `Ctrl+Hover` thumbnail previews |
| Compression Format | WebP, WebP Lossless, AVIF, PNG, or JPEG |
| Multi-Webhook Upload | Enable selecting multiple webhooks for upload |

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+Shift+U` | Open file picker (global, when enabled) |
| `Ctrl+Hover` | Preview image thumbnail in queue |

## System Requirements

- **Windows**: Windows 10 or later (x64)
- **Linux**: Any modern distribution with GTK 3.24+ (x64)

## Important Note: File Size Limits

> **⚠️ Discord servers without Nitro have an 8MB file upload limit per file.**
>
> This app includes safeguards to handle large files:
> - Automatic compression (WebP/JPEG) when files exceed Discord's limit
> - Multi-tier fallback compression (progressively reduces quality and resolution)
> - Intelligent chunking to split uploads across multiple messages
> - Automatic retry with smaller payloads on failure
>
> However, **uploads may still fail** if images are extremely large or if compression cannot reduce them enough. For best results with non-Nitro servers, consider using lower resolution screenshots or enabling compression in your VRChat settings.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Support

- **Bug Reports**: [GitHub Issues](https://github.com/fynn9563/vrchat-photo-uploader/issues)
- **Feature Requests**: [GitHub Issues](https://github.com/fynn9563/vrchat-photo-uploader/issues)

---

Created by **Fynn9563** | Made with ❤️ for the VRChat community
