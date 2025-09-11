// Test utility functions and helpers

export const createMockFile = (name: string, type: string = 'image/png', size: number = 1024): File => {
  const content = new Array(size).fill(0).map(() => String.fromCharCode(Math.floor(Math.random() * 256))).join('');
  return new File([content], name, { type });
};

export const createMockFileList = (files: File[]): FileList => {
  const dt = new DataTransfer();
  files.forEach(file => dt.items.add(file));
  return dt.files;
};

export const createMockQueueItem = (overrides: Partial<any> = {}): any => {
  return {
    id: `item_${Date.now()}_${Math.random()}`,
    filePath: '/test/image.png',
    filename: 'image.png',
    status: 'queued',
    progress: 0,
    error: null,
    fileSize: 1024,
    dimensions: { width: 800, height: 600 },
    retryCount: 0,
    selected: true,
    ...overrides
  };
};

export const createMockWebhook = (overrides: Partial<any> = {}): any => {
  return {
    id: Date.now(),
    name: 'Test Webhook',
    url: 'https://discord.com/api/webhooks/123456789012345678/abcdefghijklmnopqrstuvwxyz123456',
    is_forum: false,
    ...overrides
  };
};

export const createMockUploadProgress = (overrides: Partial<any> = {}): any => {
  return {
    total_images: 3,
    completed: 1,
    current_image: 'test.png',
    current_progress: 50,
    failed_uploads: [],
    successful_uploads: ['uploaded.png'],
    session_status: 'uploading',
    estimated_time_remaining: 120,
    ...overrides
  };
};

export const waitForElement = (selector: string, timeout: number = 5000): Promise<Element> => {
  return new Promise((resolve, reject) => {
    const element = document.querySelector(selector);
    if (element) {
      resolve(element);
      return;
    }

    const observer = new MutationObserver(() => {
      const element = document.querySelector(selector);
      if (element) {
        observer.disconnect();
        resolve(element);
      }
    });

    observer.observe(document.body, {
      childList: true,
      subtree: true
    });

    setTimeout(() => {
      observer.disconnect();
      reject(new Error(`Element ${selector} not found within ${timeout}ms`));
    }, timeout);
  });
};

export const simulateEvent = (element: Element, eventType: string, eventInit: any = {}): void => {
  const event = new Event(eventType, { bubbles: true, cancelable: true, ...eventInit });
  element.dispatchEvent(event);
};

export const simulateDragEvent = (element: Element, eventType: string, files: File[] = []): void => {
  const dt = new DataTransfer();
  files.forEach(file => dt.items.add(file));
  
  const event = new DragEvent(eventType, {
    bubbles: true,
    cancelable: true,
    dataTransfer: dt
  });
  
  element.dispatchEvent(event);
};

export const simulateFileInput = (input: HTMLInputElement, files: File[]): void => {
  const dt = new DataTransfer();
  files.forEach(file => dt.items.add(file));
  
  Object.defineProperty(input, 'files', {
    value: dt.files,
    configurable: true
  });
  
  simulateEvent(input, 'change');
};

export const createQueueItemElement = (item: any): HTMLElement => {
  const element = document.createElement('div');
  element.className = 'queue-item';
  element.id = `queue-item-${item.id}`;
  element.innerHTML = `
    <div class="queue-item-header">
      <input type="checkbox" ${item.selected ? 'checked' : ''} class="queue-checkbox" data-id="${item.id}" />
      <span class="filename">${item.filename}</span>
      <span class="status ${item.status}">${item.status}</span>
    </div>
    <div class="queue-item-details">
      <span class="file-size">${formatFileSize(item.fileSize)}</span>
      <span class="dimensions">${item.dimensions?.width || 0}x${item.dimensions?.height || 0}</span>
    </div>
    <div class="progress-bar" style="width: ${item.progress}%"></div>
  `;
  return element;
};

export const formatFileSize = (bytes: number): string => {
  const sizes = ['Bytes', 'KB', 'MB', 'GB'];
  if (bytes === 0) return '0 Bytes';
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return Math.round(bytes / Math.pow(1024, i) * 100) / 100 + ' ' + sizes[i];
};

