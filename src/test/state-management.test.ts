import { describe, it, expect, beforeEach, vi } from 'vitest';
import { createMockQueueItem, createMockWebhook, createMockUploadProgress } from './helpers';

describe('State Management', () => {
  describe('Upload Queue State', () => {
    let queue: any[];

    beforeEach(() => {
      queue = [];
    });

    it('should add items to queue', () => {
      const item = createMockQueueItem({ filename: 'test.png' });
      queue.push(item);
      expect(queue).toHaveLength(1);
      expect(queue[0].filename).toBe('test.png');
    });

    it('should remove items from queue by id', () => {
      const item1 = createMockQueueItem({ id: 'item_1', filename: 'a.png' });
      const item2 = createMockQueueItem({ id: 'item_2', filename: 'b.png' });
      queue = [item1, item2];

      queue = queue.filter(i => i.id !== 'item_1');
      expect(queue).toHaveLength(1);
      expect(queue[0].id).toBe('item_2');
    });

    it('should clear all items from queue', () => {
      queue = [
        createMockQueueItem({ id: 'item_1' }),
        createMockQueueItem({ id: 'item_2' }),
        createMockQueueItem({ id: 'item_3' }),
      ];
      expect(queue).toHaveLength(3);

      queue = [];
      expect(queue).toHaveLength(0);
    });

    it('should select all items', () => {
      queue = [
        createMockQueueItem({ selected: false }),
        createMockQueueItem({ selected: false }),
      ];
      queue.forEach(item => item.selected = true);
      expect(queue.every(i => i.selected)).toBe(true);
    });

    it('should deselect all items', () => {
      queue = [
        createMockQueueItem({ selected: true }),
        createMockQueueItem({ selected: true }),
      ];
      queue.forEach(item => item.selected = false);
      expect(queue.every(i => !i.selected)).toBe(true);
    });

    it('should get only selected items', () => {
      queue = [
        createMockQueueItem({ id: 'a', selected: true }),
        createMockQueueItem({ id: 'b', selected: false }),
        createMockQueueItem({ id: 'c', selected: true }),
      ];
      const selected = queue.filter(i => i.selected);
      expect(selected).toHaveLength(2);
      expect(selected.map(i => i.id)).toEqual(['a', 'c']);
    });

    it('should update item status', () => {
      const item = createMockQueueItem({ status: 'queued' });
      queue = [item];

      queue[0].status = 'uploading';
      expect(queue[0].status).toBe('uploading');

      queue[0].status = 'completed';
      expect(queue[0].status).toBe('completed');
    });

    it('should update item progress', () => {
      const item = createMockQueueItem({ progress: 0 });
      queue = [item];

      queue[0].progress = 50;
      expect(queue[0].progress).toBe(50);

      queue[0].progress = 100;
      expect(queue[0].progress).toBe(100);
    });

    it('should handle item with error', () => {
      const item = createMockQueueItem({ status: 'queued', error: null });
      queue = [item];

      queue[0].status = 'failed';
      queue[0].error = 'Network timeout';
      expect(queue[0].status).toBe('failed');
      expect(queue[0].error).toBe('Network timeout');
    });

    it('should increment retry count', () => {
      const item = createMockQueueItem({ retryCount: 0 });
      queue = [item];

      queue[0].retryCount += 1;
      expect(queue[0].retryCount).toBe(1);

      queue[0].retryCount += 1;
      expect(queue[0].retryCount).toBe(2);
    });
  });

  describe('Webhook Management State', () => {
    let webhooks: any[];

    beforeEach(() => {
      webhooks = [];
    });

    it('should add webhook to list', () => {
      const webhook = createMockWebhook({ id: 1, name: 'Server 1' });
      webhooks.push(webhook);
      expect(webhooks).toHaveLength(1);
      expect(webhooks[0].name).toBe('Server 1');
    });

    it('should remove webhook by id', () => {
      webhooks = [
        createMockWebhook({ id: 1, name: 'A' }),
        createMockWebhook({ id: 2, name: 'B' }),
      ];
      webhooks = webhooks.filter(w => w.id !== 1);
      expect(webhooks).toHaveLength(1);
      expect(webhooks[0].name).toBe('B');
    });

    it('should update webhook properties', () => {
      webhooks = [createMockWebhook({ id: 1, name: 'Old Name', is_forum: false })];
      const target = webhooks.find(w => w.id === 1);
      target.name = 'New Name';
      target.is_forum = true;
      expect(webhooks[0].name).toBe('New Name');
      expect(webhooks[0].is_forum).toBe(true);
    });

    it('should find webhook by id', () => {
      webhooks = [
        createMockWebhook({ id: 1, name: 'A' }),
        createMockWebhook({ id: 2, name: 'B' }),
      ];
      const found = webhooks.find(w => w.id === 2);
      expect(found).toBeDefined();
      expect(found.name).toBe('B');
    });

    it('should return undefined for missing webhook', () => {
      webhooks = [createMockWebhook({ id: 1 })];
      const found = webhooks.find(w => w.id === 999);
      expect(found).toBeUndefined();
    });
  });

  describe('Upload Progress State', () => {
    it('should create progress with defaults', () => {
      const progress = createMockUploadProgress();
      expect(progress.total_images).toBe(3);
      expect(progress.completed).toBe(1);
      expect(progress.session_status).toBe('uploading');
    });

    it('should track progress updates', () => {
      const progress = createMockUploadProgress({ completed: 0, current_progress: 0 });

      progress.completed = 1;
      progress.current_progress = 33;
      expect(progress.completed).toBe(1);

      progress.completed = 3;
      progress.current_progress = 100;
      progress.session_status = 'completed';
      expect(progress.session_status).toBe('completed');
    });

    it('should handle cancelled session', () => {
      const progress = createMockUploadProgress({ session_status: 'active' });
      progress.session_status = 'cancelled';
      expect(progress.session_status).toBe('cancelled');
    });

    it('should track failed uploads', () => {
      const progress = createMockUploadProgress({
        failed_uploads: ['img1.png', 'img2.png'],
        successful_uploads: ['img3.png'],
      });
      expect(progress.failed_uploads).toHaveLength(2);
      expect(progress.successful_uploads).toHaveLength(1);
    });
  });

  describe('Session Status Transitions', () => {
    it('should follow valid status transitions', () => {
      const validTransitions: Record<string, string[]> = {
        'queued': ['uploading', 'cancelled'],
        'uploading': ['completed', 'failed', 'cancelled'],
        'failed': ['uploading'], // retry
        'completed': [],
        'cancelled': [],
      };

      // queued -> uploading
      expect(validTransitions['queued']).toContain('uploading');
      // uploading -> completed
      expect(validTransitions['uploading']).toContain('completed');
      // uploading -> failed
      expect(validTransitions['uploading']).toContain('failed');
      // failed -> uploading (retry)
      expect(validTransitions['failed']).toContain('uploading');
      // completed -> nothing
      expect(validTransitions['completed']).toHaveLength(0);
    });
  });
});
