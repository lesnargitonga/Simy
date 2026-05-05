# Architecture Overview

## Scope

This system is designed for lawful, defensive protection of high-risk communications. The focus is end-to-end confidentiality, device authenticity, metadata minimization, and operational resilience.

## Components

### Shared Core

`crates/comm-core` owns identity material, public key fingerprints, serialization rules, media pointer descriptors, feed-post descriptors, and the future audited protocol integrations for asynchronous 1:1 messaging and group messaging.

### Relay Service

`services/relay` accepts encrypted message envelopes, applies TTL policy, and exposes mailbox retrieval APIs. The relay must remain blind to plaintext and should never become a source of durable high-value metadata.

### Storage Layer

- PostgreSQL stores mailbox indices, TTL metadata, and operational state.
- Object storage stores encrypted blobs if message sizes exceed practical row limits.
- Redis provides replay caches, rate limits, and short-lived counters.

### Media and Feed Layer

Encrypted media must travel through object storage as padded ciphertext blobs. The relay carries only the small encrypted pointer messages that tell trusted recipients how to fetch and decrypt those blobs. Feed-style posts remain end-to-end encrypted content envelopes and should be modeled as one-way encrypted distributions to authorized contacts, not as a server-readable social feed.

### Client Wrappers

Android, iOS, and desktop clients call the Rust core for sensitive operations and keep UI/platform code thin.

## Trust Boundaries

- Untrusted network between client and relay.
- Semi-trusted relay that handles only ciphertext and minimum operational metadata.
- Trusted client runtime only to the extent that the endpoint has not been compromised.
- External push providers treated as untrusted hint channels.

## Design Principles

- Never log plaintext content.
- Prefer short-lived identifiers and narrow-scope tokens.
- Separate long-term identity keys from device and session keys.
- Make key changes explicit and visible to users.
- Keep retention windows short and enforce them in code.
- Keep large media out of the mailbox database path and move only encrypted pointers through the relay.