export const createToastElement = (message: string, type: 'success' | 'error' | 'info' = 'info'): HTMLElement => {
  const toast = document.createElement('div');
  toast.className = `toast toast-${type}`;
  toast.innerHTML = `
    <div class="toast-content">
      <span class="toast-icon">${type === 'success' ? '✅' : type === 'error' ? '❌' : 'ℹ️'}</span>
      <span class="toast-message">${message}</span>
    </div>
    <button class="toast-close">&times;</button>
  `;
  return toast;
};

export const mockLocalStorage = () => {
  const store: Record<string, string> = {};
  
  return {
    getItem: (key: string) => store[key] || null,
    setItem: (key: string, value: string) => { store[key] = value; },
    removeItem: (key: string) => { delete store[key]; },
    clear: () => { Object.keys(store).forEach(key => delete store[key]); },
    key: (index: number) => Object.keys(store)[index] || null,
    get length() { return Object.keys(store).length; }
  };
};

export const mockNotification = () => {
  return class MockNotification {
    static permission: NotificationPermission = 'granted';
    static requestPermission = () => Promise.resolve('granted' as NotificationPermission);
    
    title: string;
    options?: NotificationOptions;
    onclick: ((this: Notification, ev: Event) => any) | null = null;
    onerror: ((this: Notification, ev: Event) => any) | null = null;
    onshow: ((this: Notification, ev: Event) => any) | null = null;
    
    constructor(title: string, options?: NotificationOptions) {
      this.title = title;
      this.options = options;
    }
    
    close() {}
  };
};

// DOM testing utilities
export const cleanupDOM = (): void => {
  document.body.innerHTML = '';
  document.head.innerHTML = '';
};

export const setupBasicDOM = (): void => {
  document.body.innerHTML = `
    <div class="container">
      <select id="webhookSelect" class="form-control">
        <option value="">Select a webhook...</option>
      </select>
      <button id="manageWebhooksBtn" class="btn btn-secondary">🔧 Manage</button>
      <input type="checkbox" id="groupByMetadata" class="checkbox" checked />
      <input type="checkbox" id="isForumChannel" class="checkbox" />
      <input type="checkbox" id="includePlayerNames" class="checkbox" checked />
      <select id="maxImages" class="form-control">
        <option value="10" selected>10</option>
      </select>
      <div id="dropZone" class="drop-zone">
        <input type="file" id="fileInput" multiple accept="image/*" class="hidden" />
      </div>
      <div id="uploadQueue" class="upload-queue hidden">
        <div id="queueItems" class="queue-items"></div>
      </div>
      <button id="startUpload" class="btn btn-primary">🚀 Start Upload</button>
      <button id="clearQueue" class="btn btn-secondary">🗑️ Clear Queue</button>
      <div id="toastContainer" class="toast-container"></div>
    </div>
  `;
};

// Performance testing utilities
export const measureRenderTime = async (renderFn: () => void | Promise<void>): Promise<number> => {
  const start = performance.now();
  await renderFn();
  const end = performance.now();
  return end - start;
};

export const waitForAsyncUpdate = (timeout: number = 100): Promise<void> => {
  return new Promise(resolve => setTimeout(resolve, timeout));
};

// Accessibility testing helpers
export const checkAriaLabels = (element: Element): boolean => {
  const interactiveElements = element.querySelectorAll('button, input, select, textarea, a[href]');
  
  for (const el of interactiveElements) {
    const hasLabel = el.hasAttribute('aria-label') || 
                     el.hasAttribute('aria-labelledby') || 
                     (el as HTMLElement).textContent?.trim() !== '';
    
    if (!hasLabel) {
      console.warn('Missing aria-label:', el);
      return false;
    }
  }
  
  return true;
};

export const checkKeyboardNavigation = (container: Element): boolean => {
  const focusableElements = container.querySelectorAll(
    'button, input, select, textarea, a[href], [tabindex]:not([tabindex="-1"])'
  );
  
  return focusableElements.length > 0;
};