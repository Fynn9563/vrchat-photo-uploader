import './styles.css';
import { invoke } from '@tauri-apps/api/tauri';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/api/dialog';
import { readBinaryFile } from '@tauri-apps/api/fs';

console.log('VRChat Photo Uploader starting...');

// Enhanced type definitions
interface Webhook {
  id: number;
  name: string;
  url: string;
  is_forum: boolean;
}

interface QueueItem {
  id: string;
  filePath: string;
  filename: string;
  status: 'queued' | 'uploading' | 'success' | 'error';
  progress: number;
  error: string | null;
  fileSize?: number;
  dimensions?: { width: number; height: number };
  retryCount: number;
  selected: boolean;
  thumbnailData?: string;
}

interface UploadProgress {
  total_images: number;
  completed: number;
  current_image?: string;
  current_progress: number;
  failed_uploads: FailedUpload[];
  successful_uploads: string[];
  session_status: string;
  estimated_time_remaining?: number;
}

interface FailedUpload {
  file_path: string;
  error: string;
  retry_count: number;
}

interface AppConfig {
  last_webhook_id?: number;
  group_by_metadata: boolean;
  max_images_per_message: number;
  enable_global_shortcuts: boolean;
  auto_compress_threshold: number;
  upload_quality: number;
}

// Enhanced state management
class AppState {
  public webhooks: Webhook[] = [];
  private uploadQueue: QueueItem[] = [];
  private currentUploadSession: string | null = null;
  public selectedWebhookId: number | null = null;
  private progressPollingInterval: number | null = null;
  private isUploading: boolean = false;
  private notificationsEnabled: boolean = true;

  // Notification methods
  private async requestNotificationPermission(): Promise<boolean> {
    if (!('Notification' in window)) {
      console.warn('This browser does not support notifications');
      return false;
    }

    if (Notification.permission === 'granted') {
      return true;
    }

    if (Notification.permission === 'denied') {
      return false;
    }

    const permission = await Notification.requestPermission();
    return permission === 'granted';
  }

  private async showDesktopNotification(title: string, message: string, type: 'success' | 'error' | 'info' = 'info') {
    if (!this.notificationsEnabled) return;

    const hasPermission = await this.requestNotificationPermission();
    if (!hasPermission) return;

    const options: NotificationOptions = {
      body: message,
      icon: type === 'success' ? '✅' : type === 'error' ? '❌' : 'ℹ️',
      badge: '📸',
      tag: 'vrchat-uploader',
      requireInteraction: type === 'error',
    };

    try {
      const notification = new Notification(title, options);
      if (type !== 'error') {
        setTimeout(() => notification.close(), 5000);
      }
    } catch (error) {
      console.warn('Failed to show notification:', error);
    }
  }

  setNotificationsEnabled(enabled: boolean) {
    this.notificationsEnabled = enabled;
    localStorage.setItem('notifications-enabled', enabled.toString());
  }

  loadNotificationsSetting() {
    const saved = localStorage.getItem('notifications-enabled');
    this.notificationsEnabled = saved !== 'false';
    
    const checkbox = document.getElementById('enableNotifications') as HTMLInputElement;
    if (checkbox) {
      checkbox.checked = this.notificationsEnabled;
    }
  }
  // Helper method for efficient base64 conversion
  private async arrayBufferToBase64(buffer: Uint8Array): Promise<string> {
      let binary = '';
      const bytes = new Uint8Array(buffer);
      const chunkSize = 0x8000; // 32KB chunks to avoid call stack issues
      
      for (let i = 0; i < bytes.length; i += chunkSize) {
        const chunk = bytes.subarray(i, i + chunkSize);
        binary += String.fromCharCode.apply(null, Array.from(chunk));
        
        // Yield control back to UI every few chunks for large files
        if (i % (chunkSize * 4) === 0) {
          await new Promise(resolve => setTimeout(resolve, 0));
        }
      }
      
      return btoa(binary);
    }

  // In the AppState class, update loadWebhooks method:
  async loadWebhooks() {
    try {
      this.webhooks = await invoke('get_webhooks');
      
      // Check if currently selected webhook still exists
      if (this.selectedWebhookId) {
        const webhookExists = this.webhooks.some(w => w.id === this.selectedWebhookId);
        if (!webhookExists) {
          // Reset selection if webhook was deleted
          this.selectedWebhookId = null;
        }
      }
      
      this.updateWebhookSelector();
    } catch (error) {
      this.showError(`Failed to load webhooks: ${error}`);
    }
  }

  updateWebhookSelector() {
    const select = document.getElementById('webhookSelect') as HTMLSelectElement;
    if (!select) return;
    
    select.innerHTML = '<option value="">Select a webhook...</option>';
    
    this.webhooks.forEach(webhook => {
      const option = document.createElement('option');
      option.value = webhook.id.toString();
      option.textContent = webhook.name;
      select.appendChild(option);
    });

    // Restore selection if webhook still exists
    if (this.selectedWebhookId) {
      const webhookExists = this.webhooks.some(w => w.id === this.selectedWebhookId);
      if (webhookExists) {
        select.value = this.selectedWebhookId.toString();
      } else {
        // Webhook was deleted, clear selection
        this.selectedWebhookId = null;
        select.value = '';
      }
    }

    this.updateExistingWebhooksDropdown();
    this.updateControlButtons(); // Update upload button state
  }

  updateExistingWebhooksDropdown() {
    const existingSelect = document.getElementById('existingWebhooks') as HTMLSelectElement;
    if (!existingSelect) return;
    
    existingSelect.innerHTML = '<option value="">Select webhook to manage…</option>';
    
    this.webhooks.forEach(webhook => {
      const option = document.createElement('option');
      option.value = webhook.id.toString();
      option.textContent = webhook.name;
      existingSelect.appendChild(option);
    });

    const deleteBtn = document.getElementById('deleteWebhookBtn') as HTMLButtonElement;
    const editBtn = document.getElementById('editWebhookBtn') as HTMLButtonElement;
    if (deleteBtn) deleteBtn.disabled = true;
    if (editBtn) editBtn.disabled = true;
  }

  async addWebhook(name: string, url: string) {
    try {
      const newWebhookId = await invoke('add_webhook', { name, url, is_forum: false });
      await this.loadWebhooks();
      
      // Auto-select the newly created webhook
      this.selectedWebhookId = newWebhookId;
      this.updateWebhookSelector(); // This will now show the new webhook as selected
      
      this.showSuccess('Webhook added and selected!');
    } catch (error) {
      this.showError(`Failed to add webhook: ${error}`);
    }
  }

  async updateWebhook(id: number, name: string, url: string) {
    try {
      await invoke('delete_webhook', { id });
      await invoke('add_webhook', { name, url, is_forum: false });
      await this.loadWebhooks();
      this.showSuccess('Webhook updated successfully!');
    } catch (error) {
      this.showError(`Failed to update webhook: ${error}`);
    }
  }

  async deleteWebhook(id: number) {
    try {
      await invoke('delete_webhook', { id });
      await this.loadWebhooks();
      this.showSuccess('Webhook deleted successfully!');
    } catch (error) {
      this.showError(`Failed to delete webhook: ${error}`);
    }
  }

  resetUploadState() {
    this.isUploading = false;
    this.currentUploadSession = null;
    this.stopProgressPolling();
    
    const startBtn = document.getElementById('startUpload') as HTMLButtonElement;
    const pauseBtn = document.getElementById('pauseUpload') as HTMLButtonElement;
    const retryBtn = document.getElementById('retryFailed');
    const progressSummary = document.getElementById('progressSummary');
    
    if (startBtn) startBtn.disabled = false;
    if (pauseBtn) pauseBtn.classList.add('hidden');
    if (retryBtn) retryBtn.classList.add('hidden');
    if (progressSummary) progressSummary.classList.add('hidden');
    
    this.updateControlButtons();
    
    this.uploadQueue.forEach(item => {
      if (item.status === 'uploading') {
        item.status = 'queued';
        item.progress = 0;
        item.error = null;
      }
    });
    
    this.updateQueueDisplay();
    
    console.log('Upload state reset completely');
  }

