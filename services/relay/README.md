# Relay Service

This service stores only encrypted message envelopes. It does not perform cryptographic decryption and should never receive plaintext content.

## Capabilities

- PostgreSQL-backed mailbox and message persistence
- Redis-backed replay token reservation
- Redis-backed coarse submit and retrieval throttling
- Mailbox provisioning with client-generated retrieval secret
- Server-backed account profiles with `admin` and `user` roles
- Dedicated admin bootstrap path for promoting a mailbox into an admin account
- Managed-user provisioning under a specific admin owner mailbox
- Managed-user mailboxes cannot be promoted into admin accounts through the bootstrap path
- Managed-user disable, re-enable, and access-reset controls for authenticated admins
- Browser admin sessions must be explicitly unlocked with the live relay admin token even after the mailbox itself is already an admin account
- Relay-backed mailbox contact relationships with mailbox-scoped auth
- Relay-backed private feed post distribution with mailbox-visible encrypted copies
- Relay-backed private feed replies scoped between the post author and one recipient mailbox
- Same-origin encrypted media content retrieval through media access grants
- TTL enforcement and periodic cleanup of expired or deleted messages
- Media upload-intent, manifest, and access-grant workflows
- S3-compatible presigned `PUT` requests for ciphertext media uploads
- S3-compatible presigned requests returned when resolving media access grants
- Authenticated X3DH prekey-bundle publication
- Public X3DH prekey-bundle fetch with one-time-prekey consumption
- Authenticated signed device-record publication
- Public signed device-record listing
- Authenticated prekey inventory status checks
- Browser-based live integration console served from `/`

## Environment

Use the variables in the workspace [/.env.example](../../.env.example).

`RELAY_ADMIN_TOKEN` is required. Replace the placeholder example value with a long random secret before starting the relay.

Media presigning depends on these object-store settings:

- `MEDIA_OBJECT_STORE_ENDPOINT`
- `MEDIA_OBJECT_STORE_BUCKET`
- `MEDIA_OBJECT_STORE_REGION`
- `MEDIA_OBJECT_STORE_ACCESS_KEY_ID`
- `MEDIA_OBJECT_STORE_SECRET_ACCESS_KEY`

## Endpoints

### `GET /`

Serves the live browser test console for local integration checks.

### `POST /v1/mailboxes`

Creates or activates a mailbox record and returns the server-backed account profile.

Request body:

```json
{
  "mailbox_id": "recipient_mailbox_identifier",
  "mailbox_token_b64": "client_generated_random_token_base64",
  "codename": "optional-user-visible-alias"
}
```

If the mailbox already exists and the token matches, the relay treats the request as a valid activation of that mailbox.

### `GET /v1/mailboxes/{mailbox_id}/account`

Returns the mailbox account profile. Requires `x-mailbox-token`.

This is the server truth for:

- codename
- role
- status
- owner admin mailbox for managed users

### `GET /v1/mailboxes/{mailbox_id}/contacts`

Returns the authenticated mailbox's relay-backed contacts. Requires `x-mailbox-token`.

Each contact entry includes:

- `contact_mailbox_id`
- saved `codename`
- the contact mailbox's current server-backed `role`
- the contact mailbox's current server-backed `status`

### `POST /v1/mailboxes/{mailbox_id}/contacts`

Creates or updates a relay-backed contact relationship for the authenticated mailbox.

Required header:

- `x-mailbox-token`

Request body:

```json
{
  "contact_mailbox_id": "existing_mailbox_uuid",
  "codename": "optional-contact-codename"
}
```

The relay stores only the relationship metadata and chosen codename. Browser-local shared secrets for direct-message encryption are not uploaded.

### `DELETE /v1/mailboxes/{mailbox_id}/contacts/{contact_mailbox_id}`

Deletes a relay-backed contact relationship for the authenticated mailbox. Requires `x-mailbox-token`.

### `POST /v1/mailboxes/{mailbox_id}/feed/posts`

Creates an encrypted private feed post for the authenticated mailbox.

