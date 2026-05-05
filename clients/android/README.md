# Android Wrapper Plan

- Build the UI in Kotlin.
- Keep cryptographic state management in the shared Rust core.
- Store long-term device secrets with Android Keystore.
- Use certificate pinning and empty push tickles only.
- Gate unsafe screenshots and clipboard access where the platform allows.

## Ratchet Storage Skeleton

- Added Android-side trait-compatible storage boundary at [src/main/kotlin/com/simy/security/ratchet/RatchetSessionStorage.kt](src/main/kotlin/com/simy/security/ratchet/RatchetSessionStorage.kt).
- The `RatchetSessionStorage` interface mirrors the Rust `DoubleRatchetSessionStore` methods: load, save, and delete.
- `AndroidEncryptedRatchetStore` now includes:
	- non-blank session ID validation
	- stable hashed filenames from session IDs
	- atomic write pattern using temporary files
	- Android Keystore-backed AES-GCM encryption and decryption of stored ratchet session blobs

### Immediate Wiring Steps

1. Bridge opaque serialized ratchet session payloads between Rust and Kotlin FFI boundary.
2. Run Android instrumentation tests at [src/androidTest/kotlin/com/simy/security/ratchet/AndroidEncryptedRatchetStoreInstrumentedTest.kt](src/androidTest/kotlin/com/simy/security/ratchet/AndroidEncryptedRatchetStoreInstrumentedTest.kt).
	Command:
	`gradle :simy-security:connectedDebugAndroidTest --stacktrace`
3. Add key-rotation and record-version migration handling for long-lived installations.
4. Track parity gates in [../../docs/mobile-ratchet-validation.md](../../docs/mobile-ratchet-validation.md).
