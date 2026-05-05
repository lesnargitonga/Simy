use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};

use base64ct::{Base64, Encoding};
use chacha20poly1305::{
    aead::{Aead, Payload},
    KeyInit, XChaCha20Poly1305, XNonce,
};
use comm_core::{
    bootstrap_initiator_session, bootstrap_responder_session, open_initial_message,
    seal_initial_message, CoreError, DoubleRatchetSession, DoubleRatchetSessionStore,
    IdentityKeyPair, PersistedDoubleRatchetSession, PreKeyBundle, RelayContentKind, SignedPreKey,
};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
struct EncryptedInMemoryRatchetStore {
    key: [u8; 32],
    records: Arc<RwLock<BTreeMap<String, EncryptedRecord>>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct EncryptedRecord {
    nonce_b64: String,
    ciphertext_b64: String,
}

impl EncryptedInMemoryRatchetStore {
    fn new_random() -> Self {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        Self {
            key,
            records: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    fn encrypt_persisted(
        &self,
        session_id: &str,
        session: &PersistedDoubleRatchetSession,
    ) -> Result<EncryptedRecord, CoreError> {
        let serialized = serde_json::to_vec(session).map_err(|_| CoreError::StorageFailure)?;
        let mut nonce = [0u8; 24];
        OsRng.fill_bytes(&mut nonce);
        let cipher = XChaCha20Poly1305::new((&self.key).into());
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &serialized,
                    aad: session_id.as_bytes(),
                },
            )
            .map_err(|_| CoreError::StorageFailure)?;

        Ok(EncryptedRecord {
            nonce_b64: Base64::encode_string(&nonce),
            ciphertext_b64: Base64::encode_string(&ciphertext),
        })
    }

    fn decrypt_persisted(
        &self,
        session_id: &str,
        record: &EncryptedRecord,
    ) -> Result<PersistedDoubleRatchetSession, CoreError> {
        let nonce_bytes = Base64::decode_vec(&record.nonce_b64).map_err(|_| CoreError::StorageFailure)?;
        let nonce: [u8; 24] = nonce_bytes.try_into().map_err(|_| CoreError::StorageFailure)?;
        let ciphertext = Base64::decode_vec(&record.ciphertext_b64).map_err(|_| CoreError::StorageFailure)?;
        let cipher = XChaCha20Poly1305::new((&self.key).into());
        let plaintext = cipher
            .decrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &ciphertext,
                    aad: session_id.as_bytes(),
                },
            )
            .map_err(|_| CoreError::StorageFailure)?;

        serde_json::from_slice::<PersistedDoubleRatchetSession>(&plaintext)
            .map_err(|_| CoreError::StorageFailure)
    }
}

impl DoubleRatchetSessionStore for EncryptedInMemoryRatchetStore {
    fn load_session(&self, session_id: &str) -> Result<Option<PersistedDoubleRatchetSession>, CoreError> {
        if session_id.trim().is_empty() {
            return Err(CoreError::InvalidRatchetSessionId);
        }

        let records = self.records.read().map_err(|_| CoreError::StorageFailure)?;
        let persisted = records
            .get(session_id)
            .map(|record| self.decrypt_persisted(session_id, record))
            .transpose()?;
        Ok(persisted)
    }

    fn save_session(
        &self,
        session_id: &str,
        session: &PersistedDoubleRatchetSession,
    ) -> Result<(), CoreError> {
        if session_id.trim().is_empty() {
            return Err(CoreError::InvalidRatchetSessionId);
        }

        let encrypted = self.encrypt_persisted(session_id, session)?;
        let mut records = self.records.write().map_err(|_| CoreError::StorageFailure)?;
        records.insert(session_id.to_string(), encrypted);
        Ok(())
    }

    fn delete_session(&self, session_id: &str) -> Result<(), CoreError> {
        if session_id.trim().is_empty() {
            return Err(CoreError::InvalidRatchetSessionId);
        }

        let mut records = self.records.write().map_err(|_| CoreError::StorageFailure)?;
        records.remove(session_id);
        Ok(())
    }
}

fn main() {
    let alice_identity = IdentityKeyPair::generate();
    let bob_identity = IdentityKeyPair::generate();
    let bob_signed_prekey = SignedPreKey::generate(&bob_identity);
    let bob_bundle = PreKeyBundle::from_parts(&bob_identity, &bob_signed_prekey, None);

    let (initial_envelope, alice_handshake) = seal_initial_message(
        &alice_identity,
        &bob_bundle,
        b"session-bootstrap",
        RelayContentKind::SessionBootstrap,
    )
    .expect("failed to create bootstrap envelope");
    let opened = open_initial_message(&bob_identity, &bob_signed_prekey, None, &initial_envelope)
        .expect("failed to open bootstrap envelope");

    let alice_session =
        bootstrap_initiator_session(alice_handshake).expect("failed to bootstrap alice");
    let mut bob_session = bootstrap_responder_session(&bob_signed_prekey, &opened)
        .expect("failed to bootstrap bob");

    let store = EncryptedInMemoryRatchetStore::new_random();
    let session_id = "alice:bob:primary";

    alice_session
        .save_to_store(&store, session_id)
        .expect("failed to save alice session");

    let mut restored_alice = DoubleRatchetSession::load_from_store(&store, session_id)
        .expect("failed to load from store")
        .expect("session missing from store");

    let message_from_bob = bob_session
        .encrypt(b"message after restore", b"chat-1")
        .expect("bob encrypt failed");
    let opened_by_alice = restored_alice
        .decrypt(&message_from_bob, b"chat-1")
        .expect("alice decrypt failed");
    println!(
        "Restored Alice decrypted: {}",
        String::from_utf8(opened_by_alice).expect("plaintext should be utf8")
    );

    let reply_from_alice = restored_alice
        .encrypt(b"alice persisted reply", b"chat-1")
        .expect("alice encrypt failed");
    let opened_by_bob = bob_session
        .decrypt(&reply_from_alice, b"chat-1")
        .expect("bob decrypt failed");
    println!(
        "Bob decrypted from restored Alice: {}",
        String::from_utf8(opened_by_bob).expect("plaintext should be utf8")
    );

    restored_alice
        .save_to_store(&store, session_id)
        .expect("failed to persist updated session");

    DoubleRatchetSession::delete_from_store(&store, session_id)
        .expect("failed to delete persisted session");
    let exists_after_delete = DoubleRatchetSession::load_from_store(&store, session_id)
        .expect("failed to load after delete")
        .is_some();

    println!("Session exists after delete: {}", exists_after_delete);
}
