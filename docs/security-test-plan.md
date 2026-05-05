# Security Test Plan

## Release Gates

- Rust workspace builds cleanly.
- Static analysis and linting enabled.
- Dependency audit performed on each release candidate.
- Manual review of any changes to crypto, trust logic, or retention rules.

## Test Areas

### Protocol and Key Handling

- Identity generation is stable and fingerprint formatting is deterministic.
- Key changes trigger explicit trust warnings.
- Device revocation blocks future message acceptance.

### Relay Behavior

- Ciphertext-only submission and retrieval.
- TTL bounds enforced at the API boundary.
- Expired messages are not returned.
- Replay identifiers are rejected inside the replay window.
- Mailbox enumeration attempts are rate limited.

### Client Security

- Local encrypted store resists access without app unlock.
- Screenshot and clipboard restrictions behave as expected per platform.
- Push notifications contain only generic wake signals.

### Operational Security

- Logging policy verified in integration tests.
- Signed update pipeline tested for rollback resistance.
- Backup and restore flows do not violate retention promises.

## External Review

- Independent cryptography review before production rollout.
- Penetration test covering auth, relay APIs, update channel, and client trust UX.
- Tabletop incident exercise for lost device and relay compromise scenarios.
