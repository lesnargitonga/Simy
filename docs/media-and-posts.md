# Media and Private Posts

## Goal

Support pictures and feed-style private posts without breaking the relay's defensive posture or turning the server into a metadata-rich content platform.

## Design Rule

Large media must not travel through the mailbox relay. The relay should carry only small encrypted pointer envelopes. Encrypted media blobs belong in object storage.

## Media Flow

1. The client strips EXIF and other file metadata before encryption.
2. The client generates a one-time symmetric media key.
3. The client encrypts the media locally.
4. The client pads the encrypted blob to a standard size bucket.
5. The client uploads the padded ciphertext blob to object storage.
6. The client sends an encrypted pointer message through the relay.
7. The recipient decrypts the pointer locally, downloads the blob, and decrypts it locally.

## Current Build Status

Implemented now:

- local MinIO object storage in [docker-compose.yml](../docker-compose.yml)
- media padding bucket selection in [crates/comm-core/src/lib.rs](../crates/comm-core/src/lib.rs)
- chunked XChaCha20-Poly1305 media encryption in [crates/comm-core/src/lib.rs](../crates/comm-core/src/lib.rs)
- encrypted blob pointer descriptor types in [crates/comm-core/src/lib.rs](../crates/comm-core/src/lib.rs)
- feed-post descriptor types in [crates/comm-core/src/lib.rs](../crates/comm-core/src/lib.rs)
- authenticated media upload-intent endpoint in [services/relay/src/main.rs](../services/relay/src/main.rs)
- S3-compatible presigned upload request generation in [services/relay/src/main.rs](../services/relay/src/main.rs)
- authenticated media manifest registration in [services/relay/src/main.rs](../services/relay/src/main.rs)
- short-lived media access grants in [services/relay/src/main.rs](../services/relay/src/main.rs)
- S3-compatible presigned access-request resolution in [services/relay/src/main.rs](../services/relay/src/main.rs)
- relay-backed private feed post creation and listing in [services/relay/src/main.rs](../services/relay/src/main.rs)
- relay-backed private feed replies scoped between the post author and one recipient mailbox in [services/relay/src/main.rs](../services/relay/src/main.rs)
- same-origin encrypted media fetch through access grants in [services/relay/src/main.rs](../services/relay/src/main.rs)
- browser private feed composer and timeline with local AES-GCM encryption plus media upload/grant embedding in [services/relay/static/index.html](../services/relay/static/index.html)

Not implemented yet:

- background media upload queue in clients
- image thumbnail pipeline with privacy review
- post audience distribution through audited end-to-end group state
- proof-of-upload checks before manifest registration
- production bucket policy hardening and object lifecycle enforcement

## Padding Strategy

The current core crate exposes fixed buckets:

- 1 MiB
- 5 MiB
- 10 MiB
- 25 MiB
- 50 MiB

The client should choose the smallest bucket that fits the encrypted file, then upload that padded ciphertext blob. This reduces easy traffic analysis based on exact object size.

## Private Feed Posts

A private post should be treated as an encrypted content type, not as a server-readable feed record.

Current implementation shape:

- post identifier
- one author-visible encrypted ciphertext copy
- one encrypted ciphertext copy per recipient contact
- mailbox-paired encrypted replies between the post author and each replying recipient
- audience type
- reply policy
- optional encrypted media grant descriptor embedded in the encrypted browser payload
- expiration time

The relay still remains blind to the plaintext content. It validates recipients against relay-backed contacts and stores only opaque ciphertext copies plus distribution metadata.

## Local Development

MinIO is exposed at:

- API: `http://127.0.0.1:9000`
- Console: `http://127.0.0.1:9001`
- Bucket: `simy-media`

For local development the bootstrap container ensures the `simy-media` bucket exists.

## Next Engineering Steps

1. add streaming file encryption and integrity verification to the Rust core
2. enforce object existence or upload-completion checks before manifest registration
3. add client-side EXIF stripping and media queue management
4. define encrypted post payload schemas shared across mobile and desktop clients
5. integrate audited session and group cryptography so posts and pointers are distributed end to end
