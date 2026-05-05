CREATE TABLE IF NOT EXISTS relay_contacts (
    owner_mailbox_id TEXT NOT NULL REFERENCES relay_mailboxes(mailbox_id) ON DELETE CASCADE,
    contact_mailbox_id TEXT NOT NULL REFERENCES relay_mailboxes(mailbox_id) ON DELETE CASCADE,
    codename TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (owner_mailbox_id, contact_mailbox_id),
    CHECK (owner_mailbox_id <> contact_mailbox_id)
);

CREATE INDEX IF NOT EXISTS relay_contacts_owner_idx
    ON relay_contacts (owner_mailbox_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS relay_contacts_contact_idx
    ON relay_contacts (contact_mailbox_id, updated_at DESC);