  // File handling
  async addFilesToQueue(filePaths: string[]) {
    if (this.isUploading) {
      this.showError('Cannot add files while upload is in progress. Please wait for current upload to complete.');
      return;
    }

    if (filePaths.length === 0) return;

    this.showLoadingIndicator(`Validating ${filePaths.length} files...`);

    const validFiles: QueueItem[] = [];
    const YIELD_INTERVAL = 3; // Yield every 3 files
    const BATCH_SIZE = 5; // Process in batches for better progress updates
    
    try {
      // Process files in batches to prevent UI blocking
      for (let batchStart = 0; batchStart < filePaths.length; batchStart += BATCH_SIZE) {
        const batchEnd = Math.min(batchStart + BATCH_SIZE, filePaths.length);
        const batch = filePaths.slice(batchStart, batchEnd);
        
        console.log(`Processing batch ${Math.floor(batchStart/BATCH_SIZE) + 1}/${Math.ceil(filePaths.length/BATCH_SIZE)}`);
        
        for (let i = 0; i < batch.length; i++) {
          const globalIndex = batchStart + i;
          const filePath = batch[i];
          
          try {
            // Update progress more frequently for better UX
            const progressPercent = Math.round((globalIndex / filePaths.length) * 100);
            this.updateLoadingProgress(
              `Processing files... (${progressPercent}%)`, 
              filePath.split(/[\\/]/).pop()
            );
            
            // Yield control every few files
            if (globalIndex % YIELD_INTERVAL === 0 && globalIndex > 0) {
              console.log(`Yielding control after processing ${globalIndex + 1} files`);
              await new Promise(resolve => setTimeout(resolve, 0));
            }
            
            // Get file info with timeout protection
            console.log(`Processing file: ${filePath}`);
            const params = { filePath: filePath };
            
            // Add timeout to prevent hanging on corrupted files
            const fileInfoPromise = invoke('get_image_info', params) as Promise<[number, number, number]>;
            const timeoutPromise = new Promise<never>((_, reject) => 
              setTimeout(() => reject(new Error('File processing timeout')), 10000)
            );
            
            const [width, height, fileSize] = await Promise.race([fileInfoPromise, timeoutPromise]);
            const filename = filePath.split(/[\\/]/).pop() || filePath;

            // Generate thumbnail data (with reasonable limits for VRChat screenshots)
            let thumbnailData: string | undefined;
            const MAX_THUMBNAIL_SIZE = 50 * 1024 * 1024; // 50MB limit (very generous for VRChat)
            
            if (fileSize <= MAX_THUMBNAIL_SIZE) {
              try {
                // Use Tauri's readBinaryFile for all files
                const imageData = await readBinaryFile(filePath);
                
                // Convert Uint8Array to base64 using your existing method
                const base64String = await this.arrayBufferToBase64(imageData);
                
                const ext = filePath.toLowerCase().split('.').pop();
                const mimeType = ext === 'png' ? 'image/png' : 
                                ext === 'jpg' || ext === 'jpeg' ? 'image/jpeg' : 
                                ext === 'webp' ? 'image/webp' : 
                                ext === 'gif' ? 'image/gif' : 'image/png';
                
                thumbnailData = `data:${mimeType};base64,${base64String}`;
                console.log(`Generated thumbnail for ${filename} (${this.formatFileSize(fileSize)})`);
              } catch (error) {
                console.warn(`Failed to generate thumbnail for ${filename}:`, error);
                // Still add the file to queue even if thumbnail fails
              }
            } else {
              console.log(`Skipping thumbnail for extremely large file: ${filename} (${this.formatFileSize(fileSize)})`);
            }

            const queueItem: QueueItem = {
              id: `${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
              filePath: filePath,
              filename: filename,
              status: 'queued',
              progress: 0,
              error: null,
              fileSize: fileSize,
              dimensions: { width, height },
              retryCount: 0,
              selected: true,
              thumbnailData: thumbnailData,
            };

            validFiles.push(queueItem);
            console.log(`Added to queue: ${filename} (${width}x${height}, ${this.formatFileSize(fileSize)})`);
            
          } catch (error) {
            console.error(`Failed to add ${filePath}:`, error);
            this.showError(`Failed to add ${filePath.split(/[\\/]/).pop()}: ${error}`);
          }
        }
        
        // Yield after each batch to keep UI responsive
        if (batchEnd < filePaths.length) {
          console.log(`Completed batch, yielding before next batch...`);
          await new Promise(resolve => setTimeout(resolve, 10));
        }
      }

    } finally {
      this.hideLoadingIndicator();
    }

    // Update UI with results
    this.uploadQueue.push(...validFiles);
    this.updateQueueDisplay();
    
    if (validFiles.length > 0) {
      this.showSuccess(`Added ${validFiles.length} files to upload queue`);
    }
  }

  updateQueueDisplay() {
    const queueContainer = document.getElementById('uploadQueue');
    const queueItems = document.getElementById('queueItems');
    
    if (this.uploadQueue.length === 0) {
      queueContainer?.classList.add('hidden');
      this.resetUploadState();
      return;
    }

    queueContainer?.classList.remove('hidden');
    
    if (queueItems) {
      queueItems.innerHTML = '';
      
      this.uploadQueue.forEach(item => {
        const itemElement = this.createQueueItemElement(item);
        queueItems.appendChild(itemElement);
      });
    }

    this.updateControlButtons();
  }

  createQueueItemElement(item: QueueItem): HTMLElement {
    const element = document.createElement('div');
    element.className = `queue-item ${item.status}`;
    element.dataset.id = item.id;

    const sizeText = item.fileSize ? this.formatFileSize(item.fileSize) : '';
    const dimensionsText = item.dimensions ? `${item.dimensions.width}×${item.dimensions.height}` : '';

    let statusIcon = '';
    switch (item.status) {
      case 'success':
        statusIcon = '<span class="status-icon success-icon">✅</span>';
        break;
      case 'error':
        statusIcon = '<span class="status-icon error-icon">❌</span>';
        break;
      case 'uploading':
        statusIcon = '<span class="status-icon upload-icon">🔄</span>';
        break;
      default:
        statusIcon = '<span class="status-icon">📄</span>';
    }

    element.innerHTML = `
      <input type="checkbox" class="queue-checkbox" ${item.selected ? 'checked' : ''}>
      <div class="queue-thumbnail">
        ${item.thumbnailData ? 
          `<img src="${item.thumbnailData}" alt="${item.filename}" class="queue-thumbnail-img" />` :
          item.filename.substring(0, 3).toUpperCase()
        }
      </div>
      <div class="queue-info">
        <div class="queue-filename">${item.filename}</div>
        <div class="queue-status">${item.status} ${statusIcon}</div>
        <div class="queue-size">${sizeText} ${dimensionsText}</div>
        ${item.status === 'uploading' ? `
          <div class="queue-progress">
            <div class="queue-progress-bar" style="width: ${item.progress}%"></div>
          </div>
        ` : ''}
        ${item.error ? `<div class="error-message">${item.error}</div>` : ''}
      </div>
      <div class="queue-actions">
        ${item.status === 'error' && item.retryCount < 3 ? `
          <button class="btn btn-small btn-secondary retry-btn" data-id="${item.id}">🔄 Retry</button>
        ` : ''}
        <button class="btn btn-small btn-secondary remove-btn" data-id="${item.id}">🗑️</button>
      </div>
    `;

    const checkbox = element.querySelector('.queue-checkbox') as HTMLInputElement;
    checkbox.addEventListener('change', () => {
      const queueItem = this.uploadQueue.find(q => q.id === item.id);
      if (queueItem) {
        queueItem.selected = checkbox.checked;
        this.updateControlButtons();
      }
    });

    const removeBtn = element.querySelector('.remove-btn');
    removeBtn?.addEventListener('click', () => {
      this.removeFromQueue(item.id);
    });

    const retryBtn = element.querySelector('.retry-btn');
    retryBtn?.addEventListener('click', () => {
      this.retryUpload(item.id);
    });

    // Setup image preview for thumbnail
    const thumbnail = element.querySelector('.queue-thumbnail') as HTMLElement;
    if (thumbnail && item.thumbnailData) {
      previewManager.setupThumbnailPreview(
        thumbnail, 
        item.thumbnailData, 
        item.filename, 
        item.fileSize, 
        item.dimensions
      );
    }

    return element;
  }

  formatFileSize(bytes: number): string {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  }

  removeFromQueue(itemId: string) {
    // Clean up preview listeners before removing
    const item = this.uploadQueue.find(q => q.id === itemId);
    if (item) {
      const element = document.querySelector(`[data-id="${itemId}"]`);
      if (element) {
        const thumbnail = element.querySelector('.queue-thumbnail') as HTMLElement;
        if (thumbnail && (thumbnail as any)._previewCleanup) {
          (thumbnail as any)._previewCleanup();
        }
      }
    }

    this.uploadQueue = this.uploadQueue.filter(item => item.id !== itemId);
    this.updateQueueDisplay();
  }

  clearQueue() {
    // Clean up all preview listeners
    this.uploadQueue.forEach(item => {
      const element = document.querySelector(`[data-id="${item.id}"]`);
      if (element) {
        const thumbnail = element.querySelector('.queue-thumbnail') as HTMLElement;
        if (thumbnail && (thumbnail as any)._previewCleanup) {
          (thumbnail as any)._previewCleanup();
        }
      }
    });

    if (this.isUploading && this.currentUploadSession) {
      console.log('Stopping active upload to clear queue');
      this.forceStopUpload();
    }
    
    this.uploadQueue = [];
    this.updateQueueDisplay();
    this.showInfo('Queue cleared');
  }

  selectAllItems() {
    this.uploadQueue.forEach(item => item.selected = true);
    this.updateQueueDisplay();
  }

  deselectAllItems() {
    this.uploadQueue.forEach(item => item.selected = false);
    this.updateQueueDisplay();
  }

  updateControlButtons() {
    const selectedCount = this.uploadQueue.filter(item => item.selected).length;
    const startBtn = document.getElementById('startUpload') as HTMLButtonElement;
    const viewMetadataBtn = document.getElementById('viewMetadataBtn') as HTMLButtonElement;
    
    if (startBtn) {
      startBtn.disabled = selectedCount === 0 || !this.selectedWebhookId || this.isUploading;
    }
    if (viewMetadataBtn) viewMetadataBtn.disabled = selectedCount === 0;
  }

  async startUpload() {
    if (this.isUploading) {
      console.log('Upload already in progress, ignoring start request');
      return;
    }

    if (!this.selectedWebhookId) {
      this.showError('Please select a webhook first');
      return;
    }

    const selectedItems = this.uploadQueue.filter(item => item.selected);
    if (selectedItems.length === 0) {
      this.showError('Please select files to upload');
      return;
    }

    try {
      this.isUploading = true;
      console.log('Starting upload - setting isUploading = true');

      const groupByMetadata = (document.getElementById('groupByMetadata') as HTMLInputElement).checked;
      const isForumChannel = (document.getElementById('isForumChannel') as HTMLInputElement).checked;
      const maxImages = parseInt((document.getElementById('maxImages') as HTMLSelectElement).value);
      const includePlayerNames = (document.getElementById('includePlayerNames') as HTMLInputElement).checked;

      const progressSummary = document.getElementById('progressSummary');
      progressSummary?.classList.remove('hidden');

      const filePaths = selectedItems.map(item => item.filePath);
      
      console.log('Starting upload with original file paths:', filePaths);

      const sessionId = await invoke('upload_images', {
        request: {
          webhook_id: this.selectedWebhookId,
          file_paths: filePaths,
          group_by_metadata: groupByMetadata,
          max_images_per_message: maxImages,
          is_forum_channel: isForumChannel,
          include_player_names: includePlayerNames
        }
      });

      // CRITICAL: Store the current session ID
      this.currentUploadSession = sessionId as string;
      console.log('Current upload session set to:', sessionId);
      this.showSuccess('Upload started!');
      
      selectedItems.forEach(item => {
        item.status = 'uploading';
      });
      this.updateQueueDisplay();
      
      const startBtn = document.getElementById('startUpload') as HTMLButtonElement;
      const pauseBtn = document.getElementById('pauseUpload') as HTMLButtonElement;
      if (startBtn) startBtn.disabled = true;
      if (pauseBtn) pauseBtn.classList.remove('hidden');
      
      this.startProgressPolling(sessionId as string);
      
    } catch (error) {
      this.isUploading = false;
      this.currentUploadSession = null;
      console.log('Upload failed - setting isUploading = false');
      
      this.showError(`Failed to start upload: ${error}`);
      const progressSummary = document.getElementById('progressSummary');
      progressSummary?.classList.add('hidden');
      
      this.updateControlButtons();
    }
  }

  startProgressPolling(sessionId: string) {
    if (this.progressPollingInterval) {
      clearInterval(this.progressPollingInterval);
    }

    console.log('Starting progress polling for session:', sessionId);

    this.progressPollingInterval = window.setInterval(async () => {
      try {
        const progress: UploadProgress | null = await invoke('get_upload_progress', {
          sessionId: sessionId
        });
        
        if (progress) {
          console.log('Progress update:', {
            completed: progress.completed,
            total: progress.total_images,
            status: progress.session_status,
            current: progress.current_image,
            successful: progress.successful_uploads.length,
            failed: progress.failed_uploads.length
          });
          
          this.updateProgressFromSession(progress);
          
          const allFilesProcessed = progress.completed >= progress.total_images;
          const sessionCompleted = progress.session_status === 'completed' || 
                                  progress.session_status === 'failed' ||
                                  progress.session_status === 'cancelled' ||
                                  allFilesProcessed;
          
          if (sessionCompleted) {
            console.log('Upload session completed, stopping polling');
            this.stopProgressPolling();
            this.onUploadComplete(progress);
          }
        } else {
          console.warn('No progress data received for session:', sessionId);
        }
      } catch (error) {
        console.error('Failed to poll progress:', error);
        this.stopProgressPolling();
        this.showError(`Failed to get upload progress: ${error}`);
        this.isUploading = false;
        this.updateControlButtons();
      }
    }, 1000);
  }

  stopProgressPolling() {
    if (this.progressPollingInterval) {
      clearInterval(this.progressPollingInterval);
      this.progressPollingInterval = null;
    }
  }

  updateProgressFromSession(progress: UploadProgress) {
    this.uploadQueue.forEach(item => {
      if (item.selected && item.filePath) {
        const isSuccessful = progress.successful_uploads.some(path => 
          path.includes(item.filename) || path === item.filePath
        );
        
        if (isSuccessful) {
          item.status = 'success';
          item.progress = 100;
          item.error = null;
          return;
        }
        
        const failedUpload = progress.failed_uploads.find(failed => 
          failed.file_path.includes(item.filename) || failed.file_path === item.filePath
        );
        
        if (failedUpload) {
          item.status = 'error';
          item.error = failedUpload.error || 'Upload failed';
          item.retryCount = failedUpload.retry_count || 0;
          return;
        }
        
        if (progress.current_image) {
          if (progress.current_image.includes(item.filename) || progress.current_image === item.filePath) {
            item.status = 'uploading';
            item.progress = progress.current_progress;
            return;
          }
        }
        
        if (progress.session_status === 'completed' && item.status === 'uploading') {
          item.status = 'success';
          item.progress = 100;
          item.error = null;
        }
      }
    });
    
    this.updateQueueDisplay();
    this.updateProgressSummary(progress);
  }

  updateProgressSummary(progress: UploadProgress) {
    const progressSummary = document.getElementById('progressSummary');
    const progressText = document.getElementById('progressText');
    const progressCount = document.getElementById('progressCount');
    const progressFill = document.getElementById('overallProgressFill') as HTMLElement;
    const estimatedTime = document.getElementById('estimatedTime');
    
    if (progressSummary && !progressSummary.classList.contains('hidden')) {
      if (progressText) {
              if (progress.current_image) {
                if (progress.current_image.includes(' - ')) {
                  // Phase info already included (e.g., "Optimizing - filename")
                  const [phase, filename] = progress.current_image.split(' - ');
                  if (phase === 'Compressing') {
                    progressText.textContent = `Optimizing ${filename} for Discord...`;
                  } else if (phase === 'Uploading') {
                    progressText.textContent = `Uploading ${filename}...`;
                  } else if (phase === 'Preparing') {
                    progressText.textContent = `Preparing ${filename}...`;
                  } else {
                    progressText.textContent = progress.current_image;
                  }
                } else {
                  // Default to uploading
                  const filename = progress.current_image.split(/[\\/]/).pop();
                  progressText.textContent = `Uploading ${filename}...`;
                }
              } else {
                progressText.textContent = 'Preparing uploads...';
              }
            }
      
      if (progressCount) {
        progressCount.textContent = `${progress.completed} / ${progress.total_images}`;
      }
      
      if (progressFill) {
        const percentage = progress.total_images > 0 ? 
          (progress.completed / progress.total_images) * 100 : 0;
        progressFill.style.width = `${percentage}%`;
      }
      
      if (estimatedTime && progress.estimated_time_remaining) {
        const minutes = Math.floor(progress.estimated_time_remaining / 60);
        const seconds = progress.estimated_time_remaining % 60;
        estimatedTime.textContent = `ETA: ${minutes}m ${seconds}s`;
        estimatedTime.classList.remove('hidden');
      }
    }
  }

  onUploadComplete(progress: UploadProgress) {
    const successCount = progress.successful_uploads.length;
    const failedCount = progress.failed_uploads.length;
    
    console.log('Upload completed:', { successCount, failedCount, progress });
    
    this.isUploading = false;
    console.log('Upload complete - setting isUploading = false');
    
    this.uploadQueue.forEach(item => {
      if (item.selected && item.status === 'uploading') {
        const isSuccessful = progress.successful_uploads.some(path => 
          path.includes(item.filename) || path === item.filePath
        );
        
        if (isSuccessful) {
          item.status = 'success';
          item.progress = 100;
          item.error = null;
        } else {
          item.status = 'error';
          item.error = 'Upload status unknown';
        }
      }
    });
    
    this.updateQueueDisplay();
    
    const hasGroupFailure = progress.failed_uploads.some(failure => 
      failure.error.includes('[Group:')
    );
    
    if (progress.session_status === 'cancelled') {
      this.showInfo('Upload was stopped by user');
    } else if (failedCount === 0) {
      this.showSuccess(`Upload complete! Successfully uploaded ${successCount} files!`);
      // Update progress text to show completion
      const progressText = document.getElementById('progressText');
      if (progressText) {
        progressText.textContent = 'Upload complete!';
      }
      this.showDesktopNotification(
        'VRChat Photo Uploader', 
        `Successfully uploaded ${successCount} files!`, 
        'success'
      );
    } else if (successCount === 0) {
      if (hasGroupFailure) {
        this.showError(`Group upload failed - ${failedCount} files not uploaded. Use "Retry Failed" to retry the entire group.`);
      } else {
        this.showError(`Upload failed - ${failedCount} files could not be uploaded.`);
        // Update progress text to show failure
        const progressText = document.getElementById('progressText');
        if (progressText) {
          progressText.textContent = 'Upload failed';
        }
      }
      this.showDesktopNotification(
        'Upload Failed', 
        `Failed to upload ${failedCount} files`, 
        'error'
      );
    } else {
      if (hasGroupFailure) {
        this.showWarning(`Partial success: ${successCount} files uploaded, but a group of ${failedCount} files failed. Use "Retry Failed" to retry the failed group.`);
      } else {
        this.showWarning(`Uploaded ${successCount} files successfully, ${failedCount} failed.`);
      }
      this.showDesktopNotification(
        'Upload Partially Complete', 
        `${successCount} succeeded, ${failedCount} failed`, 
        'info'
      );
    }
    
    // Reset upload state
    this.resetUploadState();
    
    const retryBtn = document.getElementById('retryFailed');
    if (retryBtn && failedCount > 0) {
      retryBtn.classList.remove('hidden');
      
      if (hasGroupFailure) {
        retryBtn.textContent = '🔄 Retry Failed Group';
        retryBtn.title = 'Retry the entire failed group of images';
      } else {
        retryBtn.textContent = '🔄 Retry Failed';
        retryBtn.title = 'Retry individual failed images';
      }
    }
  }

  async retryUpload(itemId: string) {
    const item = this.uploadQueue.find(q => q.id === itemId);
    if (!item || !this.selectedWebhookId || !item.filePath) return;

    try {
      await invoke('retry_failed_upload', {
        sessionId: this.currentUploadSession || '',
        filePath: item.filePath,
        webhookId: this.selectedWebhookId
      });
      
      item.status = 'uploading';
      item.error = null;
      this.updateQueueDisplay();
    } catch (error) {
      this.showError(`Failed to retry upload: ${error}`);
    }
  }

  async retryFailedUploads() {
    if (!this.currentUploadSession) {
      this.showError('No active upload session');
      return;
    }

    const failedItems = this.uploadQueue.filter(item => item.status === 'error' && item.filePath);
    
    if (failedItems.length === 0) {
      this.showWarning('No failed uploads to retry');
      return;
    }

    const groupFailureMessages = failedItems
      .map(item => item.error)
      .filter(error => error && error.includes('[Group:'));
    
    if (groupFailureMessages.length > 0) {
      this.showInfo(`Retrying failed group (${failedItems.length} images)...`);
      
      failedItems.forEach(item => {
        item.status = 'queued';
        item.error = null;
        item.retryCount += 1;
        item.selected = true;
      });
      
      this.updateQueueDisplay();
      
      this.isUploading = false;
      await this.startUpload();
      
    } else {
      for (const item of failedItems) {
        try {
          await invoke('retry_failed_upload', {
            sessionId: this.currentUploadSession,
            filePath: item.filePath,
            webhookId: this.selectedWebhookId
          });
          
          item.status = 'uploading';
          item.error = null;
        } catch (error) {
          this.showError(`Failed to retry ${item.filename}: ${error}`);
        }
      }
      
      this.updateQueueDisplay();
      
      const retryBtn = document.getElementById('retryFailed');
      retryBtn?.classList.add('hidden');
      
      if (this.currentUploadSession) {
        this.startProgressPolling(this.currentUploadSession);
      }
    }
  }

  showLoadingIndicator(message: string) {
    const loadingOverlay = document.getElementById('loadingOverlay');
    const loadingText = document.getElementById('loadingText');
    
    if (loadingOverlay && loadingText) {
      loadingText.textContent = message;
      loadingOverlay.classList.remove('hidden');
    }
  }

  updateLoadingProgress(message: string, fileName?: string) {
    const loadingText = document.getElementById('loadingText');
    if (loadingText) {
      const shortName = fileName ? fileName.split(/[\\/]/).pop() : '';
      const displayMessage = shortName ? `${message}\n\n📄 ${shortName}` : message;
      loadingText.textContent = displayMessage;
    }
  }

  hideLoadingIndicator() {
    const loadingOverlay = document.getElementById('loadingOverlay');
    if (loadingOverlay) {
      loadingOverlay.classList.add('hidden');
    }
  }

  async forceStopUpload() {
    console.log('Force stopping upload session:', this.currentUploadSession);
    
    if (this.currentUploadSession) {
      try {
        await invoke('cancel_upload_session', {
          sessionId: this.currentUploadSession
        });
        console.log('Upload session cancelled successfully via Tauri command');
        this.showInfo('Upload stopped');
      } catch (error) {
        console.error('Failed to cancel upload session via Tauri:', error);
        this.showError(`Failed to stop upload: ${error}`);
      }
    } else {
      console.warn('No active session to cancel');
      this.showWarning('No active upload to stop');
    }
    
    // Always reset local state
    this.resetUploadState();
  }

  // UI helpers
  showSuccess(message: string) {
    this.showToast(message, 'success');
  }

  showError(message: string) {
    this.showToast(message, 'error');
  }

  showWarning(message: string) {
    this.showToast(message, 'warning');
  }

  showInfo(message: string) {
    this.showToast(message, 'info');
  }

  showToast(message: string, type: 'success' | 'error' | 'warning' | 'info' = 'info') {
    const container = document.getElementById('toastContainer');
    if (!container) return;

    const toast = document.createElement('div');
    toast.className = `toast ${type}`;
    toast.textContent = message;

    container.appendChild(toast);

    setTimeout(() => toast.classList.add('show'), 100);

    setTimeout(() => {
      toast.classList.remove('show');
      setTimeout(() => {
        if (container.contains(toast)) {
          container.removeChild(toast);
        }
      }, 300);
    }, 5000);
  }
}

const state = new AppState();

// Image Preview System
class ImagePreviewManager {
  private isCtrlPressed: boolean = false;
  private currentPreview: HTMLElement | null = null;
  private previewContainer: HTMLElement;
  private mousePosition: { x: number; y: number } = { x: 0, y: 0 };
  private previewTimeout: number | null = null;
  private enabled: boolean = true;
  private currentHoveredElement: HTMLElement | null = null;
  private currentHoveredData: { imagePath: string; filename: string; fileSize?: number; dimensions?: { width: number; height: number } } | null = null;

  constructor() {
    this.previewContainer = document.getElementById('imagePreviewContainer') as HTMLElement;
    this.setupEventListeners();
    this.loadSettings();
  }

  private loadSettings() {
    const enabled = localStorage.getItem('image-preview-enabled');
    this.enabled = enabled !== 'false';
  }

  setEnabled(enabled: boolean) {
    this.enabled = enabled;
    localStorage.setItem('image-preview-enabled', enabled.toString());
    
    if (!enabled && this.currentPreview) {
      this.hidePreview();
    }
  }

  private setupEventListeners() {
    // Track Ctrl key state
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Control' && !this.isCtrlPressed) {
        this.isCtrlPressed = true;
        this.updateCtrlHints(true);
        
        // If we're hovering over an element, show preview immediately
        if (this.currentHoveredElement && this.currentHoveredData && this.enabled) {
          this.showPreview(
            this.currentHoveredData.imagePath,
            this.currentHoveredData.filename,
            this.currentHoveredData.fileSize,
            this.currentHoveredData.dimensions
          );
        }
      }
    });

    document.addEventListener('keyup', (e) => {
      if (e.key === 'Control' && this.isCtrlPressed) {
        this.isCtrlPressed = false;
        this.updateCtrlHints(false);
        this.hidePreview();
      }
    });

    // Track mouse position globally
    document.addEventListener('mousemove', (e) => {
      this.mousePosition = { x: e.clientX, y: e.clientY };
    });

    // Handle window blur (when user switches tabs/windows)
    window.addEventListener('blur', () => {
      this.isCtrlPressed = false;
      this.updateCtrlHints(false);
      this.hidePreview();
    });
  }

  private updateCtrlHints(show: boolean) {
    const hints = document.querySelectorAll('.ctrl-preview-hint');
    hints.forEach(hint => {
      if (show) {
        hint.classList.add('show');
      } else {
        hint.classList.remove('show');
      }
    });
  }

  setupThumbnailPreview(thumbnailElement: HTMLElement, imagePath: string, filename: string, fileSize?: number, dimensions?: { width: number; height: number }) {
      if (!this.enabled) return;

      // Add hint element
      const hint = document.createElement('div');
      hint.className = 'ctrl-preview-hint';
      hint.textContent = 'Preview';
      thumbnailElement.style.position = 'relative';
      thumbnailElement.appendChild(hint);

      const handleMouseEnter = () => {
        // Track what we're hovering over
        this.currentHoveredElement = thumbnailElement;
        this.currentHoveredData = { imagePath, filename, fileSize, dimensions };
        
        if (this.isCtrlPressed && this.enabled) {
          this.showPreview(imagePath, filename, fileSize, dimensions);
        }
      };

      const handleMouseLeave = () => {
        // Clear hover tracking
        this.currentHoveredElement = null;
        this.currentHoveredData = null;
        this.hidePreview();
      };

      thumbnailElement.addEventListener('mouseenter', handleMouseEnter);
      thumbnailElement.addEventListener('mouseleave', handleMouseLeave);

      // Store cleanup function for later removal
      (thumbnailElement as any)._previewCleanup = () => {
        thumbnailElement.removeEventListener('mouseenter', handleMouseEnter);
        thumbnailElement.removeEventListener('mouseleave', handleMouseLeave);
        if (hint.parentNode) {
          hint.parentNode.removeChild(hint);
        }
      };
    }

  private showPreview(imagePath: string, filename: string, fileSize?: number, dimensions?: { width: number; height: number }) {
    if (!this.enabled) return;

    // Clear any existing preview
    this.hidePreview();

    // Clear any pending timeout
    if (this.previewTimeout) {
      clearTimeout(this.previewTimeout);
      this.previewTimeout = null;
    }

    // Create preview element
    const preview = document.createElement('div');
    preview.className = 'image-preview';

    // Determine aspect ratio and set appropriate class
    if (dimensions) {
      const aspectRatio = dimensions.width / dimensions.height;
      
      // VRChat standard aspect ratios
      if (Math.abs(aspectRatio - (16/9)) < 0.1) {
        // 16:9 landscape (most common VRChat screenshots)
        preview.classList.add('landscape-large');
      } else if (Math.abs(aspectRatio - (9/16)) < 0.1) {
        // 9:16 portrait (vertical screenshots)
        preview.classList.add('portrait');
      } else if (Math.abs(aspectRatio - 1) < 0.1) {
        // Square images
        preview.classList.add('square');
      } else if (aspectRatio > 1.5) {
        // Wide landscape
        preview.classList.add('landscape-large');
      } else if (aspectRatio < 0.7) {
        // Tall portrait
        preview.classList.add('portrait');
      }
      // Default size will be used for other ratios
    }

    // Create image element
    const img = document.createElement('img');
    img.src = imagePath;
    img.alt = filename;

    // Create info overlay
    const info = document.createElement('div');
    info.className = 'image-preview-info';

    // Add aspect ratio info for VRChat images
    if (dimensions) {
      const aspectRatio = dimensions.width / dimensions.height;
      const aspectDiv = document.createElement('div');
      aspectDiv.className = 'image-preview-aspect';
      
      if (Math.abs(aspectRatio - (16/9)) < 0.1) {
        aspectDiv.textContent = '16:9 Landscape';
      } else if (Math.abs(aspectRatio - (9/16)) < 0.1) {
        aspectDiv.textContent = '9:16 Portrait';
      } else {
        aspectDiv.textContent = `${(aspectRatio).toFixed(2)}:1 Ratio`;
      }
      
      info.appendChild(aspectDiv);
    }

    preview.appendChild(img);
    preview.appendChild(info);

    // Position the preview
    this.positionPreview(preview);

    // Add to container
    this.previewContainer.appendChild(preview);
    this.currentPreview = preview;

    // Trigger show animation
    requestAnimationFrame(() => {
      preview.classList.add('show');
    });
  }

  private positionPreview(preview: HTMLElement) {
    const { x, y } = this.mousePosition;
    
    // Get preview dimensions based on aspect ratio class
    let previewWidth = 356;  // Default 16:9 landscape
    let previewHeight = 200;
    
    if (preview.classList.contains('portrait')) {
      previewWidth = 225;   // 9:16 portrait
      previewHeight = 400;
    } else if (preview.classList.contains('landscape-large')) {
      previewWidth = 400;   // Large 16:9 landscape
      previewHeight = 225;
    } else if (preview.classList.contains('square')) {
      previewWidth = 300;   // Square
      previewHeight = 300;
    }
    
    const offset = 20; // Distance from cursor
    
    const viewport = {
      width: window.innerWidth,
      height: window.innerHeight
    };

    let left = x + offset;
    let top = y + offset;

    // Check if preview would go off the right edge
    if (left + previewWidth > viewport.width) {
      left = x - previewWidth - offset;
    }

    // Check if preview would go off the bottom edge
    if (top + previewHeight > viewport.height) {
      top = y - previewHeight - offset;
    }

    // Ensure preview doesn't go off the left edge
    if (left < 10) {
      left = 10;
    }

    // Ensure preview doesn't go off the top edge
    if (top < 10) {
      top = 10;
    }

    // For very wide screens, consider centering vertically aligned previews
    if (viewport.width > 1400 && preview.classList.contains('portrait')) {
      // Center portrait previews vertically when there's plenty of horizontal space
      if (x > viewport.width / 2) {
        left = x - previewWidth - offset;
      } else {
        left = x + offset;
      }
      // Try to center vertically around cursor
      top = y - previewHeight / 2;
      top = Math.max(10, Math.min(top, viewport.height - previewHeight - 10));
    }

    preview.style.left = `${left}px`;
    preview.style.top = `${top}px`;
  }

  private hidePreview() {
    if (this.currentPreview) {
      const preview = this.currentPreview;
      preview.classList.remove('show');
      preview.classList.add('exiting');

      // Remove after animation
      setTimeout(() => {
        if (preview.parentNode) {
          preview.parentNode.removeChild(preview);
        }
      }, 150);

      this.currentPreview = null;
    }

    // Clear any pending timeout
    if (this.previewTimeout) {
      clearTimeout(this.previewTimeout);
      this.previewTimeout = null;
    }
  }

  private formatFileSize(bytes: number): string {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  }

  cleanup() {
    this.hidePreview();
    this.isCtrlPressed = false;
    this.updateCtrlHints(false);
  }
}

// Initialize preview manager
const previewManager = new ImagePreviewManager();

// Modal management
class ModalManager {
  static openModal(modalId: string) {
    const modal = document.getElementById(modalId);
    modal?.classList.remove('hidden');
  }

  static closeModal(modalId: string) {
    const modal = document.getElementById(modalId);
    modal?.classList.add('hidden');
  }

  static setupModalEvents() {
    document.querySelectorAll('.close-btn').forEach(btn => {
      btn.addEventListener('click', (e) => {
        const modal = (e.target as Element).closest('.modal');
        if (modal) {
          modal.classList.add('hidden');
        }
      });
    });

    document.querySelectorAll('.modal').forEach(modal => {
      modal.addEventListener('click', (e) => {
        if (e.target === modal) {
          modal.classList.add('hidden');
        }
      });
    });
  }
}

// Metadata Editor Variables
let selectedPngPath: string | null = null;
let originalCreationTime: number | null = null;

// Initialize everything when DOM is loaded
document.addEventListener('DOMContentLoaded', async () => {
  console.log('DOM loaded, initializing app...');

  // Load initial data
  await state.loadWebhooks();
  state.loadNotificationsSetting();

  // Setup modal events
  ModalManager.setupModalEvents();

  listen('tauri://file-drop', async (event) => {
    const filePaths = event.payload as string[];
    console.log('Native drag & drop - files:', filePaths);
    
    // Filter for image files only
    const imageFiles = filePaths.filter(path => {
      const ext = path.toLowerCase().split('.').pop();
      return ['png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp'].includes(ext || '');
    });
    
    if (imageFiles.length === 0) {
      state.showWarning('No valid image files were dropped');
      return;
    }
    
    await state.addFilesToQueue(imageFiles);
    state.showSuccess(`Added ${imageFiles.length} images via drag & drop`);
  });

  // Listen for system tray events
  listen('upload-files-request', async () => {
    console.log('Tray: Upload files requested');
    try {
      const selected = await open({
        multiple: true,
        filters: [{
          name: 'Images',
          extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp']
        }]
      });
      
      if (selected && Array.isArray(selected)) {
        await state.addFilesToQueue(selected);
        state.showSuccess(`Added ${selected.length} files from tray menu`);
      } else if (selected && typeof selected === 'string') {
        await state.addFilesToQueue([selected]);
        state.showSuccess('Added 1 file from tray menu');
      }
    } catch (error) {
      state.showError(`Failed to select files from tray: ${error}`);
    }
  });

  listen('show-settings', () => {
    console.log('Tray: Settings requested');
    updateVRChatFolderDisplay();
    ModalManager.openModal('settingsModal');
  });

  listen('show-about', () => {
    console.log('Tray: About requested');
    ModalManager.openModal('aboutModal');
  });

  listen('show-metadata-editor', () => {
  console.log('Tray: Metadata Editor requested');
  ModalManager.openModal('metadataEditorModal');
  });

  // Listen for global shortcut events
  listen('global-shortcut-upload', async () => {
    console.log('Global shortcut triggered: Upload files');
    
    const globalShortcutsEnabled = localStorage.getItem('global-shortcuts-enabled') !== 'false';
    if (!globalShortcutsEnabled) {
      console.log('Global shortcuts are disabled, ignoring shortcut');
      return;
    }
    
    try {
      const selected = await open({
        multiple: true,
        filters: [{
          name: 'Images',
          extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp']
        }]
      });
      
      if (selected && Array.isArray(selected)) {
        console.log('Selected files via global shortcut:', selected);
        await state.addFilesToQueue(selected);
        state.showSuccess(`Added ${selected.length} files via global shortcut (Ctrl+Shift+U)`);
      } else if (selected && typeof selected === 'string') {
        console.log('Selected single file via global shortcut:', selected);
        await state.addFilesToQueue([selected]);
        state.showSuccess('Added 1 file via global shortcut (Ctrl+Shift+U)');
      }
    } catch (error) {
      console.error('Global shortcut file selection error:', error);
      state.showError(`Global shortcut failed: ${error}`);
    }
  });

  function loadGlobalShortcutsSetting() {
    const enabled = localStorage.getItem('global-shortcuts-enabled') !== 'false';
    const checkbox = document.getElementById('enableGlobalShortcuts') as HTMLInputElement;
    if (checkbox) {
      checkbox.checked = enabled;
    }
    return enabled;
  }

  // Image preview settings
  const enableImagePreview = document.getElementById('enableImagePreview') as HTMLInputElement;
  enableImagePreview?.addEventListener('change', (e) => {
    const target = e.target as HTMLInputElement;
    const enabled = target.checked;
    previewManager.setEnabled(enabled);
    
    if (enabled) {
      state.showSuccess('Image previews enabled (Ctrl+Hover)');
    } else {
      state.showInfo('Image previews disabled');
    }
  });

  // Load image preview setting
  function loadImagePreviewSetting() {
    const enabled = localStorage.getItem('image-preview-enabled') !== 'false';
    const checkbox = document.getElementById('enableImagePreview') as HTMLInputElement;
    if (checkbox) {
      checkbox.checked = enabled;
    }
    previewManager.setEnabled(enabled);
    return enabled;
  }

  loadImagePreviewSetting();

  // Webhook selector
  const webhookSelect = document.getElementById('webhookSelect') as HTMLSelectElement;
  webhookSelect.addEventListener('change', (e) => {
    const target = e.target as HTMLSelectElement;
    state.selectedWebhookId = target.value ? parseInt(target.value) : null;
    state.updateControlButtons();
  });

  state.updateExistingWebhooksDropdown();

  // Manage webhooks button
  const manageWebhooksBtn = document.getElementById('manageWebhooksBtn');
  manageWebhooksBtn?.addEventListener('click', () => {
    const nameInput = document.getElementById('webhookName') as HTMLInputElement;
    const urlInput = document.getElementById('webhookUrl') as HTMLInputElement;
    const addBtn = document.getElementById('addWebhookBtn');
    
    if (nameInput) nameInput.value = '';
    if (urlInput) urlInput.value = '';
    if (addBtn) {
      addBtn.textContent = '➕ Add Webhook';
      delete addBtn.dataset.editingId;
    }

    ModalManager.openModal('webhookModal');
  });

  // Add webhook button
  const addWebhookBtn = document.getElementById('addWebhookBtn');
  addWebhookBtn?.addEventListener('click', async () => {
    const nameInput = document.getElementById('webhookName') as HTMLInputElement;
    const urlInput = document.getElementById('webhookUrl') as HTMLInputElement;
    const addBtn = addWebhookBtn;
    
    if (!nameInput.value.trim() || !urlInput.value.trim()) {
      state.showError('Please enter both name and URL');
      return;
    }

    const editingId = addBtn.dataset.editingId;
    
    if (editingId) {
      await state.updateWebhook(
        parseInt(editingId), 
        nameInput.value.trim(), 
        urlInput.value.trim()
      );
    } else {
      await state.addWebhook(
        nameInput.value.trim(), 
        urlInput.value.trim()
      );
      
      // Reset selection after adding new webhook
      state.selectedWebhookId = null;
    }

    // Clear form
    nameInput.value = '';
    urlInput.value = '';
    addBtn.textContent = '➕ Add Webhook';
    delete addBtn.dataset.editingId;
    
    const existingSelect = document.getElementById('existingWebhooks') as HTMLSelectElement;
    if (existingSelect) existingSelect.value = '';
    
    ModalManager.closeModal('webhookModal');
  });

  // Existing webhooks dropdown
  const existingWebhooksSelect = document.getElementById('existingWebhooks') as HTMLSelectElement;
  existingWebhooksSelect?.addEventListener('change', (e) => {
    const target = e.target as HTMLSelectElement;
    const selectedId = target.value;
    
    const deleteBtn = document.getElementById('deleteWebhookBtn') as HTMLButtonElement;
    const editBtn = document.getElementById('editWebhookBtn') as HTMLButtonElement;
    
    if (deleteBtn) deleteBtn.disabled = !selectedId;
    if (editBtn) editBtn.disabled = !selectedId;
  });

  // Delete webhook button
  const deleteWebhookBtn = document.getElementById('deleteWebhookBtn');
  deleteWebhookBtn?.addEventListener('click', async () => {
    const existingSelect = document.getElementById('existingWebhooks') as HTMLSelectElement;
    const selectedId = existingSelect.value;
    
    if (!selectedId) {
      state.showError('Please select a webhook to delete');
      return;
    }

    const selectedWebhook = state.webhooks.find(w => w.id.toString() === selectedId);
    if (!selectedWebhook) return;

    if (confirm(`Are you sure you want to delete "${selectedWebhook.name}"?`)) {
      await state.deleteWebhook(parseInt(selectedId));
      existingSelect.value = '';
      const deleteBtn = document.getElementById('deleteWebhookBtn') as HTMLButtonElement;
      const editBtn = document.getElementById('editWebhookBtn') as HTMLButtonElement;
      if (deleteBtn) deleteBtn.disabled = true;
      if (editBtn) editBtn.disabled = true;
    }
  });

  // Edit webhook button
  const editWebhookBtn = document.getElementById('editWebhookBtn');
  editWebhookBtn?.addEventListener('click', () => {
    const existingSelect = document.getElementById('existingWebhooks') as HTMLSelectElement;
    const selectedId = existingSelect.value;
    
    if (!selectedId) {
      state.showError('Please select a webhook to edit');
      return;
    }

    const selectedWebhook = state.webhooks.find(w => w.id.toString() === selectedId);
    if (!selectedWebhook) return;

    const nameInput = document.getElementById('webhookName') as HTMLInputElement;
    const urlInput = document.getElementById('webhookUrl') as HTMLInputElement;

    if (nameInput) nameInput.value = selectedWebhook.name;
    if (urlInput) urlInput.value = selectedWebhook.url;

    const addBtn = document.getElementById('addWebhookBtn');
    if (addBtn) {
      addBtn.textContent = '💾 Update Webhook';
      addBtn.dataset.editingId = selectedId;
    }

    state.showInfo('Webhook loaded for editing. Modify the details above and click "Update Webhook".');
  });

  // File upload handling
  const dropZone = document.getElementById('dropZone');

  dropZone?.addEventListener('click', async () => {
    try {
      const selected = await open({
        multiple: true,
        filters: [{
          name: 'Images',
          extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp']
        }]
      });
      
      if (selected && Array.isArray(selected)) {
        console.log('Selected files via dialog:', selected);
        await state.addFilesToQueue(selected);
      } else if (selected && typeof selected === 'string') {
        console.log('Selected single file via dialog:', selected);
        await state.addFilesToQueue([selected]);
      }
    } catch (error) {
      console.error('File selection error:', error);
      state.showError(`Failed to select files: ${error}`);
    }
  });

  // Drag & Drop Event Listeners
  dropZone?.addEventListener('dragenter', (e) => {
    e.preventDefault();
    e.stopPropagation();
    dropZone.classList.add('dragover');
  });

  dropZone?.addEventListener('dragover', (e) => {
    e.preventDefault();
    e.stopPropagation();
    dropZone.classList.add('dragover');
  });

  dropZone?.addEventListener('dragleave', (e) => {
    e.preventDefault();
    e.stopPropagation();
    if (!dropZone.contains(e.relatedTarget as Node)) {
      dropZone.classList.remove('dragover');
    }
  });

  dropZone?.addEventListener('drop', async (e) => {
    e.preventDefault();
    e.stopPropagation();
    dropZone.classList.remove('dragover');
    
    state.showInfo('Drag & drop detected! Please use the file dialog to select your images.');
    
    try {
      const selected = await open({
        multiple: true,
        filters: [{
          name: 'Images',
          extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp']
        }]
      });
      
      if (selected && Array.isArray(selected)) {
        console.log('Selected files via drag & drop dialog:', selected);
        await state.addFilesToQueue(selected);
      } else if (selected && typeof selected === 'string') {
        console.log('Selected single file via drag & drop dialog:', selected);
        await state.addFilesToQueue([selected]);
      }
    } catch (error) {
      console.error('File selection error after drag & drop:', error);
      state.showError(`Failed to select files: ${error}`);
    }
  });

  // Queue control buttons
  const selectAllBtn = document.getElementById('selectAllBtn');
  selectAllBtn?.addEventListener('click', () => state.selectAllItems());

  const deselectAllBtn = document.getElementById('deselectAllBtn');
  deselectAllBtn?.addEventListener('click', () => state.deselectAllItems());

  const removeSelectedBtn = document.getElementById('removeSelectedBtn');
  removeSelectedBtn?.addEventListener('click', () => {
    const selectedIds = state.uploadQueue.filter(item => item.selected).map(item => item.id);
    selectedIds.forEach(id => state.removeFromQueue(id));
  });

  const clearQueueBtn = document.getElementById('clearQueue');
  clearQueueBtn?.addEventListener('click', () => state.clearQueue());

  const startUploadBtn = document.getElementById('startUpload');
  if (startUploadBtn) {
    const newStartBtn = startUploadBtn.cloneNode(true) as HTMLButtonElement;
    startUploadBtn.parentNode?.replaceChild(newStartBtn, startUploadBtn);
    
    newStartBtn.addEventListener('click', () => {
      console.log('Start upload button clicked');
      state.startUpload();
    });
  }

  const retryFailedBtn = document.getElementById('retryFailed');
  retryFailedBtn?.addEventListener('click', () => state.retryFailedUploads());

  // FIXED PAUSE BUTTON EVENT LISTENER
  const pauseUploadBtn = document.getElementById('pauseUpload');
  if (pauseUploadBtn) {
    // Remove any existing listeners by cloning the node
    const newPauseBtn = pauseUploadBtn.cloneNode(true) as HTMLButtonElement;
    pauseUploadBtn.parentNode?.replaceChild(newPauseBtn, pauseUploadBtn);
    
    newPauseBtn.addEventListener('click', async () => {
      console.log('Pause/Stop button clicked');
      
      try {
        // Show immediate feedback
        newPauseBtn.disabled = true;
        newPauseBtn.textContent = '⏹️ Stopping...';
        
        await state.forceStopUpload();
        
        // Button will be hidden by resetUploadState, but just in case:
        newPauseBtn.disabled = false;
        newPauseBtn.textContent = '⏸️ Pause';
        
      } catch (error) {
        console.error('Error stopping upload:', error);
        state.showError(`Failed to stop upload: ${error}`);
        
        // Re-enable button on error
        newPauseBtn.disabled = false;
        newPauseBtn.textContent = '⏸️ Pause';
      }
    });
  }

  // Forum channel warning
  const isForumChannelCheckbox = document.getElementById('isForumChannel') as HTMLInputElement;
  const maxImagesSelect = document.getElementById('maxImages') as HTMLSelectElement;

  // Quick actions
  let selectedVRChatFolder: string | null = localStorage.getItem('vrchat-folder-path');

  const openVRChatFolderBtn = document.getElementById('openVRChatFolderBtn');
  openVRChatFolderBtn?.addEventListener('click', async () => {
    try {
      if (selectedVRChatFolder) {
        await invoke('shell_open', { path: selectedVRChatFolder });
        state.showSuccess(`Opened VRChat folder: ${selectedVRChatFolder}`);
      } else {
        const selected = await open({
          directory: true,
          title: 'Select VRChat Photos Folder'
        });
        
        if (selected && typeof selected === 'string') {
          selectedVRChatFolder = selected;
          localStorage.setItem('vrchat-folder-path', selected);
          state.showSuccess(`Selected VRChat folder: ${selected}`);
          
          if (openVRChatFolderBtn) {
            openVRChatFolderBtn.innerHTML = '📂 Open VRChat Folder';
          }
          updateVRChatFolderDisplay();
        }
      }
    } catch (error) {
      state.showError(`Failed to open folder: ${error}`);
    }
  });

  if (selectedVRChatFolder && openVRChatFolderBtn) {
    openVRChatFolderBtn.innerHTML = '📂 Open VRChat Folder';
  }

  // Settings and other modals
  const settingsBtn = document.getElementById('settingsBtn');
  settingsBtn?.addEventListener('click', () => {
    updateVRChatFolderDisplay();
    ModalManager.openModal('settingsModal');
  });

  const aboutBtn = document.getElementById('aboutBtn');
  aboutBtn?.addEventListener('click', () => {
    ModalManager.openModal('aboutModal');
  });

  // Metadata tools
  const viewMetadataBtn = document.getElementById('viewMetadataBtn');
  viewMetadataBtn?.addEventListener('click', async () => {
    const selectedItems = state.uploadQueue.filter(item => item.selected);
    if (selectedItems.length > 0) {
      try {
        console.log('Calling get_image_metadata with:', { filePath: selectedItems[0].filePath });
        
        const metadata = await invoke('get_image_metadata', { 
          filePath: selectedItems[0].filePath
        });
        
        if (metadata) {
          state.showSuccess(`Metadata found for ${selectedItems[0].filename}`);
          console.log('Metadata:', metadata);
        } else {
          state.showWarning(`No metadata found in ${selectedItems[0].filename}`);
        }
      } catch (error) {
        state.showError(`Failed to read metadata: ${error}`);
      }
    }
  });

  const metadataEditorBtn = document.getElementById('metadataEditorBtn');
  metadataEditorBtn?.addEventListener('click', () => {
    ModalManager.openModal('metadataEditorModal');
  });

  // Metadata Editor functionality
  const loadPngMetadataBtn = document.getElementById('loadPngMetadataBtn');
  loadPngMetadataBtn?.addEventListener('click', async () => {
    try {
      const selected = await open({
        filters: [{
          name: 'PNG Images',
          extensions: ['png']
        }]
      });
      
      if (selected && typeof selected === 'string') {
        // Show loading state
        const originalText = loadPngMetadataBtn.innerHTML;
        loadPngMetadataBtn.innerHTML = '🔄 Loading Metadata...';
        loadPngMetadataBtn.disabled = true;
        
        try {
          await loadPngMetadata(selected);
        } finally {
          // Restore button state
          loadPngMetadataBtn.innerHTML = originalText;
          loadPngMetadataBtn.disabled = false;
        }
      }
    } catch (error) {
      state.showError(`Failed to load PNG: ${error}`);
      // Restore button in case of error
      loadPngMetadataBtn.innerHTML = '📂 Load PNG Metadata';
      loadPngMetadataBtn.disabled = false;
    }
  });

  const loadRawJsonBtn = document.getElementById('loadRawJsonBtn');
  loadRawJsonBtn?.addEventListener('click', () => {
    const rawJsonText = document.getElementById('rawJsonText') as HTMLTextAreaElement;
    const rawJson = rawJsonText.value.trim();
    
    if (!rawJson) {
      state.showError('No raw JSON provided');
      return;
    }
    
    try {
      const metadata = JSON.parse(rawJson);
      populateFormFields(metadata);
      state.showSuccess('Loaded raw JSON metadata');
    } catch (error) {
      state.showError(`Invalid JSON: ${error}`);
    }
  });

  const selectPngForEmbeddingBtn = document.getElementById('selectPngForEmbeddingBtn');
  selectPngForEmbeddingBtn?.addEventListener('click', async () => {
    try {
      const selected = await open({
        filters: [{
          name: 'PNG Images',
          extensions: ['png']
        }]
      });
      
      if (selected && typeof selected === 'string') {
        selectedPngPath = selected;
        const filename = selected.split(/[\\/]/).pop() || selected;
        const selectedPngInfo = document.getElementById('selectedPngInfo');
        if (selectedPngInfo) {
          selectedPngInfo.textContent = `Selected for embedding: ${filename}`;
        }
        
        const embedBtn = document.getElementById('embedMetadataBtn') as HTMLButtonElement;
        if (embedBtn) embedBtn.disabled = false;
        
        state.showSuccess(`Selected PNG for embedding: ${filename}`);
      }
    } catch (error) {
      state.showError(`Failed to select PNG: ${error}`);
    }
  });

  const embedMetadataBtn = document.getElementById('embedMetadataBtn');
  embedMetadataBtn?.addEventListener('click', async () => {
    if (!selectedPngPath) {
      state.showError('No PNG file selected for embedding');
      return;
    }
    
    // Show loading state
    const originalText = embedMetadataBtn.innerHTML;
    embedMetadataBtn.innerHTML = '🔄 Embedding Metadata...';
    embedMetadataBtn.disabled = true;
    
    try {
      await embedMetadataIntoPng();
    } finally {
      // Restore button state
      embedMetadataBtn.innerHTML = originalText;
      embedMetadataBtn.disabled = false;
    }
  });

  async function loadPngMetadata(filePath: string) {
    try {
      // Extract metadata using your existing command
      const metadata = await invoke('get_image_metadata', { filePath });
      
      if (!metadata) {
        state.showError('No metadata found in the PNG file');
        return;
      }
      
      // Get file info for timestamps
      try {
        const [width, height, fileSize] = await invoke('get_image_info', { filePath }) as [number, number, number];
        // Extract creation time from filename
        originalCreationTime = extractCreationTimeFromFilename(filePath);
        
        const creationDateSpan = document.getElementById('creationDate');
        if (creationDateSpan && originalCreationTime) {
          const date = new Date(originalCreationTime * 1000);
          creationDateSpan.textContent = `Creation Date: ${date.toLocaleString()}`;
        }
      } catch (error) {
        console.warn('Could not get file info:', error);
      }
      
      // Populate form fields
      populateFormFields(metadata);
      
      // Update raw JSON display
      const rawJsonText = document.getElementById('rawJsonText') as HTMLTextAreaElement;
      if (rawJsonText) {
        rawJsonText.value = JSON.stringify(metadata, null, 2);
      }
      
      state.showSuccess('Loaded metadata and timestamps');
      
    } catch (error) {
      state.showError(`Error loading PNG metadata: ${error}`);
    }
  }

  function populateFormFields(metadata: any) {
    // Author fields
    const authorDisplayName = document.getElementById('authorDisplayName') as HTMLInputElement;
    const authorId = document.getElementById('authorId') as HTMLInputElement;
    
    if (metadata.author) {
      if (authorDisplayName) authorDisplayName.value = metadata.author.display_name || metadata.author.displayName || '';
      if (authorId) authorId.value = metadata.author.id || '';
    }
    
    // World fields
    const worldName = document.getElementById('worldName') as HTMLInputElement;
    const worldId = document.getElementById('worldId') as HTMLInputElement;
    const worldInstanceId = document.getElementById('worldInstanceId') as HTMLInputElement;
    
    if (metadata.world) {
      if (worldName) worldName.value = metadata.world.name || '';
      if (worldId) worldId.value = metadata.world.id || '';
      if (worldInstanceId) worldInstanceId.value = metadata.world.instance_id || metadata.world.instanceId || '';
    }

    // Players field
    const playersText = document.getElementById('playersText') as HTMLTextAreaElement;
    if (playersText && metadata.players && Array.isArray(metadata.players)) {
      const lines = metadata.players.map((p: any) => `${p.display_name || p.displayName || ''}, ${p.id || ''}`);
      playersText.value = lines.join('\n');
    }
  }

  function extractCreationTimeFromFilename(filePath: string): number | null {
    const filename = filePath.split(/[\\/]/).pop() || '';
    const match = filename.match(/(\d{4}-\d{2}-\d{2})_(\d{2}-\d{2}-\d{2}(?:\.\d+)?)/);
    
    if (match) {
      const [, datePart, timePart] = match;
      const timeFormatted = timePart.replace(/-/g, ':');
      const dateTimeStr = `${datePart} ${timeFormatted}`;
      
      try {
        const date = new Date(dateTimeStr);
        return Math.floor(date.getTime() / 1000);
      } catch (error) {
        console.warn('Failed to parse creation time from filename:', error);
      }
    }
    
    return null;
  }

  async function embedMetadataIntoPng() {
    if (!selectedPngPath) {
      state.showError('No PNG file selected');
      return;
    }
    
    // Gather metadata from form fields
    const authorDisplayName = (document.getElementById('authorDisplayName') as HTMLInputElement).value.trim();
    const authorId = (document.getElementById('authorId') as HTMLInputElement).value.trim();
    const worldName = (document.getElementById('worldName') as HTMLInputElement).value.trim();
    const worldId = (document.getElementById('worldId') as HTMLInputElement).value.trim();
    const worldInstanceId = (document.getElementById('worldInstanceId') as HTMLInputElement).value.trim();
    const playersText = (document.getElementById('playersText') as HTMLTextAreaElement).value.trim();
    
    // Parse players
    const players = [];
    if (playersText) {
      const lines = playersText.split('\n');
      for (const line of lines) {
        if (line.includes(',')) {
          const [displayName, id] = line.split(',').map(s => s.trim());
          if (displayName && id) {
            players.push({ displayName, id });
          }
        }
      }
    }
    
    // Create metadata object
    const metadata = {
      application: "VRChat Photo Uploader",
      version: 2,
      created_at: new Date().toISOString(),
      author: authorDisplayName && authorId ? { display_name: authorDisplayName, id: authorId } : null,
      world: worldName && worldId ? { name: worldName, id: worldId, instance_id: worldInstanceId } : null,
      players: players.map(p => ({ display_name: p.displayName, id: p.id }))
    };
    
    // Remove null values
    Object.keys(metadata).forEach(key => {
      if (metadata[key as keyof typeof metadata] === null) {
        delete metadata[key as keyof typeof metadata];
      }
    });
    
    try {
      // Use the update_image_metadata command to embed metadata
      const outputPath = await invoke('update_image_metadata', {
        filePath: selectedPngPath,
        metadata: metadata
      });
      
      state.showSuccess(`Metadata embedded successfully! Saved as: ${outputPath.split(/[\\/]/).pop()}`);
      
      // Reset the form
      selectedPngPath = null;
      const embedBtn = document.getElementById('embedMetadataBtn') as HTMLButtonElement;
      if (embedBtn) embedBtn.disabled = true;
      
      const selectedPngInfo = document.getElementById('selectedPngInfo');
      if (selectedPngInfo) selectedPngInfo.textContent = '';
      
    } catch (error) {
      state.showError(`Failed to embed metadata: ${error}`);
    }
  }

  // Theme handling
  const themeSelect = document.getElementById('themeSelect') as HTMLSelectElement;
  themeSelect?.addEventListener('change', (e) => {
    const theme = (e.target as HTMLSelectElement).value;
    applyTheme(theme);
    localStorage.setItem('theme-preference', theme);
  });

  function applyTheme(theme: string) {
    if (theme === 'auto') {
      const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
      document.documentElement.setAttribute('data-theme', prefersDark ? 'dark' : 'light');
    } else {
      document.documentElement.setAttribute('data-theme', theme);
    }
  }

  function loadTheme() {
    const savedTheme = localStorage.getItem('theme-preference') || 'dark';
    const themeSelect = document.getElementById('themeSelect') as HTMLSelectElement;
    if (themeSelect) {
      themeSelect.value = savedTheme;
    }
    applyTheme(savedTheme);
  }

  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
    const currentTheme = localStorage.getItem('theme-preference') || 'dark';
    if (currentTheme === 'auto') {
      applyTheme('auto');
    }
  });

  loadTheme();

  // Notification settings
  const enableNotifications = document.getElementById('enableNotifications') as HTMLInputElement;
  enableNotifications?.addEventListener('change', (e) => {
    const target = e.target as HTMLInputElement;
    state.setNotificationsEnabled(target.checked);
    
    if (target.checked) {
      state.showDesktopNotification(
        'Notifications Enabled', 
        'You will now receive desktop notifications for uploads', 
        'success'
      );
    }
  });

  // Global shortcuts settings
  const enableGlobalShortcuts = document.getElementById('enableGlobalShortcuts') as HTMLInputElement;
  enableGlobalShortcuts?.addEventListener('change', (e) => {
    const target = e.target as HTMLInputElement;
    const enabled = target.checked;
    localStorage.setItem('global-shortcuts-enabled', enabled.toString());
    
    if (enabled) {
      state.showSuccess('Global shortcuts enabled (Ctrl+Shift+U)');
    } else {
      state.showInfo('Global shortcuts disabled');
    }
  });

  // Settings Save and Cancel buttons
  const saveSettingsBtn = document.getElementById('saveSettingsBtn');
  saveSettingsBtn?.addEventListener('click', async () => {
    try {
      const themeSelect = document.getElementById('themeSelect') as HTMLSelectElement;
      const enableNotifications = document.getElementById('enableNotifications') as HTMLInputElement;
      const enableGlobalShortcuts = document.getElementById('enableGlobalShortcuts') as HTMLInputElement;
      const groupByMetadata = document.getElementById('groupByMetadata') as HTMLInputElement;
      const maxImages = document.getElementById('maxImages') as HTMLSelectElement;

      if (themeSelect) {
        const theme = themeSelect.value;
        localStorage.setItem('theme-preference', theme);
        applyTheme(theme);
      }

      const config: AppConfig = {
        last_webhook_id: state.selectedWebhookId,
        group_by_metadata: groupByMetadata?.checked || true,
        max_images_per_message: parseInt(maxImages?.value || '10'),
        enable_global_shortcuts: enableGlobalShortcuts?.checked || true,
        auto_compress_threshold: 8,
        upload_quality: 85,
      };

      await invoke('save_app_config', { config });
      
      state.showSuccess('Settings saved successfully!');
      ModalManager.closeModal('settingsModal');
    } catch (error) {
      state.showError(`Failed to save settings: ${error}`);
    }
  });

  const cancelSettingsBtn = document.getElementById('cancelSettingsBtn');
  cancelSettingsBtn?.addEventListener('click', () => {
    ModalManager.closeModal('settingsModal');
  });

  // Clear VRChat folder button
  const clearVRChatFolderBtn = document.getElementById('clearVRChatFolderBtn');
  clearVRChatFolderBtn?.addEventListener('click', () => {
    localStorage.removeItem('vrchat-folder-path');
    selectedVRChatFolder = null;
    
    const openVRChatFolderBtn = document.getElementById('openVRChatFolderBtn');
    if (openVRChatFolderBtn) {
      openVRChatFolderBtn.innerHTML = '📂 Select VRChat Folder';
    }
    
    updateVRChatFolderDisplay();
    state.showSuccess('VRChat folder location cleared');
  });

  function updateVRChatFolderDisplay() {
    const currentPathSpan = document.getElementById('currentVRChatPath');
    const clearBtn = document.getElementById('clearVRChatFolderBtn') as HTMLButtonElement;
    
    if (!currentPathSpan) {
      console.warn('currentVRChatPath element not found - settings modal may not be open');
      return;
    }
    
    if (selectedVRChatFolder) {
      const shortPath = selectedVRChatFolder.length > 50 
        ? '...' + selectedVRChatFolder.slice(-47)
        : selectedVRChatFolder;
      currentPathSpan.textContent = shortPath;
      currentPathSpan.style.color = 'var(--text-primary)';
      if (clearBtn) clearBtn.disabled = false;
    } else {
      currentPathSpan.textContent = 'No folder selected';
      currentPathSpan.style.color = 'var(--text-muted)';
      if (clearBtn) clearBtn.disabled = true;
    }
  }

  updateVRChatFolderDisplay();
  
  console.log('App initialized successfully');
});