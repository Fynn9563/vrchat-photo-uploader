import { vi } from 'vitest';

// Mock Tauri API
const mockTauri = {
  invoke: vi.fn(),
  listen: vi.fn(),
  emit: vi.fn(),
  open: vi.fn(),
  readBinaryFile: vi.fn(),
  getVersion: vi.fn(() => Promise.resolve('2.0.10')),
};

// Mock @tauri-apps/api modules
vi.mock('@tauri-apps/api/tauri', () => ({
  invoke: mockTauri.invoke,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: mockTauri.listen,
  emit: mockTauri.emit,
}));

vi.mock('@tauri-apps/api/dialog', () => ({
  open: mockTauri.open,
}));

vi.mock('@tauri-apps/api/fs', () => ({
  readBinaryFile: mockTauri.readBinaryFile,
}));

vi.mock('@tauri-apps/api/app', () => ({
  getVersion: mockTauri.getVersion,
}));

// Mock browser APIs
Object.defineProperty(window, 'Notification', {
  writable: true,
  value: class MockNotification {
    static permission: NotificationPermission = 'default';
    static requestPermission = vi.fn(() => Promise.resolve('granted' as NotificationPermission));
    
    constructor(public title: string, public options?: NotificationOptions) {}
    
    close = vi.fn();
    onclick = null;
    onerror = null;
    onshow = null;
  },
});

// Mock localStorage
Object.defineProperty(window, 'localStorage', {
  value: {
    getItem: vi.fn(),
    setItem: vi.fn(),
    removeItem: vi.fn(),
    clear: vi.fn(),
    key: vi.fn(),
    length: 0,
  },
  writable: true,
});

// Mock File API
globalThis.File = class MockFile {
  constructor(
    public bits: BlobPart[],
    public name: string,
    public options?: FilePropertyBag
  ) {}
  
  size = 1024;
  type = 'image/png';
  lastModified = Date.now();
  
  arrayBuffer = vi.fn(() => Promise.resolve(new ArrayBuffer(1024)));
  slice = vi.fn();
  stream = vi.fn();
  text = vi.fn(() => Promise.resolve(''));
} as any;

// Mock FileReader
globalThis.FileReader = class MockFileReader extends EventTarget {
  result: string | ArrayBuffer | null = null;
  error: DOMException | null = null;
  readyState = 0;
  
  readAsDataURL = vi.fn((file: File) => {
    setTimeout(() => {
      this.result = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==';
      this.readyState = 2;
      this.dispatchEvent(new Event('load'));
    }, 0);
  });
  
  readAsArrayBuffer = vi.fn();
  readAsBinaryString = vi.fn();
  readAsText = vi.fn();
  abort = vi.fn();
  
  EMPTY = 0;
  LOADING = 1;
  DONE = 2;
} as any;

// Mock drag and drop events
globalThis.DataTransfer = class MockDataTransfer {
  dropEffect: 'none' | 'copy' | 'link' | 'move' = 'none';
  effectAllowed: 'none' | 'copy' | 'copyLink' | 'copyMove' | 'link' | 'linkMove' | 'move' | 'all' | 'uninitialized' = 'uninitialized';
  files: FileList = [] as any;
  items: DataTransferItemList = [] as any;
  types: readonly string[] = [];
  
  clearData = vi.fn();
  getData = vi.fn();
  setData = vi.fn();
  setDragImage = vi.fn();
} as any;

// Export mocked functions for use in tests
export { mockTauri };