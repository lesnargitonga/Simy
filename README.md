# Simy Secure Communications Rebuild

Simy is a defensive rebuild of a privacy-preserving communications platform for high-risk use cases. The repository currently delivers the secure relay and operational foundation first: a Rust workspace, a shared cryptographic core crate, a PostgreSQL-backed relay service, Redis-backed replay and throttle controls, a browser-based secure workspace plus operator console, server-backed account and role records, local infrastructure for development, local object storage for encrypted media blobs, and detailed security documentation.

## Showcase Focus

- Security-first systems design in Rust.
- Cryptographic protocol foundations for asynchronous secure messaging.
- Durable relay infrastructure with explicit operational controls and documentation.

## At a Glance

- Shared crypto core for identity material, X3DH, and Double Ratchet foundations.
- PostgreSQL-backed relay with Redis replay detection and throttling.
- Browser-based secure workspace and protected operator controls.
- Managed-user provisioning and server-backed roles.
- Desktop reference client with encrypted session persistence.

This repository is intentionally opinionated about what it does and does not do.

- It does implement a durable ciphertext relay with mailbox provisioning, retrieval authentication, TTL cleanup, integration-test tooling, hardened local and CI test paths, local media-blob development infrastructure, server-backed admin bootstrap, managed-user provisioning, and a desktop ratchet-persistence reference client.
- It does not yet implement a production-ready end-user X3DH plus Double Ratchet messaging client with audited session lifecycle handling, or MLS-backed group state.
- It does not yet include complete Android or iOS clients, and the desktop client is still a development reference surface rather than a finished messenger.

## Table of Contents

1. Overview
2. Repository Structure
3. Implemented Components
4. Security Model
5. Relay API Surface
6. Live Testing
7. Local Development Setup
8. Runtime Configuration
9. Data Model and Storage
10. Operational Behavior
11. Documentation Map
12. Current Gaps
13. Media and Private Posts
14. Recommended Next Steps

## Overview

The repository is structured as a Rust workspace because the long-term design depends on a shared, memory-safe core that can be reused across mobile and desktop clients. The relay service exists to carry encrypted envelopes, not to inspect or transform message content. PostgreSQL stores mailbox and message records durably. Redis provides replay detection and coarse rate-limiting state. Docker Compose stands up the local backing services so the relay can be tested end to end on a developer machine.

The current root route of the relay also serves a browser-based secure chat client with a hidden advanced relay drawer. The main screen now focuses on explicit workspace entry: open a saved workspace, activate a managed user mailbox from issued join data, open a mailbox from a user-shared onboarding link, or intentionally enter the protected operator path only when the live relay admin token is available. The protected surface is no longer presented as a normal first-run option; it is intentionally hidden behind an explicit access route and session unlock. The advanced drawer still exposes mailbox, relay, media, prekey, and device lifecycle testing when needed. It is a meaningful live client surface, but it is still not the final X3DH plus Double Ratchet messenger. In parallel, the desktop Tauri client now provides a working reference implementation of encrypted Double Ratchet session persistence and restart-safe session continuity.

## Repository Structure

### Workspace root

- [Cargo.toml](Cargo.toml): Rust workspace manifest and shared dependency versions.
- [.env.example](.env.example): required runtime configuration template.
- [.env](.env): local runtime configuration used on this machine.
- [docker-compose.yml](docker-compose.yml): local PostgreSQL, Redis, and MinIO services.
- [.gitignore](.gitignore): ignored build and environment artifacts.

### Shared core crate

- [crates/comm-core/Cargo.toml](crates/comm-core/Cargo.toml): core crate manifest.
- [crates/comm-core/src/lib.rs](crates/comm-core/src/lib.rs): identity material generation, public identity serialization, media padding plans, chunked media encryption, and feed/media descriptor helpers.
- [crates/comm-core/src/x3dh.rs](crates/comm-core/src/x3dh.rs): X3DH prekey generation, bundle verification, and initiator/responder shared-secret derivation.
- [crates/comm-core/src/ratchet_store_fs.rs](crates/comm-core/src/ratchet_store_fs.rs): feature-gated encrypted file-backed Double Ratchet session store for restart-safe local persistence.
- [crates/comm-core/examples/generate_prekey_bundle.rs](crates/comm-core/examples/generate_prekey_bundle.rs): local helper that emits a valid prekey-bundle publish payload for relay testing.
- [crates/comm-core/examples/bootstrap_initial_message.rs](crates/comm-core/examples/bootstrap_initial_message.rs): local example that seals and opens a first-contact X3DH bootstrap envelope.
- [crates/comm-core/examples/bootstrap_double_ratchet.rs](crates/comm-core/examples/bootstrap_double_ratchet.rs): local example that bootstraps a Double Ratchet session from X3DH and exchanges messages in both directions.
- [crates/comm-core/examples/generate_identity_fixture.rs](crates/comm-core/examples/generate_identity_fixture.rs): local helper that emits a coherent prekey-bundle payload and signed device record for relay lifecycle testing.

### Relay service

- [services/relay/Cargo.toml](services/relay/Cargo.toml): relay crate manifest.
- [services/relay/src/main.rs](services/relay/src/main.rs): HTTP service, config loading, health checks, mailbox creation, message submission, retrieval, deletion, Redis integration, and cleanup worker.
- [services/relay/migrations/0001_init.sql](services/relay/migrations/0001_init.sql): initial schema for mailboxes and relay messages.
- [services/relay/migrations/0002_media_upload_intents.sql](services/relay/migrations/0002_media_upload_intents.sql): upload-intent schema for planned encrypted media objects.
- [services/relay/migrations/0003_media_manifests_and_access_grants.sql](services/relay/migrations/0003_media_manifests_and_access_grants.sql): completed media object registration and short-lived access grants.
- [services/relay/migrations/0006_accounts.sql](services/relay/migrations/0006_accounts.sql): server-backed account records, roles, statuses, and admin ownership of managed users.
- [services/relay/static/index.html](services/relay/static/index.html): browser-based secure workspace and relay operator workbench served at `/`.
- [services/relay/README.md](services/relay/README.md): relay-specific capabilities and API summary.

