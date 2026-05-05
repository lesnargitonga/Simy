# Live Testing Guide

## Purpose

This guide explains how to exercise the running relay locally through a browser and through direct HTTP calls.

## Browser Secure Chat

When the relay is running, open:

- `http://127.0.0.1:8081/`

The root route now serves a first-party secure chat client with an advanced relay drawer.

The main chat surface can:

- open a saved mailbox already known to this browser
- activate a mailbox from a shared onboarding link or a managed-user join link
- activate a managed user mailbox from an admin-issued join link
- bootstrap the current mailbox into an admin account with the live relay admin token
- keep admin identity separate from the unlocked admin browser session by re-entering the live relay admin token when privileged admin actions are needed
- read mailbox role and status from `GET /v1/mailboxes/{mailbox_id}/account`
- sync relay-backed contacts from `GET /v1/mailboxes/{mailbox_id}/contacts`
- create or remove relay-backed contacts while keeping the shared secret browser-local
- generate or import contact artifacts such as mailbox cards, pairing links, pairing reply links, and direct contact links
- encrypt direct messages in the browser with AES-256-GCM before relay submission
- retrieve mailbox messages and decrypt matching secure envelopes locally
- publish encrypted private feed posts to saved contacts
- upload encrypted feed photos through the relay media pipeline, then decrypt private feed posts and media locally in the browser
- send encrypted replies on private feed posts between the post author and each recipient mailbox

Current browser account flow:

- use the public route to open a saved workspace or join a workspace that was shared with you
- if you need a fresh root mailbox, open the hidden protected route and use the live relay admin token to create and bootstrap it
- after that, use `Unlock Admin Session` whenever the browser needs to perform privileged admin actions again
- use `Create Managed Workspace` in the protected panel to provision managed users
- use `Add Contact` to import existing contact artifacts or to create the next relay mailbox branch for someone else from an already-open workspace

The advanced drawer can:

- accept a pasted `generate_identity_fixture` JSON payload and publish prekeys and signed device records
- query current prekey inventory status and list public device records
- submit raw demo payloads as base64 relay ciphertext
- retrieve messages with the `x-mailbox-token` header
- delete individual messages
- display request and response traces
- display current relay health

## Important Limitation

The browser page now has a real encrypted messaging mode, but it is still not the final production messenger client.

What the browser page now does provide:

- end-to-end confidentiality for secure-mode direct messages by encrypting and decrypting in the browser with browser-local relationship secrets established through pairing or managed-user onboarding
- relay blindness to plaintext for those secure-mode messages

What it still does not provide:

- browser integration of the Rust X3DH prekey bootstrap
- browser integration of the Rust Double Ratchet session state
- audited device trust UX, ratchet persistence, and recovery policy

## User-Side End-to-End Verification

This is the minimum browser test that must keep working when client-facing logic changes.

### Goal

Prove that two user workspaces can:

- create or open their own mailboxes
- establish a secure relationship without a manual logic dead end
- sync that relationship on both sides
- exchange direct encrypted messages successfully
- recover from refresh and continue using the same relationship state

### Recommended setup

Use two isolated browser contexts:

- User A: normal browser window
- User B: private or incognito window

This keeps mailbox tokens, saved workspaces, and browser-local relationship secrets separate.

### Two-user pairing flow

1. Open User A and User B at `http://127.0.0.1:8081/`.
2. Open User A from a saved mailbox or a user-shared onboarding link.
3. Open User B from a saved mailbox or a user-shared onboarding link created from an existing branch.
4. In User A, click `Share Workspace` and copy the `Fresh Pairing Link`.
5. Open that pairing link in User B.
6. Accept the pairing in User B.

Expected result:

- User B adds User A as a secure contact immediately.
- User B shows a `Pairing Reply Link`, but this is fallback only.
- User B sends an encrypted pairing acceptance back through the relay.
- User A completes the secure contact automatically on the next relay sync without requiring the reply link under normal conditions.

