# Testing Documentation

This directory contains the test suite for VRChat Photo Uploader UI components and functionality.

## Test Structure

```
src/test/
├── README.md           # This file
├── setup.ts           # Test environment setup and mocks
├── helpers.ts         # Utility functions for testing
├── example.test.ts    # Example tests demonstrating setup
├── ui.test.ts         # Unit tests for UI components
└── integration.test.ts # Integration tests for user workflows

e2e/
└── app.e2e.ts         # End-to-end tests using Playwright
```

## Test Types

### Unit Tests (`*.test.ts`)
- Test individual components and functions in isolation
- Mock external dependencies (Tauri API, browser APIs)
- Fast execution and focused scope
- Located in `src/test/`

### Integration Tests (`integration.test.ts`)
- Test complete user workflows and component interactions
- Test multiple components working together
- Simulate real user scenarios
- Still use mocks for external services

### End-to-End Tests (`e2e/*.e2e.ts`)
- Test the complete application in a real browser environment
- Interact with the actual Tauri application
- Test critical user paths from start to finish
- Slower but provides highest confidence

## Running Tests

### Unit and Integration Tests
```bash
# Run all tests
pnpm test

# Run tests in watch mode
pnpm test:watch

# Run tests with UI
pnpm test:ui

# Run tests once (CI mode)
pnpm test:run

# Run with coverage
pnpm test:run --coverage
```

### End-to-End Tests
```bash
# Run E2E tests
pnpm test:e2e

# Run E2E tests with UI
pnpm test:e2e:ui

# Install Playwright browsers (first time only)
pnpm exec playwright install
```

## Test Configuration

### Vitest (vitest.config.ts)
- Environment: `happy-dom` for DOM simulation
- Setup file: `src/test/setup.ts` for mocks and globals
- Coverage reporting enabled

### Playwright (playwright.config.ts)
- Targets Tauri dev server at `http://localhost:1420`
- Configured for desktop Chrome
- Automatic server startup
- Screenshot and video recording on failure

## Writing Tests

### Unit Test Example
```typescript
import { describe, it, expect, beforeEach } from 'vitest';
import { mockTauri } from './setup';

describe('Component Tests', () => {
  beforeEach(() => {
    // Reset DOM and mocks
    document.body.innerHTML = '<div id="app"></div>';
    vi.clearAllMocks();
  });

  it('should test component behavior', () => {
    // Test implementation
    expect(true).toBe(true);
  });
});
```

### E2E Test Example
```typescript
import { test, expect } from '@playwright/test';

test('should complete user workflow', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('#app')).toBeVisible();
  
  // Interact with the application
  await page.click('#button');
  await expect(page.locator('#result')).toContainText('Expected');
});
```

## Test Utilities

### Mock Functions (`setup.ts`)
- `mockTauri`: Mock Tauri API functions
- Mock browser APIs (Notification, localStorage, File, etc.)
- Mock drag and drop events

### Helper Functions (`helpers.ts`)
- `createMockFile()`: Create mock File objects
- `createMockQueueItem()`: Create mock upload queue items
- `waitForElement()`: Wait for DOM elements
- `simulateEvent()`: Trigger DOM events
- `setupBasicDOM()`: Set up basic DOM structure

## Test Coverage

Tests cover the following areas:

### UI Components
- ✅ Webhook management modal and form validation
- ✅ Upload settings and toggles
- ✅ File upload area and drag/drop functionality
- ✅ Upload queue management and controls
- ✅ Progress tracking and status updates
- ✅ Toast notifications and error handling
- ✅ Modal management and keyboard shortcuts

### User Workflows
- ✅ Complete upload process from file selection to completion
- ✅ Webhook CRUD operations
- ✅ Queue management (select all, remove, clear)
- ✅ Error handling and retry functionality
- ✅ Settings persistence and configuration

### Critical Paths (E2E)
- ✅ Application startup and initialization
- ✅ File selection and queue population
- ✅ Upload process with progress tracking
- ✅ Error scenarios and recovery
- ✅ Keyboard shortcuts and accessibility

## Best Practices

### Writing Tests
1. **Descriptive test names**: Use clear, descriptive test names
2. **Arrange-Act-Assert**: Structure tests with clear setup, action, and verification
3. **Independent tests**: Each test should be isolated and not depend on others
4. **Mock external dependencies**: Mock Tauri API calls and browser APIs
5. **Test user behavior**: Focus on testing what users actually do

### Maintaining Tests
1. **Keep tests up to date**: Update tests when UI changes
2. **Remove obsolete tests**: Clean up tests for removed features
3. **Group related tests**: Use `describe` blocks to organize tests
4. **Use beforeEach/afterEach**: Clean up state between tests
5. **Mock realistically**: Keep mocks close to real API behavior

### Performance
1. **Fast unit tests**: Keep unit tests fast by avoiding unnecessary DOM operations
2. **Selective E2E tests**: Only test critical paths with E2E tests
3. **Parallel execution**: Configure tests to run in parallel when possible
4. **Clean up resources**: Properly clean up DOM, timers, and event listeners

## Continuous Integration

Tests run automatically on:
- Every push to main/master branches
- All pull requests
- Manual workflow dispatch

The CI pipeline includes:
1. **Frontend tests**: Unit and integration tests with coverage
2. **Backend tests**: Rust unit and integration tests
3. **Linting**: Code quality and formatting checks
4. **Security scans**: Dependency vulnerability checks
5. **E2E tests**: Full application testing (on main branch only)

## Debugging Tests

### Local Development
```bash
# Run single test file
pnpm test ui.test.ts

# Run tests matching pattern
pnpm test --grep "webhook"

# Debug mode with browser
pnpm test:ui

# E2E debugging
pnpm test:e2e --debug
```

### Common Issues
1. **DOM not ready**: Use `waitForElement()` helper
2. **Async operations**: Properly await promises and use `waitForAsyncUpdate()`
3. **Mock not applied**: Check mock setup in `beforeEach`
4. **Tauri API errors**: Verify mock responses in `setup.ts`
5. **E2E timeouts**: Increase timeout or wait for specific conditions