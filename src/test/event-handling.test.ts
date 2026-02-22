import { describe, it, expect, beforeEach, vi } from 'vitest';
import { listen, emit } from '@tauri-apps/api/event';
import { mockTauri } from './setup';

describe('Event Handling', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('Event Listener Registration', () => {
    it('should register upload-progress listener', async () => {
      const handler = vi.fn();
      mockTauri.listen.mockResolvedValueOnce(() => {});

      await listen('upload-progress', handler);

      expect(mockTauri.listen).toHaveBeenCalledWith('upload-progress', handler);
    });

    it('should register upload-cancelled listener', async () => {
      const handler = vi.fn();
      mockTauri.listen.mockResolvedValueOnce(() => {});

      await listen('upload-cancelled', handler);

      expect(mockTauri.listen).toHaveBeenCalledWith('upload-cancelled', handler);
    });

    it('should register update-available listener', async () => {
      const handler = vi.fn();
      mockTauri.listen.mockResolvedValueOnce(() => {});

      await listen('update-available', handler);

      expect(mockTauri.listen).toHaveBeenCalledWith('update-available', handler);
    });

    it('should register upload-item-progress listener', async () => {
      const handler = vi.fn();
      mockTauri.listen.mockResolvedValueOnce(() => {});

      await listen('upload-item-progress', handler);

      expect(mockTauri.listen).toHaveBeenCalledWith('upload-item-progress', handler);
    });

    it('should register global-shortcut-upload listener', async () => {
      const handler = vi.fn();
      mockTauri.listen.mockResolvedValueOnce(() => {});

      await listen('global-shortcut-upload', handler);

      expect(mockTauri.listen).toHaveBeenCalledWith('global-shortcut-upload', handler);
    });
  });

  describe('Event Emission', () => {
    it('should emit events with payload', async () => {
      mockTauri.emit.mockResolvedValueOnce(undefined);

      await emit('upload-files-request', { paths: ['/test/img.png'] });

      expect(mockTauri.emit).toHaveBeenCalledWith('upload-files-request', { paths: ['/test/img.png'] });
    });

    it('should emit events without payload', async () => {
      mockTauri.emit.mockResolvedValueOnce(undefined);

      await emit('show-settings');

      expect(mockTauri.emit).toHaveBeenCalledWith('show-settings');
    });
  });

  describe('Upload Progress Event Handling', () => {
    it('should handle progress update payload', () => {
      const payload = {
        session_id: 'session_123',
        total_images: 10,
        completed: 3,
        current_image: 'photo_005.png',
        current_progress: 30,
        session_status: 'active',
        failed_uploads: [],
        successful_uploads: ['photo_001.png', 'photo_002.png', 'photo_003.png'],
      };

      expect(payload.session_id).toBe('session_123');
      expect(payload.completed).toBe(3);
      expect(payload.current_progress).toBe(30);
      expect(payload.failed_uploads).toHaveLength(0);
      expect(payload.successful_uploads).toHaveLength(3);
    });

    it('should handle completed session payload', () => {
      const payload = {
        session_id: 'session_123',
        total_images: 5,
        completed: 5,
        current_progress: 100,
        session_status: 'completed',
        failed_uploads: [],
        successful_uploads: ['a.png', 'b.png', 'c.png', 'd.png', 'e.png'],
      };

      expect(payload.session_status).toBe('completed');
      expect(payload.completed).toBe(payload.total_images);
      expect(payload.current_progress).toBe(100);
    });

    it('should handle failed upload in payload', () => {
      const payload = {
        session_id: 'session_456',
        total_images: 3,
        completed: 3,
        session_status: 'completed',
        failed_uploads: [{ file: 'bad.png', error: 'File too large' }],
        successful_uploads: ['ok1.png', 'ok2.png'],
      };

      expect(payload.failed_uploads).toHaveLength(1);
      expect(payload.failed_uploads[0].error).toBe('File too large');
    });
  });

  describe('Drag and Drop Event Handling', () => {
    it('should handle tauri drag-drop payload structure', () => {
      // Tauri v2 drag-drop payload shape
      const payload = {
        paths: ['/photos/img1.png', '/photos/img2.jpg'],
        position: { x: 100, y: 200 },
      };

      expect(payload.paths).toHaveLength(2);
      expect(payload.paths[0]).toBe('/photos/img1.png');
      expect(payload.position.x).toBe(100);
    });

    it('should filter image files from drag-drop paths', () => {
      const paths = [
        '/photos/img1.png',
        '/photos/img2.jpg',
        '/photos/readme.txt',
        '/photos/img3.webp',
        '/photos/video.mp4',
      ];

      const imageExtensions = ['.png', '.jpg', '.jpeg', '.webp', '.avif', '.gif'];
      const imagePaths = paths.filter(p => {
        const ext = p.substring(p.lastIndexOf('.')).toLowerCase();
        return imageExtensions.includes(ext);
      });

      expect(imagePaths).toHaveLength(3);
      expect(imagePaths).toContain('/photos/img1.png');
      expect(imagePaths).toContain('/photos/img2.jpg');
      expect(imagePaths).toContain('/photos/img3.webp');
      expect(imagePaths).not.toContain('/photos/readme.txt');
      expect(imagePaths).not.toContain('/photos/video.mp4');
    });
  });

  describe('Listener Cleanup', () => {
    it('should return unlisten function', async () => {
      const unlisten = vi.fn();
      mockTauri.listen.mockResolvedValueOnce(unlisten);

      const cleanup = await listen('test-event', vi.fn());

      expect(cleanup).toBe(unlisten);
    });

    it('should support calling unlisten to remove listener', async () => {
      const unlisten = vi.fn();
      mockTauri.listen.mockResolvedValueOnce(unlisten);

      const cleanup = await listen('test-event', vi.fn());
      cleanup();

      expect(unlisten).toHaveBeenCalled();
    });
  });
});