7. Wait for User A to poll, or trigger a manual refresh path if needed.

Expected result:

- User A now shows User B as a secure contact.
- Neither side is left in a half-paired state.
- No broken-contact warning appears for the newly paired relationship.

### Direct-message verification

1. In User A, send a secure direct message to User B.
2. In User B, verify the message arrives and decrypts successfully.
3. In User B, send a reply.
4. In User A, verify the reply arrives and decrypts successfully.

Expected result:

- messages decrypt on both sides
- relay only stores ciphertext envelopes
- the contact remains usable after message retrieval and deletion

### Refresh and resume verification

1. Hard refresh User A.
2. Hard refresh User B.
3. Let both workspaces resume from saved state.

Expected result:

- the active workspace resumes in the same tab
- pending pairing state, if any, is preserved
- account, contact, message, and feed state re-sync from the relay
- the existing secure contact does not become broken solely because of refresh

4. After refresh, send another direct message in each direction.

Expected result:

- both sides still decrypt successfully
- no unexpected secret rotation occurs for the existing relationship

### Recovery verification

If the relationship appears as a synced contact without a browser-local secret:

1. Use `Repair Secure Link` for that contact.
2. Re-attach the correct link or secret artifact.
3. Re-test direct messaging.

Expected result:

- the contact leaves the broken-contact panel
- direct messaging works again
- relay-side contact metadata remains intact

### Managed-user verification

When testing admin-to-managed-user flows, also verify:

- the admin provisions the mailbox through `Create Managed Workspace`
- the managed user activates with the issued join data
- the owner and managed user become usable secure contacts with the shared relationship state carried by the onboarding flow
- they can exchange direct encrypted messages end to end after activation

### Failure conditions that block client-facing work

Treat any of these as a release blocker for browser-side changes:

- one side requires a manual fallback link for normal pairing when relay sync is available
- a hard refresh causes a previously valid contact to lose its secret unexpectedly
- a contact exists on one side only after pairing completes
- messages arrive but cannot be decrypted by the intended recipient
- broken-contact recovery is required for a newly established normal flow

Production clients must:

- encrypt envelopes on-device
- establish sessions with the shared Rust X3DH and Double Ratchet core rather than a manually exchanged shared secret
- generate replay tokens per submission
- generate mailbox identifiers and retrieval tokens from secure randomness
- keep mailbox tokens secret
- publish signed X3DH prekey bundles before expecting asynchronous first-contact messages

## Media Upload Intent Testing

The root browser console can now create media upload intents in addition to message envelopes. Use the `Create Media Upload Intent` panel to obtain:

- bucket name
- object key
- padded blob size
- chunk size
- upload endpoint
- presigned upload method
- presigned upload URL
- presigned upload headers

That result can then be copied into the seeded `media_pointer` or `feed_post` payload templates.

The same browser console can now also:

- register a completed media manifest from the last upload intent
- create a short-lived access grant for the registered media object
- resolve that grant token through the relay into a presigned object-store request
- fetch encrypted media bytes through the same-origin relay path at `GET /v1/media/access-grants/{grant_token}/content`

## Manual API Testing

### 1. Check health

```powershell
Invoke-RestMethod -Uri http://127.0.0.1:8081/healthz | ConvertTo-Json -Depth 4
```

### 2. Create a protected admin root mailbox

