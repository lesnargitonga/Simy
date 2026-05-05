# Relay API and Mailbox Semantics

## Mailbox Model

A mailbox is identified by a high-entropy `mailbox_id` plus a separate retrieval secret. The retrieval secret is generated on the client, base64-encoded for transport, and hashed server-side before storage.

## Required Client Behavior

- Generate mailbox IDs with high entropy.
- Generate mailbox retrieval tokens with at least 32 random bytes.
- Generate a unique `replay_token` for every submission attempt.
- Encrypt payloads fully on the client before submission.
- Use the mailbox token only for retrieval and deletion, not for message submission.
- Upload large encrypted media blobs to object storage and send only encrypted blob pointers through the relay.
- Model private feed-style posts as encrypted content envelopes so the relay never needs to understand social graph semantics.

## Security Properties of the Current Relay

- The relay never stores plaintext message content.
- Duplicate submission attempts can be rejected through replay-token reservation.
- Retrieval and deletion are authenticated using a mailbox-scoped secret.
- Media upload intents are authenticated using the same mailbox-scoped secret.
- Expired or deleted messages are cleaned up in the background.
- X3DH prekey-bundle publication is authenticated using the mailbox-scoped secret.
- One-time prekeys are consumed server-side when public bundles are fetched.

## X3DH Prekey Bundles

The relay now supports the asynchronous first-contact bootstrap through:

- `POST /v1/mailboxes/{mailbox_id}/prekeys`
- `GET /v1/prekeys/{mailbox_id}`

Mailbox owners publish:

- Ed25519 identity signing public key
- X25519 identity exchange public key
- X25519 signed prekey public key
- Ed25519 signature over the signed prekey
- zero or more X25519 one-time prekeys

The relay verifies the signed prekey signature before storing the bundle. When another client fetches the bundle, the relay returns the active signed prekey data and consumes one available one-time prekey so it cannot be handed out twice.

Owners can inspect remaining one-time-prekey inventory through:

- `GET /v1/mailboxes/{mailbox_id}/prekeys/status`

This endpoint is mailbox-authenticated and returns the active signed-prekey expiry plus the current count of unconsumed one-time prekeys.

## Device Records

The relay now supports signed device records through:

- `POST /v1/mailboxes/{mailbox_id}/devices`
- `GET /v1/devices/{mailbox_id}`

Each device record contains:

- device identifier
- user-visible device label
- Ed25519 device signing public key
- X25519 device exchange public key
- creation timestamp
- optional revocation timestamp
- Ed25519 signature from the mailbox identity key

The relay verifies the device record signature against the identity signing key from the mailbox's active prekey bundle before storing the record. This lets clients discover public device state without giving the relay authority to mint or alter trust data.

## Initial Session Bootstrap Envelope

The core crate now defines a serializable first-contact envelope built on top of X3DH. It carries:

- sender identity signing public key
- sender identity exchange public key
- sender ephemeral public key
- receiver signed prekey public key
- optional receiver one-time prekey public key
- content kind
- nonce
- encrypted first payload ciphertext

This fixes an important rotation issue: the envelope explicitly identifies which receiver signed prekey was used, so a recipient that keeps overlapping signed prekeys during rotation can choose the correct secret when opening the bootstrap message.

## Media Upload Intents

The relay now exposes a media upload-intent endpoint for mailbox owners:

- `POST /v1/mailboxes/{mailbox_id}/media/upload-intents`

This endpoint does not upload media itself. It plans a padded encrypted blob upload by returning:

- object-store bucket
- object key
- padded target size
- chunk size
- upload endpoint
- presigned upload request method
- presigned upload URL
- required presigned request headers
- expiry timestamp

The client is still responsible for encrypting, padding, hashing, and uploading the ciphertext blob.

## Media Manifests

After the client uploads the encrypted blob to object storage, it can register the completed object with:

- `POST /v1/mailboxes/{mailbox_id}/media/manifests`

The manifest binds a completed encrypted object to the earlier upload intent and records:

- object key
- media type
- original size
- padded size
- chunk size
- optional SHA-256 digest metadata

Owners can review stored media objects with:

- `GET /v1/mailboxes/{mailbox_id}/media/manifests`

## Media Access Grants

The relay now supports short-lived delegated access metadata through:

- `POST /v1/mailboxes/{mailbox_id}/media/access-grants`
- `GET /v1/media/access-grants/{grant_token}`

The owner creates a grant for a registered media object. The relay stores only a hash of the grant token. A recipient holding the encrypted token can later resolve it to obtain:

- object-store endpoint
- bucket
- object key
- media type
- padded size and chunk size
- optional content digest metadata
- presigned request method
- presigned request URL
- required presigned request headers

Grant resolution is now the bridge into the object store: the relay turns the short-lived grant into an executable S3-compatible presigned request for either `download` or delegated `upload`.

## Remaining Gaps Before Production

- The relay still treats ratcheted session messages as opaque ciphertext and does not coordinate client-side ratchet persistence or replay policy.
- Abuse throttling is coarse and should be extended with network-aware controls at the edge.
- Object storage presigning exists for local development, but production hardening still needs tighter bucket policy design, lifecycle controls, and credential rotation.
- Audit-trail synchronization and signed device-event history are not implemented yet.
- MLS-backed group session handling is still missing.