### Documentation

- [docs/architecture.md](docs/architecture.md): system-level architecture overview.
- [docs/threat-model.md](docs/threat-model.md): adversaries, assumptions, and required mitigations.
- [docs/key-management.md](docs/key-management.md): key classes, lifecycle, verification, and recovery posture.
- [docs/data-retention.md](docs/data-retention.md): retention rules and deletion expectations.
- [docs/security-test-plan.md](docs/security-test-plan.md): pre-release security gates and test areas.
- [docs/relay-api.md](docs/relay-api.md): relay-specific mailbox and envelope semantics.
- [docs/push-notifications.md](docs/push-notifications.md): push constraints and metadata-minimization guidance.
- [docs/deployment.md](docs/deployment.md): deployment baseline and operational security controls.
- [docs/live-testing.md](docs/live-testing.md): local browser and manual API testing instructions.
- [docs/media-and-posts.md](docs/media-and-posts.md): object-storage, media pointer, and private-post design notes.
- [docs/next-build-plan.md](docs/next-build-plan.md): prioritized continuation plan from the current foundation.

### Client planning

- [clients/README.md](clients/README.md): platform target overview.
- [clients/android/README.md](clients/android/README.md): Android wrapper plan.
- [clients/ios/README.md](clients/ios/README.md): iOS wrapper plan.
- [clients/desktop/README.md](clients/desktop/README.md): desktop client implementation status, session-persistence architecture, and next integration steps.

## Implemented Components

### 1. Shared identity core

The core crate provides the first slice of reusable protocol-adjacent logic.

Current capabilities:

- generates Ed25519 signing keys on-device
- generates X25519 identity exchange keys for asynchronous session bootstrapping
- derives public identity bundles
- emits deterministic public key fingerprints
- base64-encodes public identity material for transport
- validates and decodes public identity bundles
- generates signed prekeys and one-time prekeys for X3DH
- derives matching X3DH shared secrets for initiators and responders
- seals and opens a serializable X3DH initial-message envelope carrying the first encrypted payload
- bootstraps Double Ratchet initiator and responder sessions from the X3DH handshake
- encrypts and decrypts ratcheted session messages with header-bound associated data
- stores skipped message keys so out-of-order delivery can still be opened safely inside a bounded window
- generates media encryption material for XChaCha20-Poly1305 content protection
- encrypts padded media in deterministic chunks with integrity metadata
- decrypts chunked media and rejects tampered ciphertext

This code is still not an audited production messenger protocol, but it now contains the concrete first-contact and post-bootstrap 1:1 session primitives the future clients need to share.

### 2. Relay service

The relay is the most complete part of the repository today.

Current capabilities:

- binds to a configured address
- checks PostgreSQL and Redis health
- auto-runs SQL migrations at startup
- reopens existing mailboxes with the correct retrieval token and only creates new mailboxes when the request is authenticated by protected admin flow or by an existing authenticated mailbox origin
- stores server-backed account profiles with `admin` and `user` roles
- bootstraps an admin account only when the mailbox token and live relay admin token are both presented
- provisions managed user accounts under a specific admin owner mailbox
- exposes mailbox-authenticated account profile lookup so clients can read role and status from the server
- stores only hashed mailbox retrieval tokens server-side
- accepts authenticated X3DH prekey-bundle publication for mailbox owners
- serves public X3DH prekey-bundle fetches and consumes one-time prekeys on retrieval
- accepts signed device-record publication for mailbox owners
- exposes mailbox prekey inventory status and public device listings
- accepts ciphertext envelopes for known mailboxes
- enforces minimum, default, and maximum TTL values
- rejects oversized ciphertext
- reserves replay tokens in Redis to reject duplicate submissions
- rate-limits mailbox submission and retrieval paths in Redis
- retrieves ciphertext envelopes only when the correct mailbox token is presented
- soft-deletes messages and purges deleted or expired records in a background cleanup loop
- serves a browser-based live integration console from the root route

### 3. Browser-based secure chat and advanced drawer

The page served at the relay root now behaves like a simpler secure chat client first and an operator console second.

The main chat surface can:

- open a saved mailbox already known to this browser
- activate a mailbox from join or shared-link onboarding data
- activate an admin-provisioned mailbox from a join link that carries the issued mailbox ID and mailbox token
- bootstrap the current mailbox into an admin account only when the live relay admin token is presented
- land on a setup and login page first instead of auto-entering the last browser workspace
- show saved workspace credentials on the setup screen so operators can switch between admin and user accounts deliberately
- read mailbox role and status from the relay instead of trusting browser-local role state
- generate or import a shared 32-byte session secret
- export or import a lightweight share pack containing mailbox ID and session secret for manual out-of-band onboarding
- encrypt direct messages in the browser with AES-256-GCM before relay submission
- retrieve stored messages and decrypt them locally when the shared session secret matches
- publish encrypted private feed posts, including compressed picture posts, to saved contacts
- decrypt received private feed posts locally with the same shared contact secrets
- send encrypted mailbox-paired replies on private feed posts so authors and recipients can continue a thread without exposing plaintext to the relay
- keep mailbox tokens and shared secrets in local browser storage unless the operator copies them out

The advanced drawer can still:

- accept a pasted identity fixture from the core examples so prekeys and signed device records can be published from the browser console
- check prekey inventory status and list public device records for the current mailbox
- seed raw relay payload templates and submit opaque envelopes
- show request and response traces
- show current relay health

The page intentionally keeps the warning visible that the browser secure mode is not the final production protocol client. The current browser workflow provides real local encryption and now uses server-backed account roles, but the stronger X3DH prekey bootstrap, Double Ratchet advancement, and persistent ratchet lifecycle still need to be bound from the Rust core into real platform clients.

### 3.1 Account model and creation paths

The repository now has two distinct server-backed account creation paths.

Admin creation:

