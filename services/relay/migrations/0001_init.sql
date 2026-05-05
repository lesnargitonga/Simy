CREATE TABLE IF NOT EXISTS relay_mailboxes (
    mailbox_id TEXT PRIMARY KEY,
    access_token_hash BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS relay_messages (
    message_id UUID PRIMARY KEY,
    mailbox_id TEXT NOT NULL REFERENCES relay_mailboxes(mailbox_id) ON DELETE CASCADE,
    sender_device_hint TEXT,
    ciphertext BYTEA NOT NULL,
    received_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    deleted_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS relay_messages_mailbox_received_idx
    ON relay_messages (mailbox_id, received_at ASC)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS relay_messages_expiry_idx
    ON relay_messages (expires_at ASC);
