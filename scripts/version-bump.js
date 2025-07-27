#!/usr/bin/env node

/**
 * Version Bump Script for VRChat Photo Uploader
 * 
 * This script synchronizes version numbers across all project files:
 * - package.json
 * - src-tauri/Cargo.toml
 * - src-tauri/tauri.conf.json
 * 
 * Usage:
 *   node scripts/version-bump.js patch   # 1.0.0 -> 1.0.1
 *   node scripts/version-bump.js minor   # 1.0.0 -> 1.1.0
 *   node scripts/version-bump.js major   # 1.0.0 -> 2.0.0
 *   node scripts/version-bump.js 1.2.3   # Set specific version
 */

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

// ANSI color codes for colored output
const colors = {
  reset: '\x1b[0m',
  bright: '\x1b[1m',
  red: '\x1b[31m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  cyan: '\x1b[36m'
};

function log(message, color = colors.reset) {
  console.log(`${color}${message}${colors.reset}`);
}

function error(message) {
  log(`❌ Error: ${message}`, colors.red);
  process.exit(1);
}

function success(message) {
  log(`✅ ${message}`, colors.green);
}

function info(message) {
  log(`ℹ️  ${message}`, colors.blue);
}

function warning(message) {
  log(`⚠️  ${message}`, colors.yellow);
}

function parseVersion(version) {
  const match = version.match(/^(\d+)\.(\d+)\.(\d+)$/);
  if (!match) {
    error(`Invalid version format: ${version}. Expected format: X.Y.Z`);
  }
  return {
    major: parseInt(match[1]),
    minor: parseInt(match[2]),
    patch: parseInt(match[3])
  };
}

function incrementVersion(currentVersion, increment) {
  const version = parseVersion(currentVersion);
  
  switch (increment) {
    case 'major':
      return `${version.major + 1}.0.0`;
    case 'minor':
      return `${version.major}.${version.minor + 1}.0`;
    case 'patch':
      return `${version.major}.${version.minor}.${version.patch + 1}`;
    default:
      // Check if increment is a specific version
      parseVersion(increment); // Validate format
      return increment;
  }
}

function updatePackageJson(newVersion) {
  const packagePath = path.join(process.cwd(), 'package.json');
  
  if (!fs.existsSync(packagePath)) {
    error('package.json not found in current directory');
  }
  
  const packageJson = JSON.parse(fs.readFileSync(packagePath, 'utf8'));
  const oldVersion = packageJson.version;
  packageJson.version = newVersion;
  
  fs.writeFileSync(packagePath, JSON.stringify(packageJson, null, 2) + '\n');
  info(`Updated package.json: ${oldVersion} → ${newVersion}`);
  
  // Update pnpm lock file if it exists
  if (fs.existsSync('pnpm-lock.yaml')) {
    try {
      info('Updating pnpm-lock.yaml...');
      execSync('pnpm install --lockfile-only', { stdio: 'pipe' });
      success('Updated pnpm-lock.yaml');
    } catch (err) {
      warning('Failed to update pnpm-lock.yaml automatically. You may need to run "pnpm install" manually.');
    }
  }
  
  return oldVersion;
}

function updateCargoToml(newVersion) {
  const cargoPath = path.join(process.cwd(), 'src-tauri', 'Cargo.toml');
  
  if (!fs.existsSync(cargoPath)) {
    error('src-tauri/Cargo.toml not found');
  }
  
  let cargoContent = fs.readFileSync(cargoPath, 'utf8');
  const versionRegex = /^version\s*=\s*"([^"]+)"/m;
  const match = cargoContent.match(versionRegex);
  
  if (!match) {
    error('Could not find version field in Cargo.toml');
  }
  
  const oldVersion = match[1];
  cargoContent = cargoContent.replace(versionRegex, `version = "${newVersion}"`);
  
  fs.writeFileSync(cargoPath, cargoContent);
  info(`Updated Cargo.toml: ${oldVersion} → ${newVersion}`);
  
  return oldVersion;
}

function updateTauriConfig(newVersion) {
  const configPath = path.join(process.cwd(), 'src-tauri', 'tauri.conf.json');
  
  if (!fs.existsSync(configPath)) {
    error('src-tauri/tauri.conf.json not found');
  }
  
  const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
  const oldVersion = config.package?.version || 'unknown';
  
  if (!config.package) {
    config.package = {};
  }
  config.package.version = newVersion;
  
  fs.writeFileSync(configPath, JSON.stringify(config, null, 2) + '\n');
  info(`Updated tauri.conf.json: ${oldVersion} → ${newVersion}`);
  
  return oldVersion;
}

