# iOS Wrapper Plan

- Build the UI in Swift.
- Generate and protect device identity with Secure Enclave or keychain-backed storage.
- Call the Rust core through generated bindings or a thin FFI layer.
- Use APNs only for empty wake notifications; fetch ciphertext over app-managed TLS.
- Apply local app lock, secure background handling, and encrypted persistence.

## Ratchet Storage Implementation

- Added iOS-side storage boundary and encrypted store at [Sources/SimySecurity/Ratchet/RatchetSessionStorage.swift](Sources/SimySecurity/Ratchet/RatchetSessionStorage.swift).
- The `RatchetSessionStorage` protocol mirrors the Rust `DoubleRatchetSessionStore` contract: load, save, and delete.
- `IOSKeychainEncryptedRatchetStore` now includes:
	- non-blank session ID validation
	- stable hashed filenames from session IDs
	- atomic file writes for encrypted ratchet records
	- Keychain-backed symmetric key generation and retrieval
	- AES-GCM encryption and decryption for persisted session blobs

### Immediate Wiring Steps

1. Bridge opaque serialized ratchet session payloads between Rust and Swift FFI boundary.
2. Run iOS ratchet storage tests at [Tests/SimySecurityTests/RatchetSessionStorageTests.swift](Tests/SimySecurityTests/RatchetSessionStorageTests.swift).
	Command:
	`swift test -v`
3. Add record-version migration and key-rotation handling for long-lived installations.
4. Track parity gates in [../../docs/mobile-ratchet-validation.md](../../docs/mobile-ratchet-validation.md).
