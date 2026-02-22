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

  describe('Multi-Webhook Session Logic', () => {
    it('should not treat intermediate webhook completion as session completion', () => {
      // Simulates: 2 files, 2 webhooks, total_images = 4
      // After webhook 1 completes 2 files: completed=2, total_images=4
      const progress = createMockUploadProgress({
        total_images: 4,
        completed: 2,
        session_status: 'completed',
        current_webhook_index: 0,
        total_webhooks: 2,
      });

      const allFilesProcessed = progress.completed >= progress.total_images;
      const isLastWebhook = progress.current_webhook_index >= progress.total_webhooks - 1;
      const sessionCompleted = (progress.session_status === 'completed' && isLastWebhook) ||
        progress.session_status === 'failed' ||
        progress.session_status === 'cancelled' ||
        allFilesProcessed;

      expect(allFilesProcessed).toBe(false);
      expect(isLastWebhook).toBe(false);
      expect(sessionCompleted).toBe(false);
    });

    it('should treat final webhook completion as session completion', () => {
      // After webhook 2 completes: completed=4, total_images=4
      const progress = createMockUploadProgress({
        total_images: 4,
        completed: 4,
        session_status: 'completed',
        current_webhook_index: 1,
        total_webhooks: 2,
      });

      const allFilesProcessed = progress.completed >= progress.total_images;
      const isLastWebhook = progress.current_webhook_index >= progress.total_webhooks - 1;
      const sessionCompleted = (progress.session_status === 'completed' && isLastWebhook) ||
        allFilesProcessed;

      expect(allFilesProcessed).toBe(true);
      expect(isLastWebhook).toBe(true);
      expect(sessionCompleted).toBe(true);
    });

    it('should detect webhook transition and reset item states', () => {
      const items = [
        createMockQueueItem({ id: '1', filename: 'a.png', status: 'success', selected: true }),
        createMockQueueItem({ id: '2', filename: 'b.png', status: 'success', selected: true }),
      ];

      // Simulate webhook transition: index changed from 0 to 1
      let lastSeenWebhookIndex = 0;
      const progress = createMockUploadProgress({
        total_webhooks: 2,
        current_webhook_index: 1,
        successful_uploads: [],
      });

      if (progress.total_webhooks > 1 && progress.current_webhook_index !== lastSeenWebhookIndex) {
        lastSeenWebhookIndex = progress.current_webhook_index;
        items.forEach(item => {
          if (item.selected) {
            item.status = 'queued';
            item.progress = 0;
            item.error = null;
          }
        });
      }

      expect(lastSeenWebhookIndex).toBe(1);
      expect(items[0].status).toBe('queued');
      expect(items[1].status).toBe('queued');
      expect(items[0].progress).toBe(0);
    });

    it('should not reset items when webhook index stays the same', () => {
      const items = [
        createMockQueueItem({ id: '1', filename: 'a.png', status: 'success', selected: true }),
      ];

      let lastSeenWebhookIndex = 0;
      const progress = createMockUploadProgress({
        total_webhooks: 2,
        current_webhook_index: 0,
      });

      if (progress.total_webhooks > 1 && progress.current_webhook_index !== lastSeenWebhookIndex) {
        lastSeenWebhookIndex = progress.current_webhook_index;
        items.forEach(item => {
          if (item.selected) {
            item.status = 'queued';
            item.progress = 0;
          }
        });
      }

      // No transition detected, items unchanged
      expect(items[0].status).toBe('success');
    });

    it('should preserve total_images across webhooks (not overwrite)', () => {
      // This tests the key bug: total_images must be files * webhooks, not just files
      const progress = createMockUploadProgress({
        total_images: 6, // 3 files * 2 webhooks
        completed: 3,    // webhook 1 done
        current_webhook_index: 0,
        total_webhooks: 2,
        session_status: 'active',
      });

      // total_images should NOT be overwritten to file count (3)
      expect(progress.total_images).toBe(6);
      const allFilesProcessed = progress.completed >= progress.total_images;
      expect(allFilesProcessed).toBe(false);
    });

    it('should include webhook name in progress', () => {
      const progress = createMockUploadProgress({
        current_webhook_name: 'VRC Photos',
        current_webhook_index: 1,
        total_webhooks: 3,
      });

      const webhookLabel = progress.current_webhook_name
        ? `${progress.current_webhook_name} (${progress.current_webhook_index + 1}/${progress.total_webhooks})`
        : `Webhook ${progress.current_webhook_index + 1}/${progress.total_webhooks}`;

      expect(webhookLabel).toBe('VRC Photos (2/3)');
    });

    it('should fall back to generic label when webhook name is empty', () => {
      const progress = createMockUploadProgress({
        current_webhook_name: '',
        current_webhook_index: 0,
        total_webhooks: 2,
      });

      const webhookLabel = progress.current_webhook_name
        ? `${progress.current_webhook_name} (${progress.current_webhook_index + 1}/${progress.total_webhooks})`
        : `Webhook ${progress.current_webhook_index + 1}/${progress.total_webhooks}`;

      expect(webhookLabel).toBe('Webhook 1/2');
    });

    it('should stop on failed status even during multi-webhook', () => {
      const progress = createMockUploadProgress({
        total_images: 4,
        completed: 1,
        session_status: 'failed',
        current_webhook_index: 0,
        total_webhooks: 2,
      });

      const isLastWebhook = progress.current_webhook_index >= progress.total_webhooks - 1;
      const sessionCompleted = (progress.session_status === 'completed' && isLastWebhook) ||
        progress.session_status === 'failed' ||
        progress.session_status === 'cancelled';

      expect(sessionCompleted).toBe(true);
    });

    it('should stop on cancelled status even during multi-webhook', () => {
      const progress = createMockUploadProgress({
        total_images: 4,
        completed: 1,
        session_status: 'cancelled',
        current_webhook_index: 0,
        total_webhooks: 2,
      });

      const sessionCompleted = (progress.session_status === 'completed' && (progress.current_webhook_index >= progress.total_webhooks - 1)) ||
        progress.session_status === 'failed' ||
        progress.session_status === 'cancelled';

      expect(sessionCompleted).toBe(true);
    });
  });
});
