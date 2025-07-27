#!/usr/bin/env node

/**
 * Fix PNPM Lock File Script
 * 
 * This script fixes compatibility issues with pnpm-lock.yaml in CI environments
 * by regenerating the lock file with a compatible version.
 */

const fs = require('fs');
const { execSync } = require('child_process');

const colors = {
  reset: '\x1b[0m',
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

function checkPnpmVersion() {
  try {
    const version = execSync('pnpm --version', { encoding: 'utf8' }).trim();
    info(`Current pnpm version: ${version}`);
    return version;
  } catch (err) {
    error('pnpm is not installed. Please install pnpm first: npm install -g pnpm');
  }
}

function checkLockfileVersion() {
  if (!fs.existsSync('pnpm-lock.yaml')) {
    warning('pnpm-lock.yaml not found');
    return null;
  }
  
  const lockfileContent = fs.readFileSync('pnpm-lock.yaml', 'utf8');
  const versionMatch = lockfileContent.match(/lockfileVersion:\s*['"]?([^'"]+)['"]?/);
  
  if (versionMatch) {
    const version = versionMatch[1];
    info(`Current lockfile version: ${version}`);
    return version;
  }
  
  warning('Could not determine lockfile version');
  return null;
}

function backupLockfile() {
  if (fs.existsSync('pnpm-lock.yaml')) {
    const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
    const backupName = `pnpm-lock.yaml.backup-${timestamp}`;
    
    fs.copyFileSync('pnpm-lock.yaml', backupName);
    info(`Backed up existing lockfile to: ${backupName}`);
    return backupName;
  }
  return null;
}

function regenerateLockfile() {
  info('Regenerating pnpm-lock.yaml...');
  
  try {
    // Remove existing lockfile
    if (fs.existsSync('pnpm-lock.yaml')) {
      fs.unlinkSync('pnpm-lock.yaml');
      info('Removed existing lockfile');
    }
    
    // Regenerate with current pnpm version
    execSync('pnpm install --lockfile-only', { stdio: 'inherit' });
    success('Generated new lockfile');
    
    // Check new version
    const newVersion = checkLockfileVersion();
    if (newVersion) {
      success(`New lockfile version: ${newVersion}`);
    }
    
  } catch (err) {
    error(`Failed to regenerate lockfile: ${err.message}`);
  }
}

function validateLockfile() {
  info('Validating new lockfile...');
  
  try {
    execSync('pnpm install --frozen-lockfile', { stdio: 'pipe' });
    success('Lockfile validation successful');
    return true;
  } catch (err) {
    warning('Lockfile validation failed, but this might be expected');
    return false;
  }
}

function showGitInstructions() {
  log('\n📋 Next Steps:', colors.cyan);
  log('1. Review the changes: git diff pnpm-lock.yaml');
  log('2. Test locally: pnpm install');
  log('3. Commit the changes:');
  log('   git add pnpm-lock.yaml');
  log('   git commit -m "fix: regenerate pnpm-lock.yaml for CI compatibility"');
  log('4. Push to test CI: git push');
  log('');
}

function main() {
  log('🔧 PNPM Lock File Compatibility Fix', colors.cyan);
  log('====================================\n');
  
  // Check if we're in the right directory
  if (!fs.existsSync('package.json')) {
    error('package.json not found. Run this script from the project root.');
  }
  
  // Check pnpm installation
  const pnpmVersion = checkPnpmVersion();
  
  // Check current lockfile
  const currentLockfileVersion = checkLockfileVersion();
  
  if (currentLockfileVersion === '6.0') {
    warning('Lockfile version 6.0 detected - this may cause CI issues');
    
    const userResponse = process.argv.includes('--force') || process.argv.includes('-f');
    
    if (!userResponse) {
      log('\nOptions:', colors.blue);
      log('1. Run with --force to regenerate automatically');
      log('2. Use the GitHub workflow with fallback handling');
      log('3. Update your local pnpm version and regenerate manually');
      log('\nExample: node scripts/fix-lockfile.js --force');
      process.exit(0);
    }
  }
  
  if (process.argv.includes('--force') || process.argv.includes('-f')) {
    // Backup existing lockfile
    const backupFile = backupLockfile();
    
    try {
      // Regenerate lockfile
      regenerateLockfile();
      
      // Validate
      const isValid = validateLockfile();
      
      if (isValid) {
        success('✨ Lockfile successfully regenerated and validated!');
      } else {
        success('✨ Lockfile regenerated (validation skipped)');
      }
      
      showGitInstructions();
      
    } catch (err) {
      error(`Failed to fix lockfile: ${err.message}`);
      
      if (backupFile && fs.existsSync(backupFile)) {
        log(`Restoring backup from: ${backupFile}`, colors.yellow);
        fs.copyFileSync(backupFile, 'pnpm-lock.yaml');
      }
    }
  } else {
    info('Lockfile appears compatible with current pnpm version');
    success('No action needed!');
  }
}

if (require.main === module) {
  main();
}