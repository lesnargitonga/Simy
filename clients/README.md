# Client Targets

The shared Rust core should be exposed to platform wrappers rather than reimplemented per platform.

- `android`: Kotlin app using hardware-backed keystore and strict certificate pinning.
- `ios`: Swift app using Secure Enclave and keychain-backed local storage.
- `desktop`: Tauri shell with hardened settings and encrypted local state.

The next client milestone is not just chat UI. Each client will also need:

- encrypted media upload and download orchestration against object storage
- EXIF stripping before media encryption
- rendering support for private feed-style posts that are decrypted locally
- a clear boundary between relay envelopes and object-store blob retrieval

Use generated bindings or FFI wrappers from the Rust core once the protocol surface is stable.
