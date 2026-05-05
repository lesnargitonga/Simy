# Threat Model

## Assets

- Long-term identity keys
- Device trust state
- Session secrets and ratchet state
- Encrypted message blobs
- Mailbox identifiers and access tokens
- Security update channel integrity

## Adversaries

### Network Observer

Can observe client-to-relay timing, volume, source IP, and destination IP. Cannot break modern TLS or audited message encryption primitives.

### Malicious Insider

Has access to infrastructure consoles, metrics, logs, and databases. Must not be able to recover message plaintext from server-side systems.

### Account Takeover Operator

Attempts phishing, token theft, SIM swap, credential stuffing, or social engineering against recovery and device onboarding flows.

### Endpoint Adversary

Has temporary or permanent access to a user device through theft, seizure, malware, or unsafe backups. This is the highest practical risk.

### Abuse Actor

Attempts flooding, replay, mailbox enumeration, and resource exhaustion.

## Assumptions

- Production cryptography comes from audited implementations, not custom protocol code.
- Hardware-backed key storage exists on supported mobile devices, but not all desktops.
- Push providers are available but treated as metadata-leaking systems and used only for empty wake hints.
- Users can verify high-risk contacts through a secondary channel when needed.

## Security Goals

- Preserve message confidentiality and integrity in transit and at rest on the server.
- Detect identity key changes and avoid silent trust replacement.
- Minimize metadata retention and observability.
- Survive partial outages without data loss or message corruption.
- Enforce expiry and replay protections.

## Non-Goals

- Perfect anonymity against a global passive adversary.
- Safe operation on fully compromised endpoints.
- Invisible use of network infrastructure or push services.

## Required Mitigations

- TLS 1.3 with certificate pinning in mobile and desktop clients.
- Audited asynchronous messaging protocol for 1:1 sessions.
- Audited group protocol for large groups.
- Short retention windows for mailbox data and operational events.
- Replay cache and bounded nonce tracking.
- Rate limiting on mailbox creation, submission, and retrieval.
- Signed client updates and rollback protection.
- User-visible verification flow for sensitive contacts.