```powershell
$AdminToken = (Get-Content .env | Where-Object { $_ -match '^RELAY_ADMIN_TOKEN=' } | Select-Object -First 1).Split('=')[1]
$MailboxId = [guid]::NewGuid().ToString()
$MailboxToken = [Convert]::ToBase64String((1..32 | ForEach-Object { Get-Random -Minimum 0 -Maximum 256 }))

Invoke-RestMethod -Uri http://127.0.0.1:8081/v1/mailboxes -Method Post -Headers @{ 'x-admin-token' = $AdminToken } -ContentType 'application/json' -Body (@{
  mailbox_id = $MailboxId
  mailbox_token_b64 = $MailboxToken
  codename = 'Root-Admin'
} | ConvertTo-Json)

Invoke-RestMethod -Uri http://127.0.0.1:8081/v1/admin/bootstrap-account -Method Post -Headers @{ 'x-admin-token' = $AdminToken; 'x-mailbox-token' = $MailboxToken } -ContentType 'application/json' -Body (@{
  mailbox_id = $MailboxId
  mailbox_token_b64 = $MailboxToken
  codename = 'Root-Admin'
} | ConvertTo-Json)
```

### 3. Fetch the mailbox account profile

```powershell
Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/mailboxes/$MailboxId/account" -Headers @{ 'x-mailbox-token' = $MailboxToken } | ConvertTo-Json -Depth 6
```

### 3a. Create a branch mailbox and then a relay-backed contact relationship

```powershell
$SecondMailboxId = [guid]::NewGuid().ToString()
$SecondMailboxToken = [Convert]::ToBase64String((1..32 | ForEach-Object { Get-Random -Minimum 0 -Maximum 256 }))

Invoke-RestMethod -Uri http://127.0.0.1:8081/v1/mailboxes -Method Post -Headers @{ 'x-origin-mailbox-id' = $MailboxId; 'x-origin-mailbox-token' = $MailboxToken } -ContentType 'application/json' -Body (@{
  mailbox_id = $SecondMailboxId
  mailbox_token_b64 = $SecondMailboxToken
  codename = 'RelayFriend-01'
} | ConvertTo-Json) | Out-Null

Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/mailboxes/$MailboxId/contacts" -Method Post -ContentType 'application/json' -Headers @{ 'x-mailbox-token' = $MailboxToken } -Body (@{
  contact_mailbox_id = $SecondMailboxId
  codename = 'RelayFriend-01'
} | ConvertTo-Json) | ConvertTo-Json -Depth 6
```

### 3b. List relay-backed contacts

```powershell
Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/mailboxes/$MailboxId/contacts" -Headers @{ 'x-mailbox-token' = $MailboxToken } | ConvertTo-Json -Depth 6
```

### 3c. Create and list an encrypted private feed post

```powershell
$AuthorCiphertext = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes('{"iv":"author","ciphertext":"opaque"}'))
$RecipientCiphertext = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes('{"iv":"contact","ciphertext":"opaque"}'))

Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/mailboxes/$MailboxId/feed/posts" -Method Post -ContentType 'application/json' -Headers @{ 'x-mailbox-token' = $MailboxToken } -Body (@{
  audience = 'contacts'
  reply_policy = 'contacts_only'
  author_ciphertext_b64 = $AuthorCiphertext
  deliveries = @(@{
    recipient_mailbox_id = $SecondMailboxId
    ciphertext_b64 = $RecipientCiphertext
  })
  ttl_seconds = 3600
} | ConvertTo-Json -Depth 6) | ConvertTo-Json -Depth 6

Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/mailboxes/$MailboxId/feed/posts" -Headers @{ 'x-mailbox-token' = $MailboxToken } | ConvertTo-Json -Depth 8
Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/mailboxes/$SecondMailboxId/feed/posts" -Headers @{ 'x-mailbox-token' = $SecondMailboxToken } | ConvertTo-Json -Depth 8
```

### 3d. Fetch encrypted media bytes through a same-origin grant path

