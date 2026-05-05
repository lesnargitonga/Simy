# Data Retention

## Principles

- Retain the minimum data required for mailbox delivery, abuse prevention, and reliability.
- Make TTL and deletion behavior deterministic and testable.
- Do not collect fields that are not operationally necessary.

## Content Data

- Message plaintext: never stored server-side.
- Encrypted payload blobs: stored only until delivery or TTL expiry.
- Default TTL target: 7 to 30 days, policy configurable.

## Metadata

### Mailbox Records

- Mailbox identifier
- Message identifier
- Receipt timestamp
- Expiry timestamp
- Delivery status

Retention: until delivery acknowledgement or TTL expiry.

### Abuse Controls

- Narrow-scope counters for rate limiting and replay detection.
- Prefer keyed hashes or short-lived opaque tokens over raw identifiers.

Retention: minutes to hours, depending on abuse control requirements.

### Authentication Events

- Device registration timestamp
- Failed token attempts count
- Security-sensitive state changes

Retention: short TTL with aggregation where possible.

## Logging Policy

- No plaintext content in logs.
- No full bearer tokens in logs.
- No durable IP logs unless strictly required for abuse response and approved by policy.
- Use aggregation and redaction for observability.

## Deletion Guarantees

- TTL expiration removes mailbox records from primary storage.
- Background jobs must clean expired blobs and counters promptly.
- Backups should inherit short retention or exclude ephemeral message stores when feasible.
