use std::{
    fs,
    path::{Path, PathBuf},
};

use base64ct::{Base64, Encoding};
use chacha20poly1305::{
    aead::{Aead, Payload},
    KeyInit, XChaCha20Poly1305, XNonce,
};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    CoreError, DoubleRatchetSessionStore, PersistedDoubleRatchetSession,
};

#[derive(Clone, Debug)]
pub struct EncryptedFileDoubleRatchetSessionStore {
    root_dir: PathBuf,
    encryption_key: [u8; 32],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct EncryptedSessionRecord {
    nonce_b64: String,
    ciphertext_b64: String,
}

impl EncryptedFileDoubleRatchetSessionStore {
    pub fn new(root_dir: impl Into<PathBuf>, encryption_key: [u8; 32]) -> Self {
        Self {
            root_dir: root_dir.into(),
            encryption_key,
        }
    }

    pub fn with_random_key(root_dir: impl Into<PathBuf>) -> Self {
        let mut encryption_key = [0u8; 32];
        OsRng.fill_bytes(&mut encryption_key);
        Self::new(root_dir, encryption_key)
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    fn session_path(&self, session_id: &str) -> PathBuf {
        let session_hash = Sha256::digest(session_id.as_bytes());
        let file_name = format!("{}.json", hex::encode(session_hash));
        self.root_dir.join(file_name)
    }

    fn encrypt(
        &self,
        session_id: &str,
        session: &PersistedDoubleRatchetSession,
    ) -> Result<EncryptedSessionRecord, CoreError> {
        let plaintext = serde_json::to_vec(session).map_err(|_| CoreError::StorageFailure)?;
        let mut nonce = [0u8; 24];
        OsRng.fill_bytes(&mut nonce);
        let cipher = XChaCha20Poly1305::new((&self.encryption_key).into());
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &plaintext,
                    aad: session_id.as_bytes(),
                },
            )
            .map_err(|_| CoreError::StorageFailure)?;

        Ok(EncryptedSessionRecord {
            nonce_b64: Base64::encode_string(&nonce),
            ciphertext_b64: Base64::encode_string(&ciphertext),
        })
    }

    fn decrypt(
        &self,
        session_id: &str,
        record: &EncryptedSessionRecord,
    ) -> Result<PersistedDoubleRatchetSession, CoreError> {
        let nonce_bytes = Base64::decode_vec(&record.nonce_b64).map_err(|_| CoreError::StorageFailure)?;
        let nonce: [u8; 24] = nonce_bytes.try_into().map_err(|_| CoreError::StorageFailure)?;
        let ciphertext = Base64::decode_vec(&record.ciphertext_b64).map_err(|_| CoreError::StorageFailure)?;
        let cipher = XChaCha20Poly1305::new((&self.encryption_key).into());
        let plaintext = cipher
            .decrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &ciphertext,
                    aad: session_id.as_bytes(),
                },
            )
            .map_err(|_| CoreError::StorageFailure)?;

        serde_json::from_slice(&plaintext).map_err(|_| CoreError::StorageFailure)
    }
}

impl DoubleRatchetSessionStore for EncryptedFileDoubleRatchetSessionStore {
    fn load_session(&self, session_id: &str) -> Result<Option<PersistedDoubleRatchetSession>, CoreError> {
        validate_session_id(session_id)?;
        let path = self.session_path(session_id);
        if !path.exists() {
            return Ok(None);
        }

        let bytes = fs::read(path).map_err(|_| CoreError::StorageFailure)?;
        let record: EncryptedSessionRecord =
            serde_json::from_slice(&bytes).map_err(|_| CoreError::StorageFailure)?;
        let session = self.decrypt(session_id, &record)?;
        Ok(Some(session))
    }