```powershell
$Bytes = [Text.Encoding]::UTF8.GetBytes('simy media roundtrip test')
$UploadFile = Join-Path $env:TEMP ([guid]::NewGuid().ToString() + '.bin')
$DownloadFile = Join-Path $env:TEMP ([guid]::NewGuid().ToString() + '.bin')
[IO.File]::WriteAllBytes($UploadFile, $Bytes)

$Intent = Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/mailboxes/$MailboxId/media/upload-intents" -Method Post -ContentType 'application/json' -Headers @{ 'x-mailbox-token' = $MailboxToken } -Body (@{
  media_type = 'image/jpeg'
  original_size_bytes = $Bytes.Length
} | ConvertTo-Json)

$HeaderArgs = @()
$Intent.presigned_upload.headers.psobject.Properties | ForEach-Object {
  $HeaderArgs += @('-H', ($_.Name + ': ' + [string]$_.Value))
}

curl.exe -sS -X PUT @HeaderArgs --data-binary ('@' + $UploadFile) $Intent.presigned_upload.url | Out-Null

$Manifest = Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/mailboxes/$MailboxId/media/manifests" -Method Post -ContentType 'application/json' -Headers @{ 'x-mailbox-token' = $MailboxToken } -Body (@{
  intent_id = $Intent.intent_id
} | ConvertTo-Json)

$Grant = Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/mailboxes/$MailboxId/media/access-grants" -Method Post -ContentType 'application/json' -Headers @{ 'x-mailbox-token' = $MailboxToken } -Body (@{
  object_key = $Manifest.object_key
  operation = 'download'
  ttl_seconds = 3600
} | ConvertTo-Json)

curl.exe -sS -o $DownloadFile "http://127.0.0.1:8081/v1/media/access-grants/$($Grant.grant_token)/content" | Out-Null
[Text.Encoding]::UTF8.GetString([IO.File]::ReadAllBytes($DownloadFile))
Remove-Item $UploadFile, $DownloadFile -ErrorAction SilentlyContinue
```

### 3e. Create and list an encrypted private feed reply

```powershell
$VisiblePosts = Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/mailboxes/$SecondMailboxId/feed/posts" -Headers @{ 'x-mailbox-token' = $SecondMailboxToken }
$PostId = $VisiblePosts.posts[0].post_id
$ReplyAuthorCiphertext = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes('{"iv":"reply-author","ciphertext":"opaque"}'))
$ReplyRecipientCiphertext = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes('{"iv":"reply-recipient","ciphertext":"opaque"}'))

Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/mailboxes/$SecondMailboxId/feed/posts/$PostId/replies" -Method Post -ContentType 'application/json' -Headers @{ 'x-mailbox-token' = $SecondMailboxToken } -Body (@{
  recipient_mailbox_id = $MailboxId
  author_ciphertext_b64 = $ReplyAuthorCiphertext
  recipient_ciphertext_b64 = $ReplyRecipientCiphertext
  ttl_seconds = 3600
} | ConvertTo-Json -Depth 6) | ConvertTo-Json -Depth 6

Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/mailboxes/$MailboxId/feed/posts" -Headers @{ 'x-mailbox-token' = $MailboxToken } | ConvertTo-Json -Depth 10
Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/mailboxes/$SecondMailboxId/feed/posts" -Headers @{ 'x-mailbox-token' = $SecondMailboxToken } | ConvertTo-Json -Depth 10
```

### 4. Bootstrap an admin mailbox

```powershell
$AdminMailboxId = [guid]::NewGuid().ToString()
$AdminMailboxToken = [Convert]::ToBase64String((1..32 | ForEach-Object { Get-Random -Minimum 0 -Maximum 256 }))
$RelayAdminToken = '<value-from-.env>'

Invoke-RestMethod -Uri http://127.0.0.1:8081/v1/mailboxes -Method Post -ContentType 'application/json' -Body (@{
  mailbox_id = $AdminMailboxId
  mailbox_token_b64 = $AdminMailboxToken
  codename = 'AdminWorkspace-01'
} | ConvertTo-Json) | Out-Null

Invoke-RestMethod -Uri http://127.0.0.1:8081/v1/admin/bootstrap-account -Method Post -ContentType 'application/json' -Headers @{
  'x-admin-token' = $RelayAdminToken
  'x-mailbox-token' = $AdminMailboxToken
} -Body (@{
  mailbox_id = $AdminMailboxId
  codename = 'AdminWorkspace-01'
} | ConvertTo-Json) | ConvertTo-Json -Depth 6
```

