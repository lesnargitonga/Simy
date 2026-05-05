# Desktop Client

## Implementation Status

**✅ Completed:**
- Tauri 2 application scaffold with React + TypeScript frontend
- Integrated `comm-core` Rust crate with Double Ratchet session support
- Encrypted file-backed session storage using `EncryptedFileDoubleRatchetSessionStore`
- Complete Tauri command interface for ratchet operations:
  - Identity generation (Ed25519 + X25519)
  - PreKey bundle generation
  - X3DH session bootstrap (initiator and responder)
  - Message encryption/decryption with session advancement
  - Session persistence and status checks
- Live test UI demonstrating full session lifecycle
- Restart-safe session continuity

**🚧 Next Steps:**
- Relay API integration for mailbox operations
- Contact management and encrypted message delivery
- Media upload/download with encrypted blob handling
- Push notification handling and wake hints
- Production build signing and update verification
- App lock and master key derivation from user password

## Architecture

The desktop client uses Tauri for a smaller attack surface than Electron-heavy alternatives.

Security-critical design decisions:

- Secret handling in Rust, not in the web layer
- Encrypted local session state with XChaCha20-Poly1305
- Session storage operations wrapped behind Tauri commands
- Disabled insecure navigation, remote content, and unnecessary permissions
- File-backed ratchet sessions for restart safety

## Running the Desktop Client

### Development Mode

```powershell
cd clients/desktop/simy-desktop
npm install
npm run tauri dev
```

The app will compile the Rust backend and launch the development window.

### Test Session Persistence

When the app launches, click **"Run Full Test"** to execute the complete Double Ratchet lifecycle:

1. Initialize encrypted session store
2. Generate Alice and Bob identities
3. Create Bob's prekey bundle
4. Alice bootstraps initiator session with X3DH
5. Bob receives and bootstraps responder session
6. Bob encrypts message → Alice decrypts
7. Alice encrypts follow-up → Bob decrypts
8. Session status verification shows message counts

All sessions are persisted to `./simy-ratchet-sessions-test/` with encrypted file storage.

## Session Storage Format

Sessions are stored as encrypted JSON files:
- **Storage location:** Configurable directory path
- **Encryption:** XChaCha20-Poly1305 with 256-bit key
- **File naming:** SHA-256 hash of session ID
- **AAD binding:** Session ID bound to ciphertext
- **Atomicity:** Write-to-temp, atomic rename pattern

Each session file contains:
```json
{
  "nonce_b64": "base64-encoded-24-byte-nonce",
  "ciphertext_b64": "base64-encrypted-session-state"
}
```

Decrypted session state includes:
- DH ratchet keys (sending and receiving)
- Chain keys and message numbers
- Skipped message keys (bounded cache)
- Root key for future ratchet steps

## Tauri Commands Reference

### `init_ratchet_store`
Initializes the encrypted session store with a storage path and 32-byte encryption key.

### `generate_identity`
Generates a new Ed25519 signing key + X25519 exchange key pair.

### `generate_prekey_bundle`
Creates a signed prekey bundle from an existing identity.

### `bootstrap_initiator`
Starts an X3DH session from Alice's perspective, sealing the initial message.

### `bootstrap_responder`
Completes X3DH session setup from Bob's perspective, opening the initial message.

### `encrypt_message`
Encrypts plaintext with the current ratchet session, advancing session state.

### `decrypt_message`
Decrypts ciphertext with the current ratchet session, advancing session state.

### `check_session_status`
Returns whether a session exists and the current message count.

## Security Model

### What the desktop client protects

- Long-term identity keys stored in encrypted session files
- Ephemeral ratchet state with forward secrecy
- Skipped message keys for out-of-order delivery
- Session metadata (message counts, DH ratchet state)

### What the desktop client does not yet protect

- Master key derivation from user password (stored as raw 32-byte key)
- App lock / screen lock integration
- Secure deletion of old session files
- OS keychain integration for encryption key storage

### Attack surface reduction

- Rust command layer prevents JavaScript from accessing raw secrets
- TypeScript can only call predefined Tauri commands
- No `eval()`, no remote script loading, no insecure navigation
- Session encryption keys never exposed to frontend
- File I/O happens only in Rust backend

## Future Relay Integration

The next phase connects the desktop client to the Simy relay:

1. **Mailbox operations:** Create, activate, retrieve messages
2. **PreKey publication:** Upload signed prekey bundles to relay
3. **Message submission:** Seal ratcheted ciphertext as relay envelopes
4. **Message retrieval:** Poll mailbox, decrypt relay envelopes locally
5. **Contact discovery:** Fetch public prekey bundles for first contact
6. **Media handling:** Upload/download encrypted blobs via presigned URLs

The current test UI demonstrates that the protocol layer is working. Relay integration will wire it to the real message transport.

## Platform-Specific Notes

### Windows
- Tauri builds with MSVC toolchain
- Session files written to user-specified directory
- No OS keychain integration yet

### macOS
- Tauri builds with system Rust
- Future: Integrate with Keychain for encryption key storage

### Linux
- Tauri builds with system Rust
- Future: Integrate with Secret Service API for key storage

## Build and Release

Not yet implemented. Next steps:

- Configure Tauri code signing for release builds
- Set up auto-update infrastructure
- Define update verification policy
- Package for Windows (MSI), macOS (DMG), Linux (AppImage/deb)