- create a mailbox first with `POST /v1/mailboxes` only when a valid relay admin token is supplied
- bootstrap that mailbox into an admin account with `POST /v1/admin/bootstrap-account`
- admin bootstrap requires both the live relay admin token from `RELAY_ADMIN_TOKEN` and the mailbox token proving ownership of the mailbox being promoted
- managed user mailboxes are not eligible for admin bootstrap, even if someone has the mailbox token
- after bootstrap, the mailbox is persisted as `role = admin` and `status = active`
- once the mailbox is already an admin account, the browser still requires the live relay admin token to be re-entered before privileged admin actions are unlocked for that browser session

User creation:

- a regular user does not create a standalone mailbox anonymously anymore
- an existing authenticated mailbox can create a new user mailbox branch with `POST /v1/mailboxes` by supplying authenticated origin headers
- an admin can also provision a managed user mailbox with `POST /v1/admin/provision-mailbox`
- managed users are stored as `role = user`, `status = provisioned`, and linked to the owning admin mailbox
- the provisioned user then activates that mailbox with the exact issued mailbox token
- the owning admin can later disable that user, re-enable them, or rotate their access token and issue a fresh join link
- the browser now keeps admin identity and admin session separate: the account can remain admin, but privileged admin actions stay locked until the live relay admin token is re-entered for that browser session

Browser workflow summary:

- the public user setup route now opens only saved workspaces or onboarding data that was shared with the user
- a mailbox can be promoted into an admin account only if the live relay admin token is supplied
- protected operator actions stay locked until the browser session explicitly enables `Protected Controls`
- `Create Managed Workspace` inside the protected panel is the in-product path for creating managed users
- `Share Workspace` now separates three distinct artifacts: a `Mailbox Card` for identity sharing, a persistent `Fresh Pairing Link` for starting a brand-new one-to-one secure relationship with an existing mailbox, and a `Pairing Reply Link` kept as a manual fallback if automatic sync is unavailable
- `Add Contact` is the import and manual-linking surface; it can consume mailbox cards, pairing links, pairing reply links, direct contact links, or generate a new relay mailbox plus onboarding pack when an existing user is creating the next branch for someone else
- the older direct contact link format still exists for workflows that intentionally carry an already chosen shared secret for one specific relationship, but it is no longer the default generic sharing action
- accepting a fresh pairing link now sends an encrypted acceptance back through the relay so the original sender normally links the contact automatically on the next sync without needing a copied reply link
- saved workspace state now preserves pending pairing invites across hard refresh, and workspace resume re-syncs account, contacts, and feed state from the relay
- if an older synced contact is missing its local secret, the browser now exposes `Repair Secure Link` to restore that browser-local linkage without touching relay-side contact records

Managed user lifecycle:

- an admin provisions the mailbox and gets a join pack containing mailbox ID, mailbox token, and suggested codename
- the managed user activates the account with that exact mailbox token
- the admin overview can reissue the join link, disable the user, re-enable the user, reset access back to `provisioned`, or delete the managed user entirely
- all of those management actions are scoped to the owning admin mailbox only

Contact relationships:

- mailbox contacts can now be stored on the relay with `POST /v1/mailboxes/{mailbox_id}/contacts`
- the relay stores contact mailbox linkage plus the chosen codename for that relationship
- browser-local shared secrets remain local and are not uploaded to the relay
- this lets the browser sync the contact list from the relay without turning the relay into a plaintext key escrow surface

Codename behavior:

- mailbox ID and mailbox token are server-bound identity material
- the suggested codename in a provisioned user pack is only a starting point
- the user may still choose a different codename during activation unless stricter alias policy is added later

### 4. Media and feed primitives in the core crate

The shared core now includes first-pass client-side media cryptography and descriptor types for the next phase of the build.

Current capabilities:

- media padding bucket selection for ciphertext blob sizing
- blob padding plan generation with fixed chunk sizing
- encrypted blob pointer descriptors for object-store retrieval metadata
- feed-post descriptor types that can reference encrypted blob pointers
- chunked XChaCha20-Poly1305 media encryption helpers
- ciphertext integrity verification during media decryption

These additions are now more than schema helpers, but they are still not full audited session integration or production mobile file pipelines.

### 5. Double Ratchet session state in the core crate

The shared core now also includes a working post-X3DH 1:1 ratchet state machine.

Current capabilities:

- bootstraps initiator and responder sessions from the X3DH bootstrap output
- derives sending and receiving chain keys from ratchet DH outputs
- emits serializable ratchet headers carrying the current ratchet public key, prior chain length, and message number
- encrypts and decrypts ratcheted session messages with XChaCha20-Poly1305
- binds external associated data to the ratcheted message ciphertext
- caches skipped message keys so a bounded amount of out-of-order delivery can be recovered
- serializes ratchet sessions into a stable persisted representation
- exposes a backend-agnostic session-store trait so future Android, iOS, and desktop clients can plug in encrypted local storage without changing protocol code
- includes a reference in-memory ratchet-session store and continuity tests proving a restored session can keep sending and receiving messages
- includes a feature-gated encrypted file-backed ratchet store module (`ratchet-store-fs`) as a production-oriented local adapter baseline

This is the right protocol layer to build on, but it still needs audited platform storage backends, replay policy, and production lifecycle handling before it should be trusted as a finished messenger protocol.

### 6. Media upload-intent planning in the relay

The relay now exposes an authenticated media planning endpoint for mailbox owners.

Current capabilities:

- validates media type, requested size, and optional SHA-256 content hash metadata
- chooses a padded blob size via the shared core logic
- records upload intents in PostgreSQL with expiry
- returns an S3-compatible presigned `PUT` request for the ciphertext blob upload
- records completed encrypted media manifests in PostgreSQL
- creates and resolves short-lived media access grants
- resolves grants into S3-compatible presigned requests for `download` or delegated `upload`
- returns the object-store bucket, object key, chunk size, upload endpoint, and required request headers
- keeps the relay blind to decryption keys and plaintext media contents

### 7. Testing and diagnostics infrastructure

The repository now includes a hardened local and CI testing path rather than ad hoc smoke checks.

Current capabilities:

- runs a preflight gate before full test execution to verify toolchain, environment, and dependency readiness
- rejects insecure placeholder admin token values when strict secret linting is enabled
- produces structured relay smoke-test telemetry with JSON artifacts and per-step event logs
- tracks retrieve and total smoke-test latency with optional SLO enforcement
- redacts token-like values from CI artifacts before upload
- publishes concise CI job summaries for smoke health and latency triage
- includes an incident runbook mapping common failures to exact diagnosis commands and likely fixes