### 5. Provision a managed user mailbox

```powershell
$Managed = Invoke-RestMethod -Uri http://127.0.0.1:8081/v1/admin/provision-mailbox -Method Post -Headers @{
  'x-admin-token' = $RelayAdminToken
  'x-admin-mailbox-id' = $AdminMailboxId
  'x-mailbox-token' = $AdminMailboxToken
}

$Managed | ConvertTo-Json -Depth 6
```

### 6. Inspect the admin overview

```powershell
Invoke-RestMethod -Uri http://127.0.0.1:8081/v1/admin/overview -Headers @{
  'x-admin-token' = $RelayAdminToken
  'x-admin-mailbox-id' = $AdminMailboxId
  'x-mailbox-token' = $AdminMailboxToken
} | ConvertTo-Json -Depth 8
```

### 7. Reissue a managed user join link

```powershell
$Reissued = Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/admin/users/$($Managed.mailbox_id)/reset-access" -Method Post -Headers @{
  'x-admin-token' = $RelayAdminToken
  'x-admin-mailbox-id' = $AdminMailboxId
  'x-mailbox-token' = $AdminMailboxToken
}

$Reissued | ConvertTo-Json -Depth 6
```

### 8. Disable or re-enable a managed user

```powershell
Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/admin/users/$($Managed.mailbox_id)/status" -Method Post -ContentType 'application/json' -Headers @{
  'x-admin-token' = $RelayAdminToken
  'x-admin-mailbox-id' = $AdminMailboxId
  'x-mailbox-token' = $AdminMailboxToken
} -Body (@{ status = 'disabled' } | ConvertTo-Json) | ConvertTo-Json -Depth 6

Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/admin/users/$($Managed.mailbox_id)/status" -Method Post -ContentType 'application/json' -Headers @{
  'x-admin-token' = $RelayAdminToken
  'x-admin-mailbox-id' = $AdminMailboxId
  'x-mailbox-token' = $AdminMailboxToken
} -Body (@{ status = 'active' } | ConvertTo-Json) | ConvertTo-Json -Depth 6
```

### 9. Delete a managed user

```powershell
Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/admin/users/$($Managed.mailbox_id)" -Method Delete -Headers @{
  'x-admin-token' = $RelayAdminToken
  'x-admin-mailbox-id' = $AdminMailboxId
  'x-mailbox-token' = $AdminMailboxToken
} | ConvertTo-Json -Depth 4
```

### 10. Generate and publish an X3DH prekey bundle

```powershell
$Fixture = cargo run --quiet -p comm-core --example generate_identity_fixture | ConvertFrom-Json
$Bundle = $Fixture.prekey_bundle

Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/mailboxes/$MailboxId/prekeys" -Method Post -ContentType 'application/json' -Headers @{ 'x-mailbox-token' = $MailboxToken } -Body ($Bundle | ConvertTo-Json -Depth 6) | ConvertTo-Json -Depth 6
```

### 11. Fetch the public prekey bundle

```powershell
Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/prekeys/$MailboxId" | ConvertTo-Json -Depth 6
```

Repeated fetches should return different `one_time_prekey_b64` values until the mailbox runs out of published one-time prekeys.

### 12. Publish and list signed device records

```powershell
$DeviceRecord = $Fixture.device_record

Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/mailboxes/$MailboxId/devices" -Method Post -ContentType 'application/json' -Headers @{ 'x-mailbox-token' = $MailboxToken } -Body ($DeviceRecord | ConvertTo-Json -Depth 8) | ConvertTo-Json -Depth 6
Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/devices/$MailboxId" | ConvertTo-Json -Depth 8
Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/mailboxes/$MailboxId/prekeys/status" -Headers @{ 'x-mailbox-token' = $MailboxToken } | ConvertTo-Json -Depth 6
```

### 13. Submit a relay envelope