function updateCargoLock() {
  try {
    info('Updating Cargo.lock...');
    execSync('cd src-tauri && cargo check', { stdio: 'pipe' });
    success('Updated Cargo.lock');
  } catch (err) {
    warning('Failed to update Cargo.lock automatically. You may need to run "cargo check" manually.');
  }
}

function validateEnvironment() {
  // Check if we're in the right directory
  if (!fs.existsSync('package.json')) {
    error('This script must be run from the project root directory');
  }
  
  if (!fs.existsSync('src-tauri')) {
    error('src-tauri directory not found. Are you in the right project?');
  }
  
  // Check if git is available and repo is clean (optional warning)
  try {
    const status = execSync('git status --porcelain', { encoding: 'utf8' });
    if (status.trim()) {
      warning('You have uncommitted changes. Consider committing them before bumping version.');
    }
  } catch (err) {
    warning('Git not available or not in a git repository');
  }
}

function showUsage() {
  log('\n📋 Version Bump Script Usage:', colors.bright);
  log('\nIncrement version:');
  log('  node scripts/version-bump.js patch   # 1.0.0 → 1.0.1');
  log('  node scripts/version-bump.js minor   # 1.0.0 → 1.1.0');
  log('  node scripts/version-bump.js major   # 1.0.0 → 2.0.0');
  log('\nSet specific version:');
  log('  node scripts/version-bump.js 1.2.3   # Set to 1.2.3');
  log('\nOptions:');
  log('  --dry-run    Show what would be changed without making changes');
  log('  --help       Show this help message');
  log('');
}

function main() {
  const args = process.argv.slice(2);
  
  if (args.length === 0 || args.includes('--help')) {
    showUsage();
    process.exit(0);
  }
  
  const isDryRun = args.includes('--dry-run');
  const increment = args.find(arg => !arg.startsWith('--'));
  
  if (!increment) {
    error('Please specify version increment (patch/minor/major) or specific version');
  }
  
  log('🚀 VRChat Photo Uploader - Version Bump Script\n', colors.bright);
  
  if (isDryRun) {
    log('🔍 DRY RUN MODE - No files will be modified\n', colors.yellow);
  }
  
  validateEnvironment();
  
  // Get current version from package.json
  const packageJson = JSON.parse(fs.readFileSync('package.json', 'utf8'));
  const currentVersion = packageJson.version;
  const newVersion = incrementVersion(currentVersion, increment);
  
  log(`📦 Current version: ${currentVersion}`, colors.cyan);
  log(`📦 New version: ${newVersion}`, colors.green);
  
  if (currentVersion === newVersion) {
    warning('Version unchanged. Nothing to do.');
    process.exit(0);
  }
  
  if (isDryRun) {
    log('\n📋 Changes that would be made:');
    log(`  - package.json: ${currentVersion} → ${newVersion}`);
    log(`  - src-tauri/Cargo.toml: version → ${newVersion}`);
    log(`  - src-tauri/tauri.conf.json: package.version → ${newVersion}`);
    log(`  - src-tauri/Cargo.lock: would be regenerated`);
    if (fs.existsSync('pnpm-lock.yaml')) {
      log(`  - pnpm-lock.yaml: would be updated`);
    }
    log('\nRun without --dry-run to apply changes.\n');
    process.exit(0);
  }
  
  log('\n🔄 Updating version files...\n');
  
  try {
    updatePackageJson(newVersion);
    updateCargoToml(newVersion);
    updateTauriConfig(newVersion);
    updateCargoLock();
    
    log('\n🎉 Version bump completed successfully!', colors.bright);
    log(`\n📋 Next steps:`, colors.blue);
    log(`  1. Review the changes: git diff`);
    
    // Detect package manager for build command
    let buildCmd = 'npm run tauri build';
    if (fs.existsSync('pnpm-lock.yaml')) {
      buildCmd = 'pnpm run tauri build';
    } else if (fs.existsSync('yarn.lock')) {
      buildCmd = 'yarn tauri build';
    }
    
    log(`  2. Test the build: ${buildCmd}`);
    log(`  3. Commit the changes: git add . && git commit -m "chore: bump version to ${newVersion}"`);
    log(`  4. Push to trigger release: git push`);
    log('');
    
  } catch (err) {
    error(`Failed to update version: ${err.message}`);
  }
}

if (require.main === module) {
  main();
}

module.exports = {
  incrementVersion,
  parseVersion,
  updatePackageJson,
  updateCargoToml,
  updateTauriConfig
};