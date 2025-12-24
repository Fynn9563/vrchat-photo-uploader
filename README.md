# VRChat Photo Uploader

![Version](https://img.shields.io/github/v/release/fynn9563/vrchat-photo-uploader)
![License](https://img.shields.io/github/license/fynn9563/vrchat-photo-uploader?branch=master)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux-brightgreen)
![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri-blue)

A powerful desktop application for uploading VRChat photos to Discord with intelligent grouping, automatic metadata extraction, and seamless integration.

## Features

### Smart Grouping
- **Group by World** - Automatically groups photos taken in the same VRChat world
- **Group by Time** - Configurable time windows (5 min to 2 hours, or custom)
- **Combined Grouping** - Use both world and time-based grouping together
- **Individual Mode** - Upload each photo as a separate message

### Metadata Extraction
- **Automatic Detection** - Reads embedded VRCX metadata from PNG files
- **World Information** - Extracts world name, ID, and instance details
- **Player Lists** - Captures all players present when the photo was taken
- **Timestamps** - Preserves original photo timestamps in Discord posts
- **VRChat & VRCX Links** - Automatically generates clickable world links

### Discord Integration
- **Multiple Webhooks** - Save and manage multiple Discord webhooks
- **Forum Channel Support** - Create threaded posts in Discord forum channels
- **Smart Message Formatting** - Automatically handles Discord's 2000 character limit
- **Batched Uploads** - Configurable images per message (1-10)
- **Player Name Lists** - Optional player names in post content

### Image Processing
- **Automatic Compression** - Compresses only when Discord's file size limit requires it
- **WebP & JPEG Support** - Choose your preferred compression format
- **Intelligent Chunking** - Splits large uploads to stay under Discord limits
- **Retry on Failure** - Automatic retry with payload splitting for 413/400 errors

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
- **VRChat Folder Quick Access** - One-click access to your photos folder

## Installation

### Windows
Download from the [Releases page](https://github.com/fynn9563/vrchat-photo-uploader/releases):
- **Installer**: `VRChat-Photo-Uploader-v{version}-x64.msi` - Recommended for most users
- **Portable**: `VRChat-Photo-Uploader-v{version}-x64.exe` - No installation required

### Linux
- **AppImage**: `VRChat-Photo-Uploader-v{version}-x86_64.AppImage` - Portable, works on most distros
- **Debian/Ubuntu**: `vrchat-photo-uploader_{version}_amd64.deb` - Native package

## Quick Start

1. **Add a Webhook**
   - In Discord: Server Settings > Integrations > Webhooks > New Webhook
   - Copy the webhook URL
   - In the app: Click **Manage** > paste URL > **Add Webhook**

2. **Configure Settings**
   - Enable **Forum Channel** if posting to a Discord forum
   - Enable **Smart Grouping** for automatic organization
   - Adjust **Images per Message** (default: 10)

3. **Upload Photos**
   - Drag & drop photos or click to browse
   - Review the queue and adjust selections
   - Click **Start Upload**

## Settings

### Upload Settings
| Setting | Description |
|---------|-------------|
| Forum Channel | Creates threaded posts in forum channels |
| Images per Message | Maximum attachments per Discord message (1-10) |
| Smart Grouping | Automatically organizes photos by metadata |
| Group by World | Groups photos from the same VRChat world |
| Group by Time | Groups photos within a time window |
| Include Player Names | Lists players in the post content |

### Preferences
| Setting | Description |
|---------|-------------|
| Theme | Dark, Light, or Auto (follows system) |
| Notifications | Show system notifications on upload completion |
| Global Shortcuts | Enable `Ctrl+Shift+U` system-wide shortcut |
| Image Previews | Enable `Ctrl+Hover` thumbnail previews |
| Compression Format | WebP (smaller) or JPEG (compatible) |

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
