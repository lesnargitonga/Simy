# Push Notification Constraints

## Rule

Push systems are wake channels only. They must not carry sender identity, ciphertext, mailbox identifiers, or human-readable content.

## Mobile Behavior

- APNs and FCM payloads should contain only a generic wake signal.
- The client wakes, opens its own TLS connection, and polls the relay for encrypted envelopes.
- Notification text shown to the user should be generic unless derived locally from already decrypted content.

## Threats Addressed

- Leakage of communication metadata to push providers.
- Exposure of sensitive content on lock screens.
- Correlation of sender identity through provider-visible payload structure.

## Implementation Notes

- Keep push token registration separate from mailbox identifiers.
- Rotate push tokens on reinstall or device reset.
- Allow users in high-risk mode to disable push entirely and rely on manual polling.