These additions improve repeatability, reduce drift between local and CI behavior, and make failures easier to diagnose without weakening secret handling.

### 8. Desktop Tauri client reference implementation

The desktop client is no longer just a wrapper plan. It now includes a working Tauri development surface tied into the shared Rust protocol core.

Current capabilities:

- runs as a Tauri 2 application with a React and TypeScript frontend
- links the shared `comm-core` crate into the desktop Rust backend
- exposes Tauri commands for identity generation, prekey-bundle generation, X3DH bootstrap, ratcheted message encryption and decryption, and session status checks
- persists Double Ratchet sessions using the encrypted file-backed session store from the shared core
- demonstrates restart-safe session continuity through a live test UI that exercises both initiator and responder flows
- keeps session encryption logic and file I/O in Rust rather than the web layer

This desktop client is still a reference surface, not a finished end-user messenger, but it now proves the core protocol and persistence boundary can be bound into a real desktop runtime.

## Security Model

The repository currently enforces a few important boundaries.

### What the relay should know

- mailbox identifiers
- hashed mailbox retrieval tokens
- encrypted envelope bytes
- timestamps needed for retention and cleanup
- temporary replay and throttle counters

### What the relay should not know

- plaintext message content
- user-readable sender identity beyond optional opaque device hints
- decrypted key material
- long-term private identity keys

### What is still missing from the security model

- third-party review and hardening of the current 1:1 session bootstrap and Double Ratchet state
- audited group session management through MLS
- device registration and revocation flows
- authenticated update channels for clients
- endpoint hardening in the actual user-facing apps
- real streaming encryption and EXIF stripping in the client media pipeline

## Relay API Surface

### `GET /`

Returns the browser-based live test console.

### `GET /healthz`

Returns relay dependency health and current server time.

Example response:

```json
{
   "status": "ok",
   "server_time": "2026-03-21T18:51:48.366950Z",
   "postgres": "ok",
   "redis": "ok"
}
```

### `POST /v1/mailboxes`

Creates or activates a mailbox record and returns the server-backed account profile for that mailbox.

Request body:

```json
{
   "mailbox_id": "high_entropy_mailbox_id",
   "mailbox_token_b64": "base64_random_token",
   "codename": "optional-user-visible-alias"
}
```

Constraints:

- `mailbox_id` must be 16 to 128 characters
- `mailbox_id` may contain ASCII alphanumeric characters, `_`, and `-`
- `mailbox_token_b64` must decode to at least 32 random bytes

If the mailbox already exists and the token matches, the relay treats the request as a valid activation of that mailbox. If the mailbox exists and the token does not match, the relay rejects the request.

### `POST /v1/admin/bootstrap-account`

Promotes an existing mailbox into a real server-backed admin account.

Required headers:

- `x-admin-token`
- `x-mailbox-token`

Request body:

```json
{
   "mailbox_id": "existing_mailbox_uuid",
   "codename": "optional-admin-codename"
}
```

### `POST /v1/admin/provision-mailbox`

Creates a managed user mailbox underneath the authenticated admin account.

Required headers:

- `x-admin-token`
- `x-admin-mailbox-id`
- `x-mailbox-token`

The returned pack includes the mailbox UUID, mailbox token, and a suggested codename that can be wrapped into a join link.

In the browser UI, this is exposed through `Create Managed Workspace` in the protected controls panel. The `Add Contact` flow remains separate and is used for direct contact linking rather than managed-user provisioning.

### `GET /v1/mailboxes/{mailbox_id}/account`

Returns the server-backed account profile for the mailbox.

Required headers:

- `x-mailbox-token`

Response includes codename, role, status, and owner admin mailbox when the account is managed.

This endpoint is also what the browser uses as the source of truth for whether the current mailbox is an admin account or a normal user account.

### `GET /v1/mailboxes/{mailbox_id}/contacts`

Returns the relay-backed contact relationships for the authenticated mailbox.

Required headers:

- `x-mailbox-token`

The response includes the saved contact codename plus the contact mailbox's current server-backed role and status.

### `POST /v1/mailboxes/{mailbox_id}/contacts`

Creates or updates a relay-backed contact relationship for the authenticated mailbox.

Required headers:

- `x-mailbox-token`

Request body:

```json
{
   "contact_mailbox_id": "contact_mailbox_uuid",
   "codename": "optional-contact-codename"
}
```

The relay stores the mailbox linkage and codename metadata only. Shared secrets for browser-side AES-GCM direct messages remain browser-local.

### `DELETE /v1/mailboxes/{mailbox_id}/contacts/{contact_mailbox_id}`

Deletes a relay-backed contact relationship for the authenticated mailbox.

Required headers:

- `x-mailbox-token`

### `POST /v1/mailboxes/{mailbox_id}/feed/posts`

Creates an encrypted private feed post for the authenticated mailbox and distributes one opaque delivery per saved contact recipient.

Required headers:

- `x-mailbox-token`

Request body:

```json
{
   "post_id": "optional-uuid",
   "audience": "contacts",
   "reply_policy": "contacts_only",
   "author_ciphertext_b64": "base64_author_copy_ciphertext",
   "deliveries": [
      {
         "recipient_mailbox_id": "contact_mailbox_uuid",
         "ciphertext_b64": "base64_recipient_copy_ciphertext"
      }
   ],
   "ttl_seconds": 604800
}
```

The relay validates that every recipient is an active relay-backed contact for the posting mailbox, but it does not decrypt the post body.

### `GET /v1/mailboxes/{mailbox_id}/feed/posts`

Lists encrypted feed posts visible to the authenticated mailbox.

Required headers:

- `x-mailbox-token`

Each entry returns the author mailbox, author codename, ciphertext copy visible to this mailbox, creation time, expiry time, and whether the mailbox is seeing the post as `authored` or `received`.

Each entry now also includes valid reply targets for the authenticated mailbox plus the encrypted replies visible on that post.

