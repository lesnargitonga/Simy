CREATE TABLE IF NOT EXISTS relay_accounts (
    mailbox_id TEXT PRIMARY KEY REFERENCES relay_mailboxes(mailbox_id) ON DELETE CASCADE,
    codename TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('admin', 'user')),
    status TEXT NOT NULL CHECK (status IN ('provisioned', 'active', 'disabled')),
    owner_mailbox_id TEXT REFERENCES relay_accounts(mailbox_id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    activated_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS relay_accounts_owner_idx
    ON relay_accounts (owner_mailbox_id, created_at DESC);

CREATE INDEX IF NOT EXISTS relay_accounts_role_status_idx
    ON relay_accounts (role, status, created_at DESC);
