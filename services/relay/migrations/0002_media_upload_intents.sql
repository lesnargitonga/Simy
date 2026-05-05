CREATE TABLE IF NOT EXISTS media_upload_intents (
    intent_id UUID PRIMARY KEY,
    mailbox_id TEXT NOT NULL REFERENCES relay_mailboxes(mailbox_id) ON DELETE CASCADE,
    bucket TEXT NOT NULL,
    object_key TEXT NOT NULL UNIQUE,
    media_type TEXT NOT NULL,
    original_size_bytes BIGINT NOT NULL,
    padded_size_bytes BIGINT NOT NULL,
    chunk_size_bytes INTEGER NOT NULL,
    content_sha256_b64 TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS media_upload_intents_mailbox_idx
    ON media_upload_intents (mailbox_id, created_at DESC);

CREATE INDEX IF NOT EXISTS media_upload_intents_expiry_idx
    ON media_upload_intents (expires_at ASC);