### `POST /v1/mailboxes/{mailbox_id}/feed/posts/{post_id}/replies`

Creates an encrypted reply on a visible private feed post.

Required headers:

- `x-mailbox-token`

Request body:

```json
{
   "reply_id": "optional-uuid",
   "recipient_mailbox_id": "contact_mailbox_uuid",
   "author_ciphertext_b64": "base64_reply_ciphertext_for_the_reply_author",
   "recipient_ciphertext_b64": "base64_reply_ciphertext_for_the_reply_recipient",
   "ttl_seconds": 604800
}
```

Recipients replying to a post can only target the original post author. The post author can reply back to any active recipient from the original audience.

### `POST /v1/mailboxes/{mailbox_id}/messages`

Stores a ciphertext envelope for a mailbox.

Request body:

```json
{
   "ciphertext_b64": "base64_ciphertext",
   "sender_device_hint": "optional-device-id",
   "ttl_seconds": 604800,
   "message_id": "optional-uuid",
   "replay_token": "unique_submission_token"
}
```

Constraints:

- the mailbox must already exist
- ciphertext must be valid base64 and non-empty
- ciphertext size is capped by `RELAY_MAX_CIPHERTEXT_BYTES`
- `replay_token` must be unique inside its Redis replay window
- TTL must fall between configured minimum and maximum values

### `GET /v1/mailboxes/{mailbox_id}/messages`

Lists mailbox messages. Requires the header:

```text
x-mailbox-token: <base64-mailbox-token>
```

### `DELETE /v1/mailboxes/{mailbox_id}/messages/{message_id}`

Marks a message deleted. Also requires `x-mailbox-token`.

### `POST /v1/mailboxes/{mailbox_id}/prekeys`

Publishes or replaces the authenticated mailbox owner's X3DH prekey bundle.

Request body:

```json
{
   "identity_signing_key_b64": "base64_ed25519_public_key",
   "identity_exchange_key_b64": "base64_x25519_public_key",
   "signed_prekey_b64": "base64_x25519_signed_prekey",
   "signed_prekey_signature_b64": "base64_ed25519_signature",
   "signed_prekey_expires_at": "2026-03-28T20:11:19Z",
   "one_time_prekeys_b64": [
      "base64_x25519_one_time_prekey"
   ]
}
```

Constraints:

- requires `x-mailbox-token`
- verifies that the signed prekey was signed by the supplied Ed25519 identity key
- replaces any still-unconsumed one-time prekeys for that mailbox with the new batch
- defaults the signed prekey expiry to 7 days in the future if omitted

### `GET /v1/prekeys/{mailbox_id}`

Fetches the current public X3DH prekey bundle for a mailbox.

Response fields include:

- `identity_signing_key_b64`
- `identity_exchange_key_b64`
- `signed_prekey_b64`
- `signed_prekey_signature_b64`
- `signed_prekey_expires_at`
- `one_time_prekey_b64`

If an unused one-time prekey exists, the relay consumes one during the fetch so repeated requests return different one-time prekeys until the current batch is exhausted.

### `GET /v1/mailboxes/{mailbox_id}/prekeys/status`

Returns the mailbox owner's currently active signed-prekey expiry and the count of still-unconsumed one-time prekeys. Requires `x-mailbox-token`.

### `POST /v1/mailboxes/{mailbox_id}/devices`

Publishes or updates a signed device record for the mailbox owner. The relay verifies the device record signature against the identity signing key from the mailbox's active prekey bundle.

### `GET /v1/devices/{mailbox_id}`

Returns the public signed device records currently registered for the mailbox.

### `POST /v1/mailboxes/{mailbox_id}/media/upload-intents`

Creates an authenticated media upload intent for a mailbox owner.

Request body:

```json
{
   "media_type": "image/jpeg",
   "original_size_bytes": 842391,
   "chunk_size_bytes": 262144,
   "ttl_seconds": 900,
   "content_sha256_b64": "base64_sha256_digest"
}
```

Response fields include:

- `intent_id`
- `bucket`
- `object_key`
- `padded_size_bytes`
- `chunk_size_bytes`
- `upload_endpoint`
- `presigned_upload.method`
- `presigned_upload.url`
- `presigned_upload.headers`
- `expires_at`

### `POST /v1/mailboxes/{mailbox_id}/media/manifests`

Registers a completed encrypted media blob from a prior upload intent.

### `GET /v1/mailboxes/{mailbox_id}/media/manifests`

Lists registered encrypted media blobs for the mailbox owner.

### `POST /v1/mailboxes/{mailbox_id}/media/access-grants`

Creates a short-lived access grant token for a registered encrypted media object.

### `GET /v1/media/access-grants/{grant_token}`

Resolves a grant token to object metadata plus a presigned object-store request that can be executed directly against MinIO or another S3-compatible backend.

## Media and Private Posts

Media and feed-style posts are now part of the intended architecture, but not by making the relay a content server.

Current build status:

- pictures and large media are encrypted on the client and stored as padded ciphertext blobs in object storage
- the relay carries only the encrypted pointer envelopes that reference those blobs
- private feed posts remain encrypted client-side content types, not server-readable post records
- the relay now stores private feed post envelopes with one author-visible ciphertext copy plus one ciphertext delivery per active relay-backed contact recipient
- the relay now also stores mailbox-paired encrypted replies for private feed posts so authors can see replies from each recipient and each recipient can reply back to the author without recipient-to-recipient key sharing
- the browser now ships a private feed composer and timeline that encrypt text locally and upload feed photos through the relay media pipeline before embedding encrypted media grant metadata in the post payload
- the shared core now includes media pointer and feed-post descriptor types plus chunked media encryption helpers
- the relay now exposes upload-intent planning so clients can obtain object keys and padding targets without inventing their own conventions
- the relay now registers completed encrypted media manifests, issues short-lived media access grants, and resolves those grants into presigned object-store requests
- the relay now also serves same-origin encrypted object bytes through `GET /v1/media/access-grants/{grant_token}/content` so the browser can fetch encrypted media without object-store CORS coupling

The detailed design and current implementation status live in [docs/media-and-posts.md](docs/media-and-posts.md).

## Live Testing