Required header:

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
      "recipient_mailbox_id": "existing_contact_mailbox_uuid",
      "ciphertext_b64": "base64_recipient_copy_ciphertext"
    }
  ],
  "ttl_seconds": 604800
}
```

Every listed recipient must already be an active relay-backed contact for the posting mailbox.

### `GET /v1/mailboxes/{mailbox_id}/feed/posts`

Returns the encrypted private feed posts visible to the authenticated mailbox. Requires `x-mailbox-token`.

Each entry includes:

- `post_id`
- `author_mailbox_id`
- `author_codename`
- mailbox-visible `ciphertext_b64`
- `created_at`
- `expires_at`
- `visibility` set to `authored` or `received`
- `reply_targets` for the authenticated mailbox
- `replies` containing the encrypted replies visible to that mailbox on the post

### `POST /v1/mailboxes/{mailbox_id}/feed/posts/{post_id}/replies`

Creates an encrypted reply on a private feed post visible to the authenticated mailbox. Requires `x-mailbox-token`.

Request body:

```json
{
  "reply_id": "optional-uuid",
  "recipient_mailbox_id": "existing_contact_mailbox_uuid",
  "author_ciphertext_b64": "base64_reply_copy_for_the_authoring_mailbox",
  "recipient_ciphertext_b64": "base64_reply_copy_for_the_target_mailbox",
  "ttl_seconds": 604800
}
```

If the authenticated mailbox is not the original post author, the reply target is fixed to the original post author. If the authenticated mailbox is the original post author, the target must be an active recipient from the original post audience.

### `GET /v1/media/access-grants/{grant_token}/content`

Fetches the encrypted media object bytes referenced by an existing download access grant.

This is intended for browser clients that need a same-origin path for encrypted media retrieval after a grant token has been embedded inside an encrypted private post payload.

### `POST /v1/admin/bootstrap-account`

Promotes an existing mailbox into an admin account.

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

This is intentionally separate from normal user creation.

Managed-user mailboxes are not allowed through this promotion path.

### `POST /v1/admin/provision-mailbox`

Creates a managed user mailbox under the authenticated admin mailbox.

Required headers:

- `x-admin-token`
- `x-admin-mailbox-id`
- `x-mailbox-token`

Response includes:

- `mailbox_id`
- `mailbox_token_b64`
- `suggested_codename`
- `created_at`

The provisioned account is stored as `role = user`, `status = provisioned`, and linked to the admin owner mailbox until the user activates it.

This is the intended path for admin-created users. The browser contact flow is intentionally separate and should only be used to link existing mailboxes.

### `GET /v1/admin/overview`

Returns the authenticated admin account plus the managed user accounts it owns.

Required headers:

- `x-admin-token`
- `x-admin-mailbox-id`
- `x-mailbox-token`

This endpoint is also used by the browser to unlock and validate a privileged admin session after the mailbox is already an admin account.

### `POST /v1/admin/users/{mailbox_id}/status`

Updates a managed user's server-side status.

Required headers:

- `x-admin-token`
- `x-admin-mailbox-id`
- `x-mailbox-token`

Request body:

```json
{
  "status": "active"
}
```

Allowed statuses are `active`, `disabled`, and `provisioned`.

### `POST /v1/admin/users/{mailbox_id}/reset-access`

Rotates the managed user's mailbox token and returns a fresh provisioning pack.

Required headers:

- `x-admin-token`
- `x-admin-mailbox-id`
- `x-mailbox-token`

The reset also moves the managed user back to `status = provisioned` until they activate the new mailbox token.

### `DELETE /v1/admin/users/{mailbox_id}`

Deletes a managed user mailbox and all relay-side state that cascades from that mailbox.

Required headers:

- `x-admin-token`
- `x-admin-mailbox-id`
- `x-mailbox-token`

Only the owning admin mailbox may delete that managed user.

### `POST /v1/mailboxes/{mailbox_id}/messages`

Stores an encrypted envelope for a mailbox.

Request body:

```json
{
  "ciphertext_b64": "base64_ciphertext",
  "sender_device_hint": "optional-device-id",
  "ttl_seconds": 604800,
  "message_id": "8f845ee9-b2d4-4f72-bcb7-0fc3eb0047ad",
  "replay_token": "unique_per_submission_token"
}
```

### `GET /v1/mailboxes/{mailbox_id}/messages`

Retrieves encrypted envelopes. Requires `x-mailbox-token` containing the same base64 token used when provisioning the mailbox.

### `DELETE /v1/mailboxes/{mailbox_id}/messages/{message_id}`

Marks a message deleted. Requires `x-mailbox-token`.

### `POST /v1/mailboxes/{mailbox_id}/prekeys`

Publishes or replaces the mailbox owner's X3DH prekey bundle. Requires `x-mailbox-token`.

### `GET /v1/prekeys/{mailbox_id}`

Returns the mailbox's current public X3DH prekey bundle and consumes one one-time prekey if one is available.

### `GET /v1/mailboxes/{mailbox_id}/prekeys/status`

Returns the current signed-prekey expiry and remaining one-time-prekey count for the mailbox owner. Requires `x-mailbox-token`.

### `POST /v1/mailboxes/{mailbox_id}/devices`

Publishes or updates a signed device record for the mailbox owner. Requires `x-mailbox-token`.

### `GET /v1/devices/{mailbox_id}`

Returns the mailbox's public signed device records.

### `POST /v1/mailboxes/{mailbox_id}/media/upload-intents`

Creates a media upload intent for a mailbox owner and returns a presigned upload request with `method`, `url`, and required `headers`.

### `POST /v1/mailboxes/{mailbox_id}/media/manifests`

Registers a completed encrypted media object.

### `GET /v1/mailboxes/{mailbox_id}/media/manifests`

Lists registered media objects for the mailbox owner.

### `POST /v1/mailboxes/{mailbox_id}/media/access-grants`

Creates a short-lived access grant token for a registered media object.

### `GET /v1/media/access-grants/{grant_token}`

Resolves a grant token into object metadata plus a presigned object-store request for `download` or delegated `upload`.

## Operational Notes

- The mailbox token must be generated client-side and kept secret.
- The relay stores only a SHA-256 hash of the mailbox token, scoped by mailbox ID.
- Admin authority is now mailbox-bound and server-backed. The browser no longer gets to decide its own role.
- Submission is unauthenticated by design so other users can deliver encrypted content, but mailbox enumeration risk must be controlled by using high-entropy mailbox IDs.
- Push notifications should remain empty wake hints; delivery polling happens over the app-managed TLS connection.
- Media transfers are still ciphertext-only from the relay's perspective; the presigned request flow delegates transport without exposing decryption keys.
- The relay verifies signed prekey signatures before accepting a published bundle, but it still never sees any private key material.
- The root console is for relay testing only and does not represent the production secure client UX.
