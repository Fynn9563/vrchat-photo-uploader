-- Add user_webhook_overrides table
CREATE TABLE IF NOT EXISTS user_webhook_overrides (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id TEXT,
    user_display_name TEXT,
    webhook_id INTEGER NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (webhook_id) REFERENCES webhooks (id) ON DELETE CASCADE,
    UNIQUE(user_id, webhook_id),
    UNIQUE(user_display_name, webhook_id)
);
