CREATE TABLE IF NOT EXISTS relay_feed_post_replies (
    reply_id UUID PRIMARY KEY,
    post_id UUID NOT NULL REFERENCES relay_feed_posts(post_id) ON DELETE CASCADE,
    author_mailbox_id TEXT NOT NULL REFERENCES relay_mailboxes(mailbox_id) ON DELETE CASCADE,
    author_codename TEXT NOT NULL,
    recipient_mailbox_id TEXT NOT NULL REFERENCES relay_mailboxes(mailbox_id) ON DELETE CASCADE,
    author_ciphertext_b64 TEXT NOT NULL,
    recipient_ciphertext_b64 TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    CHECK (author_mailbox_id <> recipient_mailbox_id)
);

CREATE INDEX IF NOT EXISTS relay_feed_post_replies_post_idx
    ON relay_feed_post_replies (post_id, created_at ASC);

CREATE INDEX IF NOT EXISTS relay_feed_post_replies_author_idx
    ON relay_feed_post_replies (author_mailbox_id, created_at DESC);

CREATE INDEX IF NOT EXISTS relay_feed_post_replies_recipient_idx
    ON relay_feed_post_replies (recipient_mailbox_id, created_at DESC);

CREATE INDEX IF NOT EXISTS relay_feed_post_replies_expiry_idx
    ON relay_feed_post_replies (expires_at);