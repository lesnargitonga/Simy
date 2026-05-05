CREATE TABLE IF NOT EXISTS relay_feed_posts (
    post_id UUID PRIMARY KEY,
    author_mailbox_id TEXT NOT NULL REFERENCES relay_mailboxes(mailbox_id) ON DELETE CASCADE,
    author_codename TEXT NOT NULL,
    audience TEXT NOT NULL CHECK (audience IN ('contacts')),
    reply_policy TEXT NOT NULL CHECK (reply_policy IN ('no_replies', 'contacts_only')),
    author_ciphertext_b64 TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS relay_feed_posts_author_idx
    ON relay_feed_posts (author_mailbox_id, created_at DESC);

CREATE INDEX IF NOT EXISTS relay_feed_posts_expiry_idx
    ON relay_feed_posts (expires_at);

CREATE TABLE IF NOT EXISTS relay_feed_post_deliveries (
    delivery_id UUID PRIMARY KEY,
    post_id UUID NOT NULL REFERENCES relay_feed_posts(post_id) ON DELETE CASCADE,
    recipient_mailbox_id TEXT NOT NULL REFERENCES relay_mailboxes(mailbox_id) ON DELETE CASCADE,
    ciphertext_b64 TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (post_id, recipient_mailbox_id)
);

CREATE INDEX IF NOT EXISTS relay_feed_post_deliveries_recipient_idx
    ON relay_feed_post_deliveries (recipient_mailbox_id, created_at DESC);

CREATE INDEX IF NOT EXISTS relay_feed_post_deliveries_post_idx
    ON relay_feed_post_deliveries (post_id);