Two live testing paths are available.

### Browser path

1. Start the relay and backing services.
2. Open `http://127.0.0.1:8081/`.
3. Generate a mailbox pair.
4. Create the mailbox.
5. Seed or type a demo payload.
6. Submit the envelope.
7. Retrieve the mailbox.
8. Delete one or more messages.

### Manual API path

See [docs/live-testing.md](docs/live-testing.md) for PowerShell examples covering health checks, mailbox creation, media upload intents, submission, retrieval, and deletion.

For a valid X3DH publish payload during development, run:

```powershell
cargo run -p comm-core --example generate_prekey_bundle
```

For a coherent prekey-plus-device fixture, run:

```powershell
cargo run -p comm-core --example generate_identity_fixture
```

For a local Double Ratchet exchange demo, run:

```powershell
cargo run -p comm-core --example bootstrap_double_ratchet
```

For an encrypted at-rest ratchet session store demo using the new session-store trait, run:

```powershell
cargo run -p comm-core --example encrypted_ratchet_store
```

For feature-gated file-backed store tests in the library module, run:

```powershell
cargo test -p comm-core --features ratchet-store-fs ratchet_store_fs
```

## Local Development Setup

### Prerequisites

- Rust toolchain through `rustup`
- Docker Desktop or equivalent local container runtime
- PowerShell or another shell capable of loading the environment variables

### Step-by-step startup for the whole system

Use this sequence on a fresh terminal in the repository root.

1. Verify or create your local env file.

   - The repository expects `.env` in the workspace root.
   - For the default Docker Compose stack, `POSTGRES_DSN` should match the local container baseline:

   ```text
   postgres://postgres:postgres@127.0.0.1:5432/simy
   ```

2. Start backing services.

   ```powershell
   docker compose up -d
   ```

3. Run a workspace compile check.

   ```powershell
   cargo check
   ```

4. Start the relay and wait for health.

   ```powershell
   powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/run_relay.ps1 -StartDependencies -WaitForHealth
   ```

5. Verify the live health endpoint.

   ```powershell
   Invoke-RestMethod -Uri http://127.0.0.1:8081/healthz
   ```

6. Optionally run the end-to-end smoke test.

   ```powershell
   powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/relay_smoke.ps1 -BaseUrl http://127.0.0.1:8081
   ```

7. Optionally run the broader automated test suite.

   ```powershell
   powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/test_all.ps1 -RunRelaySmoke -RelayHealthTimeoutSeconds 20
   ```

If step 4 fails with PostgreSQL authentication errors, check that `.env` matches the credentials declared in [docker-compose.yml](docker-compose.yml).

### One-command startup

For the normal local developer path, use the orchestration script below. It loads `.env`, runs preflight, starts Docker dependencies, launches the relay in the background, waits for `/healthz`, and prints the resulting health payload.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/start_all.ps1
```

Useful variants:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/start_all.ps1 -RunSmoke
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/start_all.ps1 -OpenUi
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/start_all.ps1 -StrictSecrets
```

What the command does:

- validates the local environment file and required variables
- optionally enforces non-placeholder admin-token policy with `-StrictSecrets`
- starts PostgreSQL, Redis, and MinIO through Docker Compose
- launches the relay in a background PowerShell process
- waits until `GET /healthz` returns `ok`
- prints the browser UI URL and health URL
- can open the browser UI automatically with `-OpenUi`
- optionally runs the relay smoke test

Relay stdout and stderr logs from the one-command path are written under `./artifacts/start_all/`.

### Browser UI for admin and user troubleshooting

The main browser UI is served from the relay root:

```text
http://127.0.0.1:8081/
```

The intentionally hidden protected entry is available at:

```text
http://127.0.0.1:8081/?access=protected
```

Use the root route for normal user troubleshooting. Use the protected route only when you need provisioning or other protected controls.

Recommended setup for testing both roles at once:

1. Start the system with:

   ```powershell
   powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/start_all.ps1 -OpenUi
   ```

2. Open the UI in two isolated browser contexts.

   - Use one normal browser window for the protected side.
   - Use one private or incognito window for the user side.

   This keeps browser-local tokens, mailbox IDs, and shared secrets separate.

   The root page now stays on the setup and login screen first. If the browser has seen prior workspaces, a `Saved Workspaces` panel appears there with mailbox IDs, mailbox tokens, roles, statuses, and buttons to reuse or copy credentials. Reloading the same tab resumes the active workspace instead of acting like a logout, restores pending pairing state, and re-syncs account, contact, message, and feed state from the relay, while a fresh visit still starts from setup.

3. In the protected window:

   - open `http://127.0.0.1:8081/?access=protected`
   - click `Create Secure Workspace`
   - promote that mailbox into an admin account if needed
   - provide the `RELAY_ADMIN_TOKEN` value from `.env`
   - when the session is locked again, use `Enable Protected Controls`
   - use `Create Managed Workspace` to provision a managed user mailbox

4. In the user window:

   - either click `Create Secure Workspace` for a standalone user mailbox
   - or activate the managed user mailbox using the join data issued by the admin flow

   If admin has already provisioned the mailbox in this browser before, the setup screen can also prefill the managed-user credentials from the `Saved Workspaces` panel.

   For person-to-person testing between two existing workspaces, use `Share Workspace` to copy a `Fresh Pairing Link` from one browser and open it in the other browser. Accepting that link adds the sender as a secure contact and sends an encrypted acceptance back through the relay so the original browser normally completes the two-way secure relationship automatically on its next sync.

   The generated `Pairing Reply Link` is still shown as a manual fallback if relay delivery is unavailable or either browser has not synced yet.

   For identity-only sharing, use the `Mailbox Card`. It identifies the mailbox but does not establish a shared secret by itself.

   For mailbox creation on behalf of someone else, `Add Contact` can still generate a new relay mailbox and onboarding pack for the other browser. That generated setup data includes the shared secret and ownership metadata needed for messages, photo posts, and replies to work immediately after activation.

5. Use the same UI to verify:

   - mailbox creation and activation
   - protected account bootstrap and protected-session locking behavior
   - managed-user provisioning
   - contact linking
   - encrypted direct-message send and retrieve flows
   - feed-post and media flows when needed

