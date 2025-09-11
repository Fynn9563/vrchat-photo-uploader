use crate::errors::{AppError, AppResult};
use reqwest::{multipart, Client};
use std::cmp::min;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration, Instant};

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub exponential_base: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(1000),
            max_delay: Duration::from_secs(120),
            exponential_base: 2.0,
        }
    }
}

/// Enhanced Discord API client with proper rate limiting
pub struct DiscordClient {
    client: Client,
    rate_limiter: Arc<Mutex<HashMap<String, Instant>>>,
    retry_config: RetryConfig,
}

impl DiscordClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap(),
            rate_limiter: Arc::new(Mutex::new(HashMap::new())),
            retry_config: RetryConfig::default(),
        }
    }

    pub async fn send_webhook_with_thread_id(
        &self,
        webhook_url: &str,
        payload: &UploadPayload,
        thread_id: Option<&str>,
    ) -> AppResult<String> {
        let webhook_id = self.extract_webhook_id(webhook_url);
        self.wait_for_rate_limit(&webhook_id).await;

        let mut attempt = 0;

        loop {
            let form = payload.build_form()?;

            // Build URL with required query parameters
            let mut url_parts = vec![];

            // Always add wait=true to get response data
            url_parts.push("wait=true".to_string());

            // Add thread_id as query parameter if provided
            if let Some(tid) = thread_id {
                url_parts.push(format!("thread_id={}", tid));
                log::info!("🔗 Adding thread_id to URL query: {}", tid);
            }

            let final_url = if webhook_url.contains('?') {
                format!("{}&{}", webhook_url, url_parts.join("&"))
            } else {
                format!("{}?{}", webhook_url, url_parts.join("&"))
            };

            log::debug!("Final webhook URL: {}", final_url);

            let response = self.client.post(&final_url).multipart(form).send().await?;

            let status = response.status();

            // Update rate limit state based on response headers
            self.update_rate_limit(&webhook_id, &response).await;

            if status.is_success() {
                let response_text = response.text().await?;
                log::debug!(
                    "Discord webhook response (first 300 chars): {}",
                    &response_text[..std::cmp::min(300, response_text.len())]
                );
                return Ok(response_text);
            }

            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            let error = AppError::UploadFailed {
                reason: format!(
                    "Discord API error {} for webhook {}: {}",
                    status, webhook_url, error_text
                ),
            };

            log::warn!(
                "Upload attempt {} failed for webhook {}, retrying: {}",
                attempt,
                self.extract_webhook_id(webhook_url),
                error
            );

            // Check if we should retry
            attempt += 1;
            if should_retry_error(status.as_u16()) && attempt <= self.retry_config.max_retries {
                let delay = if status == 429 {
                    self.extract_retry_after(&error_text)
                        .unwrap_or_else(|| self.calculate_backoff_delay(attempt))
                } else {
                    self.calculate_backoff_delay(attempt)
                };

                log::warn!(
                    "Upload attempt {} failed, retrying in {:?}: {}",
                    attempt,
                    delay,
                    error
                );
                sleep(delay).await;
                continue;
            }

            return Err(error);
        }
    }

    fn extract_webhook_id(&self, url: &str) -> String {
        url.split('/').nth_back(1).unwrap_or("default").to_string()
    }

    async fn wait_for_rate_limit(&self, webhook_id: &str) {
        let wait_time = {
            match self.rate_limiter.lock() {
                Ok(rate_limiter) => {
                    if let Some(&last_request) = rate_limiter.get(webhook_id) {
                        let elapsed = last_request.elapsed();
                        const MIN_DELAY: Duration = Duration::from_millis(1000); // Discord rate limit

                        if elapsed < MIN_DELAY {
                            Some(MIN_DELAY - elapsed)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                Err(e) => {
                    log::warn!("Failed to acquire rate limiter lock (non-critical): {}", e);
                    None
                }
            }
        }; // MutexGuard is dropped here

        if let Some(wait_time) = wait_time {
            sleep(wait_time).await;
        }
    }

    async fn update_rate_limit(&self, webhook_id: &str, _response: &reqwest::Response) {
        match self.rate_limiter.lock() {
            Ok(mut rate_limiter) => {
                rate_limiter.insert(webhook_id.to_string(), Instant::now());
            }
            Err(e) => {
                log::warn!("Failed to update rate limiter (non-critical): {}", e);
            }
        }
    }

    fn calculate_backoff_delay(&self, attempt: u32) -> Duration {
        let delay_ms = self.retry_config.base_delay.as_millis() as f64
            * self.retry_config.exponential_base.powi(attempt as i32 - 1);

        let delay = Duration::from_millis(delay_ms as u64);
        min(delay, self.retry_config.max_delay)
    }

    fn extract_retry_after(&self, error_text: &str) -> Option<Duration> {
        // Try to parse retry-after from Discord error response
        if let Some(start) = error_text.find("retry_after") {
            if let Some(end) = error_text[start..].find(',') {
                let retry_section = &error_text[start..start + end];
                if let Some(colon_pos) = retry_section.find(':') {
                    let value_str = &retry_section[colon_pos + 1..].trim();
                    if let Ok(seconds) = value_str.parse::<f64>() {
                        return Some(Duration::from_secs_f64(seconds));
                    }
                }
            }
        }
        None
    }
}

/// Helper struct to hold upload payload data
#[derive(Debug, Clone)]
pub struct UploadPayload {
    files: Vec<(String, Vec<u8>, String, String)>, // (filename, data, mime_type, field_name)
    text_fields: HashMap<String, String>,
}

impl UploadPayload {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            text_fields: HashMap::new(),
        }
    }

    pub fn add_text_field(&mut self, key: String, value: String) {
        self.text_fields.insert(key, value);
    }

    pub async fn add_file(&mut self, file_path: &str, field_name: String) -> AppResult<()> {
        let file_contents = tokio::fs::read(file_path).await?;
        let filename = Path::new(file_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Detect MIME type based on file extension
        let mime_type = match Path::new(file_path).extension().and_then(|e| e.to_str()) {
            Some("png") => "image/png",
            Some("jpg") | Some("jpeg") => "image/jpeg",
            Some("webp") => "image/webp",
            Some("gif") => "image/gif",
            _ => "image/png", // Default fallback
        };

        self.files
            .push((filename, file_contents, mime_type.to_string(), field_name));
        Ok(())
    }

    pub fn build_form(&self) -> AppResult<multipart::Form> {
        let mut form = multipart::Form::new();

        // Add text fields
        for (key, value) in &self.text_fields {
            form = form.text(key.clone(), value.clone());
        }

        // Add files
        for (filename, data, mime_type, field_name) in &self.files {
            let part = multipart::Part::bytes(data.clone())
                .file_name(filename.clone())
                .mime_str(mime_type)?;

            form = form.part(field_name.clone(), part);
        }

        Ok(form)
    }
}

fn should_retry_error(status_code: u16) -> bool {
    matches!(status_code, 429 | 500 | 502 | 503 | 504)
}

/// Extract thread ID from Discord response for forum channels
pub fn extract_thread_id(response_data: &str) -> Option<String> {
    log::info!("🔍 Attempting to extract thread_id from Discord response");
    log::debug!("Response data length: {} bytes", response_data.len());

    if response_data.is_empty() {
        log::error!("❌ Empty response body - this suggests wait=true was not used!");
        return None;
    }

    log::debug!(
        "First 200 chars of response: {}",
        &response_data[..std::cmp::min(200, response_data.len())]
    );

    // Parse Discord response to extract thread/channel ID for forum posts
    match serde_json::from_str::<serde_json::Value>(response_data) {
        Ok(json) => {
            log::debug!("✅ Successfully parsed Discord response as JSON");

            // For forum posts, Discord returns the thread/channel ID in the 'id' field
            if let Some(id_value) = json.get("id") {
                if let Some(id_str) = id_value.as_str() {
                    log::info!(
                        "🎉 Successfully extracted thread_id from 'id' field: {}",
                        id_str
                    );
                    return Some(id_str.to_string());
                }
            }

            // Alternative: sometimes it might be in 'channel_id'
            if let Some(channel_id) = json.get("channel_id").and_then(|v| v.as_str()) {
                log::info!(
                    "🎉 Extracted thread_id from 'channel_id' field: {}",
                    channel_id
                );
                return Some(channel_id.to_string());
            }

            // Log the full structure for debugging
            log::debug!(
                "Discord response structure: {}",
                serde_json::to_string_pretty(&json).unwrap_or_else(|_| "Invalid JSON".to_string())
            );

            // Check if there are any other ID-like fields
            if let Some(obj) = json.as_object() {
                for (key, value) in obj {
                    if key.contains("id") && value.is_string() {
                        log::debug!("Found ID field '{}': {}", key, value);
                    }
                }
            }

            log::error!("❌ No thread_id found in any expected fields (id, channel_id)");
        }
        Err(e) => {
            log::error!("❌ Failed to parse Discord response as JSON: {}", e);
            log::debug!("Raw response that failed to parse: {}", response_data);
        }
    }

    log::error!("❌ Could not extract thread_id from Discord response");
    None
}
