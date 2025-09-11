# 🔐 Security & Verification Guide

This document explains how to verify the authenticity and integrity of VRChat Photo Uploader releases.

## 📥 Download Verification

### Quick Verification (Recommended)

1. **Download the release** and the corresponding `SHA256SUMS` file from the [Releases page](../../releases)
2. **Verify the checksum**:

#### Windows (PowerShell)
```powershell
# Calculate checksum
$hash = Get-FileHash -Path "VRChat-Photo-Uploader.msi" -Algorithm SHA256
$hash.Hash

# Compare with SHA256SUMS file
Get-Content SHA256SUMS | Select-String "VRChat-Photo-Uploader.msi"
```

#### macOS/Linux
```bash
# Verify checksum
sha256sum -c SHA256SUMS --ignore-missing
```

### Advanced Verification

#### Code Signatures

**Windows (.exe, .msi files)**:
```powershell
# Check Authenticode signature
Get-AuthenticodeSignature .\VRChat-Photo-Uploader.msi
```

**macOS (.dmg files)**:
```bash
# Verify code signature
codesign -dv --verbose=4 VRChat-Photo-Uploader.dmg
spctl -a -t open --context context:primary-signature -v VRChat-Photo-Uploader.dmg
```

**Linux (.deb, .AppImage files)**:
```bash
# Basic integrity check
file VRChat-Photo-Uploader.AppImage
```

## 🏗️ Build Reproducibility

All releases are built using GitHub Actions with the following security measures:

- **Automated builds**: No human interaction with release binaries
- **Dependency auditing**: All dependencies are scanned for known vulnerabilities
- **Signature verification**: Build artifacts are verified during CI/CD
- **Checksum generation**: SHA256 checksums are generated for all releases
- **Multi-platform builds**: Consistent builds across Windows, macOS, and Linux

## 🚨 Security Issues

If you discover a security vulnerability, please:

1. **DO NOT** create a public GitHub issue
2. Email security concerns to: [Create a private security advisory](../../security/advisories/new)
3. Include detailed information about the vulnerability
4. Allow reasonable time for patching before public disclosure

## 🔍 Verification Checklist

Before running the application:

- [ ] Downloaded from official GitHub releases page
- [ ] SHA256 checksum matches the provided `SHA256SUMS`
- [ ] Code signature is valid (platform-specific)
- [ ] File size matches expected size in release notes
- [ ] No warnings from antivirus software

## 📋 Release Artifact Information

Each release includes:

| Platform | File Types | Signing Method |
|----------|------------|----------------|
| Windows | `.exe`, `.msi` | Authenticode (if configured) |
| macOS | `.dmg` | Apple Developer ID |
| Linux | `.deb`, `.AppImage` | Checksum verification |

## 🔒 Security Features

The application implements several security measures:

- **Input validation**: All file paths and webhooks are validated
- **Sandboxed execution**: Runs within Tauri's security model  
- **No network access to untrusted domains**: Only Discord webhooks are contacted
- **Local-only processing**: Images are processed locally before upload
- **No telemetry**: No usage data is collected or transmitted

---

**Last Updated**: Auto-generated during release builds  
**Build Information**: Available in release artifacts