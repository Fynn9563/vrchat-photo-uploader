import { describe, it, expect, beforeEach, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { mockTauri } from './setup';

describe('Tauri Command Invocations', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('Webhook Commands', () => {
    it('should invoke get_webhooks with no params', async () => {
      mockTauri.invoke.mockResolvedValueOnce([
        { id: 1, name: 'Test', url: 'https://discord.com/api/webhooks/1/abc', is_forum: false }
      ]);

      const result = await invoke('get_webhooks') as Array<{ name: string }>;
      expect(mockTauri.invoke).toHaveBeenCalledWith('get_webhooks');
      expect(result).toHaveLength(1);
      expect(result[0].name).toBe('Test');
    });

    it('should invoke add_webhook with name, url, and isForum', async () => {
      mockTauri.invoke.mockResolvedValueOnce(1);

      await invoke('add_webhook', {
        name: 'New Hook',
        url: 'https://discord.com/api/webhooks/2/def',
        isForum: false,
      });

      expect(mockTauri.invoke).toHaveBeenCalledWith('add_webhook', {
        name: 'New Hook',
        url: 'https://discord.com/api/webhooks/2/def',
        isForum: false,
      });
    });

    it('should invoke update_webhook with id, name, url, isForum', async () => {
      mockTauri.invoke.mockResolvedValueOnce(undefined);

      await invoke('update_webhook', {
        id: 1,
        name: 'Updated',
        url: 'https://discord.com/api/webhooks/3/ghi',
        isForum: true,
      });

      expect(mockTauri.invoke).toHaveBeenCalledWith('update_webhook', {
        id: 1,
        name: 'Updated',
        url: 'https://discord.com/api/webhooks/3/ghi',
        isForum: true,
      });
    });

    it('should invoke delete_webhook with id', async () => {
      mockTauri.invoke.mockResolvedValueOnce(undefined);

      await invoke('delete_webhook', { id: 1 });

      expect(mockTauri.invoke).toHaveBeenCalledWith('delete_webhook', { id: 1 });
    });
  });

  describe('Upload Commands', () => {
    it('should invoke upload_images with all parameters', async () => {
      mockTauri.invoke.mockResolvedValueOnce('session_123');

      const result = await invoke('upload_images', {
        webhookId: 1,
        filePaths: ['/photos/img1.png', '/photos/img2.png'],
        groupByMetadata: true,
        maxImagesPerMessage: 10,
        includePlayerNames: true,
        groupingTimeWindow: 60,
        groupByWorld: true,
        uploadQuality: 85,
        compressionFormat: 'webp',
        singleThreadMode: false,
        mergeNoMetadata: false,
      });

      expect(result).toBe('session_123');
      expect(mockTauri.invoke).toHaveBeenCalledWith('upload_images', expect.objectContaining({
        webhookId: 1,
        filePaths: expect.arrayContaining(['/photos/img1.png']),
      }));
    });

    it('should invoke get_upload_progress with sessionId', async () => {
      const mockProgress = {
        total_images: 5,
        completed: 2,
        current_image: 'img3.png',
        current_progress: 40,
        session_status: 'active',
      };
      mockTauri.invoke.mockResolvedValueOnce(mockProgress);

      const result = await invoke('get_upload_progress', { sessionId: 'session_123' }) as typeof mockProgress;

      expect(mockTauri.invoke).toHaveBeenCalledWith('get_upload_progress', { sessionId: 'session_123' });
      expect(result.total_images).toBe(5);
      expect(result.session_status).toBe('active');
    });

    it('should invoke cancel_upload_session with sessionId', async () => {
      mockTauri.invoke.mockResolvedValueOnce(undefined);

      await invoke('cancel_upload_session', { sessionId: 'session_123' });

      expect(mockTauri.invoke).toHaveBeenCalledWith('cancel_upload_session', { sessionId: 'session_123' });
    });

    it('should invoke retry_failed_upload with parameters', async () => {
      mockTauri.invoke.mockResolvedValueOnce('new_session');

      await invoke('retry_failed_upload', {
        sessionId: 'old_session',
        filePaths: ['/failed/img.png'],
        webhookId: 1,
      });

      expect(mockTauri.invoke).toHaveBeenCalledWith('retry_failed_upload', expect.objectContaining({
        sessionId: 'old_session',
      }));
    });
  });

  describe('Config Commands', () => {
    it('should invoke get_app_config and return config object', async () => {
      const mockConfig = {
        group_by_metadata: true,
        max_images_per_message: 10,
        upload_quality: 85,
        compression_format: 'webp',
        enable_auto_upload: false,
      };
      mockTauri.invoke.mockResolvedValueOnce(mockConfig);

      const result = await invoke('get_app_config') as typeof mockConfig;

      expect(mockTauri.invoke).toHaveBeenCalledWith('get_app_config');
      expect(result.upload_quality).toBe(85);
    });

    it('should invoke save_app_config with config object', async () => {
      mockTauri.invoke.mockResolvedValueOnce(undefined);

      const config = {
        group_by_metadata: false,
        max_images_per_message: 5,
        upload_quality: 90,
        compression_format: 'avif',
      };

      await invoke('save_app_config', { config });

      expect(mockTauri.invoke).toHaveBeenCalledWith('save_app_config', { config });
    });
  });

  describe('Image Commands', () => {
    it('should invoke get_image_metadata with filePath', async () => {
      const mockMetadata = {
        application: 'VRCX',
        world: { name: 'Test World', id: 'wrld_123' },
        players: [{ display_name: 'Alice', id: 'usr_alice' }],
      };
      mockTauri.invoke.mockResolvedValueOnce(mockMetadata);

      const result = await invoke('get_image_metadata', { filePath: '/photos/test.png' }) as typeof mockMetadata;

      expect(mockTauri.invoke).toHaveBeenCalledWith('get_image_metadata', { filePath: '/photos/test.png' });
      expect(result.world.name).toBe('Test World');
    });

    it('should invoke get_image_info with filePath', async () => {
      mockTauri.invoke.mockResolvedValueOnce([1920, 1080, 2048000]);

      const result = await invoke('get_image_info', { filePath: '/photos/test.png' });

      expect(result).toEqual([1920, 1080, 2048000]);
    });

    it('should invoke compress_image with parameters', async () => {
      mockTauri.invoke.mockResolvedValueOnce('/compressed/test.webp');

      await invoke('compress_image', {
        filePath: '/photos/test.png',
        quality: 85,
        format: 'webp',
      });

      expect(mockTauri.invoke).toHaveBeenCalledWith('compress_image', expect.objectContaining({
        filePath: '/photos/test.png',
      }));
    });
  });

  describe('Update Commands', () => {
    it('should invoke check_for_updates', async () => {
      mockTauri.invoke.mockResolvedValueOnce(undefined);

      await invoke('check_for_updates');

      expect(mockTauri.invoke).toHaveBeenCalledWith('check_for_updates');
    });
  });

  describe('User Override Commands', () => {
    it('should invoke get_user_webhook_overrides', async () => {
      mockTauri.invoke.mockResolvedValueOnce([
        { id: 1, user_id: 'usr_123', user_display_name: 'Alice', webhook_id: 1 },
      ]);

      const result = await invoke('get_user_webhook_overrides');

      expect(mockTauri.invoke).toHaveBeenCalledWith('get_user_webhook_overrides');
      expect(result).toHaveLength(1);
    });

    it('should invoke add_user_webhook_override', async () => {
      mockTauri.invoke.mockResolvedValueOnce(1);

      await invoke('add_user_webhook_override', {
        userId: 'usr_123',
        userDisplayName: 'Alice',
        webhookId: 2,
      });

      expect(mockTauri.invoke).toHaveBeenCalledWith('add_user_webhook_override', expect.objectContaining({
        webhookId: 2,
      }));
    });

    it('should invoke delete_user_webhook_override with id', async () => {
      mockTauri.invoke.mockResolvedValueOnce(undefined);

      await invoke('delete_user_webhook_override', { id: 1 });

      expect(mockTauri.invoke).toHaveBeenCalledWith('delete_user_webhook_override', { id: 1 });
    });
  });

  describe('Discord User Mapping Commands', () => {
    it('should invoke get_discord_user_mappings', async () => {
      mockTauri.invoke.mockResolvedValueOnce([
        { id: 1, vrchat_display_name: 'Alice', vrchat_user_id: 'usr_alice', discord_user_id: '123456789' },
      ]);

      const result = await invoke('get_discord_user_mappings') as Array<{ discord_user_id: string }>;

      expect(mockTauri.invoke).toHaveBeenCalledWith('get_discord_user_mappings');
      expect(result).toHaveLength(1);
      expect(result[0].discord_user_id).toBe('123456789');
    });

    it('should invoke add_discord_user_mapping with display name', async () => {
      mockTauri.invoke.mockResolvedValueOnce(1);

      await invoke('add_discord_user_mapping', {
        vrchatDisplayName: 'Alice',
        vrchatUserId: null,
        discordUserId: '123456789',
      });

      expect(mockTauri.invoke).toHaveBeenCalledWith('add_discord_user_mapping', expect.objectContaining({
        discordUserId: '123456789',
      }));
    });

    it('should invoke add_discord_user_mapping with user ID', async () => {
      mockTauri.invoke.mockResolvedValueOnce(2);

      await invoke('add_discord_user_mapping', {
        vrchatDisplayName: null,
        vrchatUserId: 'usr_alice',
        discordUserId: '987654321',
      });

      expect(mockTauri.invoke).toHaveBeenCalledWith('add_discord_user_mapping', expect.objectContaining({
        vrchatUserId: 'usr_alice',
      }));
    });

    it('should invoke update_discord_user_mapping', async () => {
      mockTauri.invoke.mockResolvedValueOnce(undefined);

      await invoke('update_discord_user_mapping', {
        id: 1,
        vrchatDisplayName: 'Alice Updated',
        vrchatUserId: null,
        discordUserId: '111111111',
      });

      expect(mockTauri.invoke).toHaveBeenCalledWith('update_discord_user_mapping', expect.objectContaining({
        id: 1,
        discordUserId: '111111111',
      }));
    });

    it('should invoke delete_discord_user_mapping with id', async () => {
      mockTauri.invoke.mockResolvedValueOnce(undefined);

      await invoke('delete_discord_user_mapping', { id: 1 });

      expect(mockTauri.invoke).toHaveBeenCalledWith('delete_discord_user_mapping', { id: 1 });
    });
  });

  describe('Error Handling', () => {
    it('should handle invoke rejection', async () => {
      mockTauri.invoke.mockRejectedValueOnce('Database error: connection failed');

      await expect(invoke('get_webhooks')).rejects.toBe('Database error: connection failed');
    });

    it('should handle invoke returning null', async () => {
      mockTauri.invoke.mockResolvedValueOnce(null);

      const result = await invoke('get_upload_progress', { sessionId: 'nonexistent' });

      expect(result).toBeNull();
    });
  });
});