If you are testing against older browser-local state from before the secure-link fix, use `Clean Broken Local State` to drop incomplete local contact snapshots while keeping valid saved workspaces, or `Reset All Local Data` to wipe all browser-local workspaces and start from a clean flow.

If you only need the UI URL without auto-opening a browser, run `start_all.ps1` without `-OpenUi`; it now prints both the UI and health endpoints explicitly.

### Account switching and credentials visibility

The browser UI now keeps account selection explicit.

- Visiting `http://127.0.0.1:8081/` opens the setup and login screen first.
- The page does not automatically drop a fresh visit into the last restored workspace anymore.
- Reloading the same tab resumes the active workspace so refresh behaves like refresh, not logout.
- Saved mailbox credentials appear in a `Saved Workspaces` section on the setup page.
- Each saved workspace entry shows:
   - codename
   - mailbox ID
   - mailbox token
   - role
   - status
- Each saved workspace entry lets you:
   - prefill the setup form with `Use Credentials`
   - copy the full credential pack with `Copy Credentials`
   - remove stale local entries with `Forget`
- The same setup surface now also exposes `Clean Broken Local State` and `Reset All Local Data` for clearing stale browser-local linkage after earlier test runs.

This is intended to make admin and managed-user troubleshooting practical without guessing which browser tab currently owns which mailbox.

### Start backing services

```powershell
docker compose up -d
```

This brings up:

- PostgreSQL on `127.0.0.1:5432`
- Redis on `127.0.0.1:6379`
- MinIO API on `127.0.0.1:9000`
- MinIO console on `127.0.0.1:9001`

### Build the workspace

```powershell
cargo check
```

