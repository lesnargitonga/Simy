# Key Management

## Key Classes

### Identity Key

- Generated on-device during onboarding.
- Long-lived and user-visible.
- Stored in hardware-backed keystore where available.
- Used for trust establishment and signature verification.

### Device Key

- Unique per registered device.
- Rotated on device reset or compromise events.
- Bound to the user identity through signed device records.

### Session Keys

- Derived through audited asynchronous messaging protocol state.
- Provide forward secrecy and post-compromise recovery.
- Never exported in plaintext from the secure core.

### Group Epoch Keys

- Managed through audited group messaging state.
- Rotated on membership changes and periodic commits.

## Lifecycle

1. Generate identity and initial device material on-device.
2. Export public bundle for contact verification and server registration.
3. Store secrets with platform keystore APIs where possible.
4. Rotate device material on compromise, device loss, or user action.
5. Revoke compromised devices and force remote session invalidation.
6. Keep signed audit events for trust changes on the client side.

## Verification

- Expose human-verifiable fingerprints or safety numbers.
- Support QR-based verification for in-person trust establishment.
- Warn loudly on identity key changes.
- Never silently trust replacement keys for verified contacts.

## Recovery

- Prefer device-to-device secure enrollment over account-password recovery.
- Avoid server-side escrow of long-term private keys.
- If recovery packages are introduced, require user-held secrets and clear risk disclosure.
