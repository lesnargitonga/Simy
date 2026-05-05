CREATE TABLE IF NOT EXISTS media_objects (
    object_id UUID PRIMARY KEY,
    mailbox_id TEXT NOT NULL REFERENCES relay_mailboxes(mailbox_id) ON DELETE CASCADE,
    intent_id UUID UNIQUE REFERENCES media_upload_intents(intent_id) ON DELETE SET NULL,
    bucket TEXT NOT NULL,
    object_key TEXT NOT NULL UNIQUE,
    media_type TEXT NOT NULL,
    original_size_bytes BIGINT NOT NULL,
    padded_size_bytes BIGINT NOT NULL,
    chunk_size_bytes INTEGER NOT NULL,
    content_sha256_b64 TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    upload_completed_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS media_objects_mailbox_idx
    ON media_objects (mailbox_id, upload_completed_at DESC);

CREATE TABLE IF NOT EXISTS media_access_grants (
    grant_id UUID PRIMARY KEY,
    object_id UUID NOT NULL REFERENCES media_objects(object_id) ON DELETE CASCADE,
    mailbox_id TEXT NOT NULL REFERENCES relay_mailboxes(mailbox_id) ON DELETE CASCADE,
    grant_token_hash BYTEA NOT NULL UNIQUE,
    operation TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    redeemed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS media_access_grants_expiry_idx
    ON media_access_grants (expires_at ASC);
