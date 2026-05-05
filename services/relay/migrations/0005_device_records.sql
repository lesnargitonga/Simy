CREATE TABLE IF NOT EXISTS relay_device_records (
    mailbox_id TEXT NOT NULL REFERENCES relay_mailboxes(mailbox_id) ON DELETE CASCADE,
    device_id TEXT NOT NULL,
    device_label TEXT NOT NULL,
    device_signing_key BYTEA NOT NULL,
    device_exchange_key BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    signature BYTEA NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (mailbox_id, device_id)
);

CREATE INDEX IF NOT EXISTS relay_device_records_mailbox_idx
    ON relay_device_records (mailbox_id, created_at DESC);