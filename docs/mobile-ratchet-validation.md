# Mobile Ratchet Validation Checklist

This checklist keeps Android and iOS ratchet persistence behavior aligned as both clients wire the shared Rust core.

## Scope

- Persist opaque serialized ratchet session blobs from Rust.
- Encrypt blobs at rest with platform key management.
- Restore after restart and continue the same session.

## Android Acceptance

- `AndroidEncryptedRatchetStore` saves and loads encrypted records successfully.
- Blank session IDs are rejected.
- Deleting a session removes the persisted record.
- Instrumentation test file exists at [clients/android/src/androidTest/kotlin/com/simy/security/ratchet/AndroidEncryptedRatchetStoreInstrumentedTest.kt](../clients/android/src/androidTest/kotlin/com/simy/security/ratchet/AndroidEncryptedRatchetStoreInstrumentedTest.kt).

Suggested execution command once Android project wiring is complete:

```bash
gradle :simy-security:connectedDebugAndroidTest --stacktrace
```

## iOS Acceptance

- `IOSKeychainEncryptedRatchetStore` saves and loads encrypted records successfully.
- Blank session IDs are rejected.
- Deleting a session removes the persisted record.
- Test file exists at [clients/ios/Tests/SimySecurityTests/RatchetSessionStorageTests.swift](../clients/ios/Tests/SimySecurityTests/RatchetSessionStorageTests.swift).

Suggested execution command on macOS:

```bash
swift test -v
```

## Cross-Platform Parity Gates

- Session ID validation behavior is equivalent across platforms.
- Record format version handling exists and rejects unsupported versions.
- Restart continuity passes on both platforms before release.
- Test evidence is attached to release notes for each mobile build.
