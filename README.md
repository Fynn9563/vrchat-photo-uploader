# VRChat Photo Uploader

![Version](https://img.shields.io/github/v/release/fynn9563/vrchat-photo-uploader)
![License](https://img.shields.io/github/license/fynn9563/vrchat-photo-uploader)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-brightgreen)

A powerful desktop application for uploading VRChat photos to Discord with intelligent grouping, automatic metadata extraction, and seamless integration.

## ✨ Features

- **🏷️ Automatic Metadata Extraction** - Reads VRChat photo metadata (world, players, timestamps)
- **📋 Smart Grouping** - Groups photos by world and players for organized uploads
- **🗜️ Intelligent Compression** - Compresses images only when Discord requires it
- **🔄 Robust Retry System** - Automatically retries failed uploads with exponential backoff
- **📊 Progress Tracking** - Real-time upload progress with detailed statistics
- **💬 Discord Forum Support** - Creates threaded posts in Discord forum channels
- **🎨 Dark/Light Theme** - Automatic theme switching based on system preference
- **⚡ Global Shortcuts** - Quick access with `Ctrl+Shift+U` for file selection
- **👁️ Image Previews** - `Ctrl+Hover` for instant thumbnail previews
- **📝 Metadata Editor** - Edit and embed custom metadata into images

## 🚀 Quick Start

1. **Download** the latest release for your platform from the [Releases page](https://github.com/fynn9563/vrchat-photo-uploader/releases)
2. **Install** the application:
   - **Windows**: Run the `.msi` installer or use the portable `.exe`
   - **macOS**: Open the `.dmg` file and drag to Applications
   - **Linux**: Use the `.AppImage` (portable) or install the `.deb` package
3. **Configure** your Discord webhook in the app
4. **Select** your VRChat photos and upload!

## 📦 Installation

### Windows
- **Installer**: Download `VRChat-Photo-Uploader-v{version}-x64.msi` for guided installation
- **Portable**: Download `VRChat-Photo-Uploader-v{version}-x64.exe` for standalone use

### macOS
- **Apple Silicon (M1/M2)**: Download `VRChat-Photo-Uploader-v{version}-aarch64.dmg`
- **Intel**: Download `VRChat-Photo-Uploader-v{version}-x64.dmg`

### Linux
- **AppImage (Portable)**: Download `VRChat-Photo-Uploader-v{version}-x86_64.AppImage`
- **Debian/Ubuntu**: Download `vrchat-photo-uploader_{version}_amd64.deb`

## 🛠️ Setup

### Discord Webhook Configuration

1. In Discord, go to **Server Settings** → **Integrations** → **Webhooks**
2. Click **New Webhook** and configure:
   - Name your webhook
   - Select the target channel
   - Copy the webhook URL
3. In the app, click **🔧 Manage** next to webhook selection
4. Add your webhook URL and name
5. Enable **Forum Channel** if uploading to a Discord forum channel

### VRChat Folder Setup

1. Click **⚙️ Preferences** in the app
2. Set your VRChat photos folder (usually `%USERPROFILE%\Pictures\VRChat`)
3. Or use **📂 Open VRChat Folder** to browse

## 📋 Usage

### Basic Upload

1. **Select Webhook** from the dropdown
2. **Configure Settings**:
   - ✅ Group by metadata (recommended)
   - 📊 Max images per message (1-10)
   - 👥 Include player names in posts
3. **Add Photos**: Drag & drop or click to browse
4. **Review Queue**: Select/deselect photos as needed
5. **Start Upload**: Click 🚀 **Start Upload**

### Advanced Features

- **Metadata Editing**: Select photos and click **✏️ Edit Metadata** to modify world/player information
- **Forum Channels**: Enable forum mode for threaded uploads with automatic continuation
- **Keyboard Shortcuts**:
  - `Ctrl+Shift+U` - Quick file selection
  - `Ctrl+Hover` - Preview images in queue

## ⚙️ System Requirements

- **Windows**: Windows 10 or later
- **macOS**: macOS 10.15 (Catalina) or later
- **Linux**: Any modern distribution with GTK 3.24+

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Built with [Tauri](https://tauri.app/) for cross-platform desktop development
- Made with ❤️ for the VRChat community
- Created by **Fynn9563**

## 📞 Support

- 🐛 **Bug Reports**: [GitHub Issues](https://github.com/fynn9563/vrchat-photo-uploader/issues)
- 💡 **Feature Requests**: [GitHub Discussions](https://github.com/fynn9563/vrchat-photo-uploader/discussions)
- 📖 **Documentation**: Check the [Wiki](https://github.com/fynn9563/vrchat-photo-uploader/wiki)

---

**Happy uploading!** 📸✨