```powershell
$Payload = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes('{"kind":"demo","body":"hello"}'))
$ReplayToken = "replay_" + [guid]::NewGuid().ToString('N')

Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/mailboxes/$MailboxId/messages" -Method Post -ContentType 'application/json' -Body (@{
  ciphertext_b64 = $Payload
  sender_device_hint = 'desktop-test'
  ttl_seconds = 604800
  replay_token = $ReplayToken
} | ConvertTo-Json)
```

### 14. Retrieve messages

```powershell
Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/mailboxes/$MailboxId/messages" -Headers @{ 'x-mailbox-token' = $MailboxToken } | ConvertTo-Json -Depth 6
```

### 15. Delete a message

```powershell
$MessageId = '<message-id-from-list-response>'
Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/mailboxes/$MailboxId/messages/$MessageId" -Method Delete -Headers @{ 'x-mailbox-token' = $MailboxToken } | ConvertTo-Json -Depth 4
```

### 16. Create a media upload intent

```powershell
$DigestBytes = New-Object byte[] 32
$Rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
$Rng.GetBytes($DigestBytes)

$Intent = Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/mailboxes/$MailboxId/media/upload-intents" -Method Post -ContentType 'application/json' -Headers @{ 'x-mailbox-token' = $MailboxToken } -Body (@{
  media_type = 'image/jpeg'
  original_size_bytes = 842391
  chunk_size_bytes = 262144
  ttl_seconds = 900
  content_sha256_b64 = [Convert]::ToBase64String($DigestBytes)
} | ConvertTo-Json)

$Intent | ConvertTo-Json -Depth 8
```

### 17. Register a media manifest

```powershell
$Manifest = Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/mailboxes/$MailboxId/media/manifests" -Method Post -ContentType 'application/json' -Headers @{ 'x-mailbox-token' = $MailboxToken } -Body (@{
  intent_id = $Intent.intent_id
  content_sha256_b64 = [Convert]::ToBase64String($DigestBytes)
  upload_completed_at = [DateTime]::UtcNow.ToString('o')
} | ConvertTo-Json)

$Manifest | ConvertTo-Json -Depth 6
```

### 18. Create and resolve an access grant

```powershell
$Grant = Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/mailboxes/$MailboxId/media/access-grants" -Method Post -ContentType 'application/json' -Headers @{ 'x-mailbox-token' = $MailboxToken } -Body (@{
  object_key = $Manifest.object_key
  operation = 'download'
  ttl_seconds = 900
} | ConvertTo-Json)

$Grant | ConvertTo-Json -Depth 6
$ResolvedGrant = Invoke-RestMethod -Uri "http://127.0.0.1:8081/v1/media/access-grants/$($Grant.grant_token)"
$ResolvedGrant | ConvertTo-Json -Depth 8
```

### 12. Inspect or execute the presigned request

The resolved grant now includes `presigned_request.method`, `presigned_request.url`, and `presigned_request.headers`.

For a `download` grant you can inspect the actual MinIO request like this:

```powershell
$ResolvedGrant.presigned_request | ConvertTo-Json -Depth 6
```

If you want to execute that download request directly from PowerShell, copy the returned headers into an `Invoke-WebRequest` call and target the returned URL.

For an upload intent, the same pattern applies to `$Intent.presigned_upload`.
```

## Failure Cases Worth Testing

- create the same mailbox twice with the same token and verify activation succeeds
- create the same mailbox twice with a different token and verify `401 unauthorized`
- bootstrap a mailbox as admin without the live relay admin token and verify rejection
- disable a managed user and verify mailbox-authenticated endpoints reject that user
- delete a managed user and verify account profile lookup returns `401` or `404` depending on the token and route used afterward
- resubmit the same replay token and verify duplicate rejection
- retrieve with the wrong mailbox token and verify `401 unauthorized`
- send oversized ciphertext and verify request rejection
- wait past TTL and verify messages disappear
