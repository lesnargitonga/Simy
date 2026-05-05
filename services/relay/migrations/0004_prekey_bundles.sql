CREATE TABLE IF NOT EXISTS relay_prekey_bundles (
    mailbox_id TEXT PRIMARY KEY REFERENCES relay_mailboxes(mailbox_id) ON DELETE CASCADE,
    identity_signing_key BYTEA NOT NULL,
    identity_exchange_key BYTEA NOT NULL,
    signed_prekey BYTEA NOT NULL,
    signed_prekey_signature BYTEA NOT NULL,
    signed_prekey_created_at TIMESTAMPTZ NOT NULL,
    signed_prekey_expires_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS relay_prekey_bundles_expiry_idx
    ON relay_prekey_bundles (signed_prekey_expires_at ASC);

CREATE TABLE IF NOT EXISTS relay_one_time_prekeys (
    prekey_id UUID PRIMARY KEY,
    mailbox_id TEXT NOT NULL REFERENCES relay_mailboxes(mailbox_id) ON DELETE CASCADE,
    public_key BYTEA NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS relay_one_time_prekeys_mailbox_idx
    ON relay_one_time_prekeys (mailbox_id, created_at ASC)
    WHERE consumed_at IS NULL;