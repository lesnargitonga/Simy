# Frontend Showcase URLs

Use URL parameters to open the frontend in presentation mode and prefill workspace state for fast demos.

## URL Parameters

- `demo=1`: enables presentation mode by default (tools hidden).
- `tools=1`: shows the tools panel.
- `theme=light|dark`: sets initial theme.
- `name=<alias>`: prefills codename.
- `mailbox=<uuid>`: prefills mailbox ID.
- `token=<base64>`: prefills mailbox token.
- `auto=1`: auto-opens workspace when prefilled credentials are provided.

## Example Links

Presentation-only mode:

```text
http://127.0.0.1:8081/?demo=1
```

Presentation mode with prefilled workspace:

```text
http://127.0.0.1:8081/?demo=1&name=Showcase-User&mailbox=11111111-1111-1111-1111-111111111111&token=<base64_token_with_32_plus_bytes>
```

Auto-launch workspace from URL:

```text
http://127.0.0.1:8081/?demo=1&auto=1&name=Showcase-User&mailbox=11111111-1111-1111-1111-111111111111&token=<base64_token_with_32_plus_bytes>

## Role Showcase

- User side: regular workspace without admin bootstrap.
- Admin side: bootstrap workspace as admin.
- Super Admin side: unlock admin workspace with live relay admin token. UI badge switches to `Super Admin` while this privileged session is active.

For presentation, use two browser contexts:

- Normal window for admin or super admin side
- Incognito/private window for user side
```

## In-Product Shortcut

Use `Share Showcase URL` in the left sidebar after opening a workspace. It generates a link with current workspace identity and presentation settings.

## Security Note

Showcase URLs can include mailbox tokens. Treat them as sensitive credentials and rotate tokens after demos if links were shared broadly.