    fn save_session(
        &self,
        session_id: &str,
        session: &PersistedDoubleRatchetSession,
    ) -> Result<(), CoreError> {
        validate_session_id(session_id)?;
        fs::create_dir_all(&self.root_dir).map_err(|_| CoreError::StorageFailure)?;

        let record = self.encrypt(session_id, session)?;
        let path = self.session_path(session_id);
        let temp_path = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec(&record).map_err(|_| CoreError::StorageFailure)?;

        fs::write(&temp_path, bytes).map_err(|_| CoreError::StorageFailure)?;
        if path.exists() {
            fs::remove_file(&path).map_err(|_| CoreError::StorageFailure)?;
        }
        fs::rename(temp_path, path).map_err(|_| CoreError::StorageFailure)?;
        Ok(())
    }

    fn delete_session(&self, session_id: &str) -> Result<(), CoreError> {
        validate_session_id(session_id)?;
        let path = self.session_path(session_id);
        if path.exists() {
            fs::remove_file(path).map_err(|_| CoreError::StorageFailure)?;
        }
        Ok(())
    }
}

fn validate_session_id(session_id: &str) -> Result<(), CoreError> {
    if session_id.trim().is_empty() {
        return Err(CoreError::InvalidRatchetSessionId);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use rand_core::{OsRng, RngCore};

    use crate::{
        bootstrap_initiator_session, bootstrap_responder_session, open_initial_message,
        seal_initial_message, DoubleRatchetSession, IdentityKeyPair, PreKeyBundle,
        RelayContentKind, SignedPreKey,
    };

    use super::*;

    #[test]
    fn file_store_round_trips_ratchet_session_after_restart() {
        let temp_dir = unique_temp_dir("simy-ratchet-store");
        let store = EncryptedFileDoubleRatchetSessionStore::with_random_key(&temp_dir);

        let alice_identity = IdentityKeyPair::generate();
        let bob_identity = IdentityKeyPair::generate();
        let bob_signed_prekey = SignedPreKey::generate(&bob_identity);
        let bob_bundle = PreKeyBundle::from_parts(&bob_identity, &bob_signed_prekey, None);
        let (initial_envelope, alice_handshake) = seal_initial_message(
            &alice_identity,
            &bob_bundle,
            b"bootstrap",
            RelayContentKind::SessionBootstrap,
        )
        .unwrap();
        let opened =
            open_initial_message(&bob_identity, &bob_signed_prekey, None, &initial_envelope).unwrap();

        let mut alice_session = bootstrap_initiator_session(alice_handshake).unwrap();
        let mut bob_session = bootstrap_responder_session(&bob_signed_prekey, &opened).unwrap();
        let session_id = "alice:bob:main";

        let first = bob_session.encrypt(b"first", b"chat-1").unwrap();
        let opened_first = alice_session.decrypt(&first, b"chat-1").unwrap();
        assert_eq!(opened_first, b"first");

        alice_session.save_to_store(&store, session_id).unwrap();

        let mut restored = DoubleRatchetSession::load_from_store(&store, session_id)
            .unwrap()
            .unwrap();

        let outbound = restored.encrypt(b"after-restart", b"chat-1").unwrap();
        let opened_by_bob = bob_session.decrypt(&outbound, b"chat-1").unwrap();
        assert_eq!(opened_by_bob, b"after-restart");

        DoubleRatchetSession::delete_from_store(&store, session_id).unwrap();
        let missing = DoubleRatchetSession::load_from_store(&store, session_id).unwrap();
        assert!(missing.is_none());

        fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn file_store_rejects_blank_session_id() {
        let temp_dir = unique_temp_dir("simy-ratchet-store-invalid");
        let store = EncryptedFileDoubleRatchetSessionStore::with_random_key(&temp_dir);
        let result = store.load_session("   ");

        assert!(matches!(result, Err(CoreError::InvalidRatchetSessionId)));
        fs::remove_dir_all(temp_dir).unwrap();
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let mut suffix = [0u8; 8];
        OsRng.fill_bytes(&mut suffix);
        let name = format!("{}-{}", prefix, hex::encode(suffix));
        let path = env::temp_dir().join(name);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