### Run the relay

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/run_relay.ps1 -StartDependencies -WaitForHealth
```

### Stop local services

```powershell
docker compose down
```

## Runtime Configuration

`RELAY_ADMIN_TOKEN` is mandatory. Do not ship the placeholder from [/.env.example](.env.example); replace it with a long random secret before running the relay anywhere outside throwaway local development.

Runtime values come from [.env.example](.env.example) and the local [.env](.env).

### Relay binding

- `RELAY_BIND_ADDR`: address and port to bind the HTTP server

### TTL policy

- `RELAY_MIN_TTL_SECONDS`: minimum allowed envelope TTL
- `RELAY_DEFAULT_TTL_SECONDS`: default TTL when a sender does not specify one
- `RELAY_MAX_TTL_SECONDS`: maximum allowed envelope TTL

### Payload policy

- `RELAY_MAX_CIPHERTEXT_BYTES`: maximum allowed decoded ciphertext size in bytes

### Replay and cleanup

- `RELAY_REPLAY_TTL_MARGIN_SECONDS`: extra time to keep replay tokens after message expiry window
- `RELAY_CLEANUP_INTERVAL_SECONDS`: interval for purging deleted or expired messages

### Abuse controls

- `RELAY_SUBMIT_RATE_LIMIT_PER_MINUTE`: submit operations per mailbox per minute
- `RELAY_RETRIEVE_RATE_LIMIT_PER_MINUTE`: retrieval or deletion operations per mailbox per minute

### Dependencies

- `POSTGRES_DSN`: PostgreSQL connection string
- `REDIS_URL`: Redis connection string
- `MEDIA_OBJECT_STORE_ENDPOINT`: object storage API endpoint
- `MEDIA_OBJECT_STORE_CONSOLE`: object storage admin console URL
- `MEDIA_OBJECT_STORE_BUCKET`: object storage bucket name for encrypted media blobs
- `MEDIA_OBJECT_STORE_REGION`: object storage region label
- `MEDIA_OBJECT_STORE_ACCESS_KEY_ID`: S3-compatible access key used by the relay to mint presigned requests
- `MEDIA_OBJECT_STORE_SECRET_ACCESS_KEY`: S3-compatible secret used by the relay to mint presigned requests
- `MEDIA_UPLOAD_INTENT_TTL_SECONDS`: default TTL for upload intents and their presigned upload requests
- `MEDIA_ACCESS_GRANT_TTL_SECONDS`: default TTL for issued media access grants
- `MEDIA_DEFAULT_CHUNK_SIZE_BYTES`: default ciphertext chunk size used when the client omits one
- `MEDIA_MAX_ORIGINAL_SIZE_BYTES`: maximum accepted plaintext-size hint for a media upload intent
- `MINIO_ROOT_USER`: local MinIO admin user for development
- `MINIO_ROOT_PASSWORD`: local MinIO admin password for development

## Data Model and Storage

### `relay_mailboxes`

Stored fields:

- `mailbox_id`
- `access_token_hash`
- `created_at`

Purpose:

- identifies a retrievable mailbox
- stores only the hash of the retrieval token, not the token itself

### `relay_messages`

Stored fields:

- `message_id`
- `mailbox_id`
- `sender_device_hint`
- `ciphertext`
- `received_at`
- `expires_at`
- `deleted_at`

Purpose:

- stores ciphertext envelopes durably
- supports ordered retrieval by receipt time
- supports cleanup of expired and deleted messages

### `media_upload_intents`

Stored fields:

- `intent_id`
- `mailbox_id`
- `bucket`
- `object_key`
- `media_type`
- `original_size_bytes`
- `padded_size_bytes`
- `chunk_size_bytes`
- `content_sha256_b64`
- `created_at`
- `expires_at`

Purpose:

- records planned encrypted media uploads before the object is registered as completed

### `media_objects`

Stored fields:

- `object_id`
- `mailbox_id`
- `intent_id`
- `bucket`
- `object_key`
- `media_type`
- `original_size_bytes`
- `padded_size_bytes`
- `chunk_size_bytes`
- `content_sha256_b64`
- `created_at`
- `upload_completed_at`

Purpose:

- records completed encrypted media blobs available for later access grants and pointer messages

### `media_access_grants`

Stored fields:

- `grant_id`
- `object_id`
- `mailbox_id`
- `grant_token_hash`
- `operation`
- `created_at`
- `expires_at`
- `redeemed_at`

Purpose:

- issues short-lived delegated access metadata without exposing long-lived object references directly

### `relay_prekey_bundles`

Stored fields:

- `mailbox_id`
- `identity_signing_key`
- `identity_exchange_key`
- `signed_prekey`
- `signed_prekey_signature`
- `signed_prekey_created_at`
- `signed_prekey_expires_at`
- `updated_at`

Purpose:

- stores the active X3DH signed prekey bundle for asynchronous first-contact setup

### `relay_one_time_prekeys`

Stored fields:

- `prekey_id`
- `mailbox_id`
- `public_key`
- `created_at`
- `consumed_at`

Purpose:

- stores single-use X3DH one-time prekeys that are consumed during public bundle fetches

### `relay_device_records`

Stored fields:

- `mailbox_id`
- `device_id`
- `device_label`
- `device_signing_key`
- `device_exchange_key`
- `created_at`
- `revoked_at`
- `signature`
- `updated_at`

Purpose:

- stores signed public device records bound to the mailbox identity for trust and device lifecycle discovery

## Operational Behavior

### Startup behavior

On startup the relay:

1. loads configuration from environment variables
2. connects to PostgreSQL
3. runs SQL migrations
4. connects to Redis and pings it
5. builds an S3-compatible object-store client for presigning upload and access requests
6. spawns the cleanup loop
7. starts serving HTTP routes

### Cleanup behavior

The relay periodically deletes records that are expired or already marked deleted. This keeps mailbox retrieval bounded and aligns runtime behavior with the retention policy documented in [docs/data-retention.md](docs/data-retention.md).

### Health behavior

The relay checks both PostgreSQL and Redis in [services/relay/src/main.rs](services/relay/src/main.rs). If either dependency is unavailable, the health endpoint returns degraded service status.

## Documentation Map

Start here depending on the question you are asking.

- Architecture and trust boundaries: [docs/architecture.md](docs/architecture.md)
- Threat model and attacker assumptions: [docs/threat-model.md](docs/threat-model.md)
- Key lifecycle and verification posture: [docs/key-management.md](docs/key-management.md)
- Retention rules: [docs/data-retention.md](docs/data-retention.md)
- Security test expectations: [docs/security-test-plan.md](docs/security-test-plan.md)
- Production readiness gates: [docs/production-readiness.md](docs/production-readiness.md)
- Production readiness brief: [docs/production-readiness-brief.md](docs/production-readiness-brief.md)
- Frontend showcase URLs: [docs/frontend-showcase-urls.md](docs/frontend-showcase-urls.md)
- Relay semantics: [docs/relay-api.md](docs/relay-api.md)
- Live testing: [docs/live-testing.md](docs/live-testing.md)
- Deployment baseline: [docs/deployment.md](docs/deployment.md)
- Push handling rules: [docs/push-notifications.md](docs/push-notifications.md)
- Media and private posts: [docs/media-and-posts.md](docs/media-and-posts.md)
- Testing quickstart and CI paths: [docs/testing-quickstart.md](docs/testing-quickstart.md)
- Testing incident runbook: [docs/testing-runbook.md](docs/testing-runbook.md)
- Desktop client status and session-persistence details: [clients/desktop/README.md](clients/desktop/README.md)
- Build continuation plan: [docs/next-build-plan.md](docs/next-build-plan.md)

## Current Gaps

This is the part that still needs real engineering work before the system can be called a secure communications application instead of a hardened relay foundation.

Missing or incomplete areas:

- audit and hardening of the implemented 1:1 X3DH plus Double Ratchet session core
- audited MLS integration for groups
- device enrollment, trust, and revocation flows
- production-grade relay wiring for the new desktop ratchet client
- platform bindings from the Rust core into Kotlin and Swift
- actual encrypted local stores on Android and iOS clients that implement the ratchet-session persistence boundary
- push token management and wake-hint delivery code
- production-grade abuse controls at the edge
- signing and update infrastructure for user-facing clients
- durable client-side ratchet state storage backends, replay policy, and session recovery logic across all platform apps
- client-side media transfer automation around the presigned object-store flow
- stronger object-store lifecycle controls, retention enforcement, and production credential management

## Recommended Next Steps

The strongest next build order is:

1. wire the desktop Tauri client to the relay for mailbox operations, prekey publication, encrypted message transport, and contact flows
2. bind the ratchet-session storage interface to encrypted local stores in the Android and iOS clients and prove restart-safe session continuity end to end
3. define device records, trust transitions, encrypted content envelopes, and revocation handling
4. scaffold the Android client around the Rust core
5. scaffold the iOS client around the Rust core
6. automate client-side media transfer around the existing presigned object-store flow
7. add signed builds, update verification, and release policy checks

## Current Live Status on This Machine

At the time of the last verification in this workspace:

- Rust toolchain is installed
- Docker Desktop is installed and working
- PostgreSQL, Redis, and MinIO are running through Docker Compose
- `cargo check` succeeds
- `cargo test -p comm-core` succeeds
- `cargo test -p relay` succeeds
- `powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/test_all.ps1 -RunRelaySmoke -RelayHealthTimeoutSeconds 20` succeeds
- relay smoke latency checks pass locally with structured JSON output
- encrypted media upload, manifest registration, access-grant resolution, and same-origin download round-trip succeed locally
- the desktop Tauri client launches in development mode and exposes the ratchet session-persistence test surface
- the relay starts successfully
- `GET /healthz` returns `ok`
- MinIO health returns `200`
- `cargo run -p comm-core --example bootstrap_double_ratchet` succeeds
- `POST /v1/mailboxes/{mailbox_id}/media/upload-intents` returns valid upload plans
- upload intents now include presigned object-store `PUT` requests
- media manifest registration and access-grant resolution work end to end
- access-grant resolution now returns presigned object-store requests for delegated media access
- X3DH prekey-bundle publish and fetch work live, including one-time-prekey consumption across repeated fetches
- the core crate now implements X3DH bootstrap envelopes plus a working Double Ratchet message flow
- the relay now supports signed device-record publication, public device listing, and authenticated prekey inventory status
- the root route now serves a live test console instead of a `404`
