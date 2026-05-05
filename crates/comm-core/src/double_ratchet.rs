use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};

use base64ct::{Base64, Encoding};
use chacha20poly1305::{
    aead::{Aead, Payload},
    KeyInit, XChaCha20Poly1305, XNonce,
};
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::{CoreError, InitiatorHandshake, OpenedInitialMessage, SignedPreKey};

const DOUBLE_RATCHET_VERSION: &str = "simy-double-ratchet-v1";
const ROOT_INFO: &[u8] = b"simy-dr-root-v1";
const CHAIN_INFO: &[u8] = b"simy-dr-chain-v1";
const MAX_SKIPPED_MESSAGE_KEYS: usize = 128;

pub struct RatchetKeyPair {
    pub secret_key: StaticSecret,
    pub public_key: PublicKey,
}

impl core::fmt::Debug for RatchetKeyPair {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("RatchetKeyPair")
            .field("public_key", &self.public_key)
            .field("secret_key", &"<redacted>")
            .finish()
    }
}

impl RatchetKeyPair {
    pub fn generate() -> Self {
        let mut csprng = OsRng;
        let secret_key = StaticSecret::random_from_rng(&mut csprng);
        Self::from_secret(secret_key)
    }

    fn from_secret(secret_key: StaticSecret) -> Self {
        let public_key = PublicKey::from(&secret_key);
        Self {
            secret_key,
            public_key,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RatchetHeader {
    pub ratchet_public_key_b64: String,
    pub previous_chain_length: u32,
    pub message_number: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RatchetMessageEnvelope {
    pub version: String,
    pub header: RatchetHeader,
    pub nonce_b64: String,
    pub ciphertext_b64: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersistedDoubleRatchetSession {
    pub version: String,
    pub root_key_b64: String,
    pub local_ratchet_secret_key_b64: String,
    pub local_ratchet_public_key_b64: String,
    pub remote_ratchet_public_key_b64: Option<String>,
    pub sending_chain_key_b64: Option<String>,
    pub receiving_chain_key_b64: Option<String>,
    pub previous_chain_length: u32,
    pub sent_messages: u32,
    pub received_messages: u32,
    pub skipped_keys: Vec<PersistedSkippedMessageKey>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersistedSkippedMessageKey {
    pub ratchet_public_key_b64: String,
    pub message_number: u32,
    pub message_key_b64: String,
}

pub trait DoubleRatchetSessionStore {
    fn load_session(&self, session_id: &str) -> Result<Option<PersistedDoubleRatchetSession>, CoreError>;

    fn save_session(
        &self,
        session_id: &str,
        session: &PersistedDoubleRatchetSession,
    ) -> Result<(), CoreError>;

    fn delete_session(&self, session_id: &str) -> Result<(), CoreError>;
}

#[derive(Clone, Debug, Default)]
pub struct InMemoryDoubleRatchetSessionStore {
    sessions: Arc<RwLock<BTreeMap<String, PersistedDoubleRatchetSession>>>,
}

impl InMemoryDoubleRatchetSessionStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl DoubleRatchetSessionStore for InMemoryDoubleRatchetSessionStore {
    fn load_session(&self, session_id: &str) -> Result<Option<PersistedDoubleRatchetSession>, CoreError> {
        validate_session_id(session_id)?;
        let sessions = self.sessions.read().map_err(|_| CoreError::StorageFailure)?;
        Ok(sessions.get(session_id).cloned())
    }

    fn save_session(
        &self,
        session_id: &str,
        session: &PersistedDoubleRatchetSession,
    ) -> Result<(), CoreError> {
        validate_session_id(session_id)?;
        let mut sessions = self.sessions.write().map_err(|_| CoreError::StorageFailure)?;
        sessions.insert(session_id.to_string(), session.clone());
        Ok(())
    }

    fn delete_session(&self, session_id: &str) -> Result<(), CoreError> {
        validate_session_id(session_id)?;
        let mut sessions = self.sessions.write().map_err(|_| CoreError::StorageFailure)?;
        sessions.remove(session_id);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ParsedRatchetHeader {
    ratchet_public_key: PublicKey,
    previous_chain_length: u32,
    message_number: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SkippedKeyId {
    ratchet_public_key: [u8; 32],
    message_number: u32,
}

pub struct DoubleRatchetSession {
    root_key: [u8; 32],
    local_ratchet_key: RatchetKeyPair,
    remote_ratchet_key: Option<PublicKey>,
    sending_chain_key: Option<[u8; 32]>,
    receiving_chain_key: Option<[u8; 32]>,
    previous_chain_length: u32,
    sent_messages: u32,
    received_messages: u32,
    skipped_keys: BTreeMap<SkippedKeyId, [u8; 32]>,
}

impl core::fmt::Debug for DoubleRatchetSession {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("DoubleRatchetSession")
            .field("local_ratchet_key", &self.local_ratchet_key)
            .field("remote_ratchet_key", &self.remote_ratchet_key)
            .field("previous_chain_length", &self.previous_chain_length)
            .field("sent_messages", &self.sent_messages)
            .field("received_messages", &self.received_messages)
            .field("skipped_keys", &self.skipped_keys.len())
            .field("root_key", &"<redacted>")
            .field("sending_chain_key", &"<redacted>")
            .field("receiving_chain_key", &"<redacted>")
            .finish()
    }
}

impl DoubleRatchetSession {
    pub fn load_from_store<S: DoubleRatchetSessionStore>(
        store: &S,
        session_id: &str,
    ) -> Result<Option<Self>, CoreError> {
        let persisted = store.load_session(session_id)?;
        persisted
            .as_ref()
            .map(Self::from_persisted)
            .transpose()
    }

    pub fn save_to_store<S: DoubleRatchetSessionStore>(
        &self,
        store: &S,
        session_id: &str,
    ) -> Result<(), CoreError> {
        store.save_session(session_id, &self.persist())
    }

    pub fn delete_from_store<S: DoubleRatchetSessionStore>(
        store: &S,
        session_id: &str,
    ) -> Result<(), CoreError> {
        store.delete_session(session_id)
    }

    pub fn encrypt(
        &mut self,
        plaintext: &[u8],
        associated_data: &[u8],
    ) -> Result<RatchetMessageEnvelope, CoreError> {
        if plaintext.is_empty() {
            return Err(CoreError::InvalidRatchetMessage);
        }

        self.ensure_sending_chain()?;
        let chain_key = self
            .sending_chain_key
            .take()
            .ok_or(CoreError::MissingChainState)?;
        let header = RatchetHeader {
            ratchet_public_key_b64: Base64::encode_string(self.local_ratchet_key.public_key.as_bytes()),
            previous_chain_length: self.previous_chain_length,
            message_number: self.sent_messages,
        };
        let (next_chain_key, message_key) = derive_chain_key_and_message_key(&chain_key)?;
        self.sending_chain_key = Some(next_chain_key);
        self.sent_messages += 1;

        let mut nonce = [0u8; 24];
        OsRng.fill_bytes(&mut nonce);
        let cipher = XChaCha20Poly1305::new((&message_key).into());
        let aad = build_ratchet_aad(&header, associated_data);
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad: &aad,
                },
            )
            .map_err(|_| CoreError::CryptoFailure)?;

        Ok(RatchetMessageEnvelope {
            version: DOUBLE_RATCHET_VERSION.to_string(),
            header,
            nonce_b64: Base64::encode_string(&nonce),
            ciphertext_b64: Base64::encode_string(&ciphertext),
        })
    }

    pub fn decrypt(
        &mut self,
        envelope: &RatchetMessageEnvelope,
        associated_data: &[u8],
    ) -> Result<Vec<u8>, CoreError> {
        if envelope.version != DOUBLE_RATCHET_VERSION {
            return Err(CoreError::UnsupportedVersion);
        }

        let header = parse_ratchet_header(&envelope.header)?;
        if let Some(plaintext) = self.try_skipped_message_key(&header, envelope, associated_data)? {
            return Ok(plaintext);
        }

        if self.remote_ratchet_key != Some(header.ratchet_public_key) {
            self.skip_message_keys(header.previous_chain_length)?;
            self.apply_remote_ratchet(header.ratchet_public_key)?;
        }

        self.skip_message_keys(header.message_number)?;
        let chain_key = self
            .receiving_chain_key
            .take()
            .ok_or(CoreError::MissingChainState)?;
        let (next_chain_key, message_key) = derive_chain_key_and_message_key(&chain_key)?;
        self.receiving_chain_key = Some(next_chain_key);
        self.received_messages += 1;

        decrypt_ratchet_message(&message_key, &header, envelope, associated_data)
    }

    pub fn local_ratchet_public_key(&self) -> PublicKey {
        self.local_ratchet_key.public_key
    }

    pub fn remote_ratchet_public_key(&self) -> Option<PublicKey> {
        self.remote_ratchet_key
    }

    pub fn persist(&self) -> PersistedDoubleRatchetSession {
        PersistedDoubleRatchetSession {
            version: DOUBLE_RATCHET_VERSION.to_string(),
            root_key_b64: Base64::encode_string(&self.root_key),
            local_ratchet_secret_key_b64: Base64::encode_string(&self.local_ratchet_key.secret_key.to_bytes()),
            local_ratchet_public_key_b64: Base64::encode_string(self.local_ratchet_key.public_key.as_bytes()),
            remote_ratchet_public_key_b64: self
                .remote_ratchet_key
                .map(|value| Base64::encode_string(value.as_bytes())),
            sending_chain_key_b64: self
                .sending_chain_key
                .map(|value| Base64::encode_string(&value)),
            receiving_chain_key_b64: self
                .receiving_chain_key
                .map(|value| Base64::encode_string(&value)),
            previous_chain_length: self.previous_chain_length,
            sent_messages: self.sent_messages,
            received_messages: self.received_messages,
            skipped_keys: self
                .skipped_keys
                .iter()
                .map(|(id, message_key)| PersistedSkippedMessageKey {
                    ratchet_public_key_b64: Base64::encode_string(&id.ratchet_public_key),
                    message_number: id.message_number,
                    message_key_b64: Base64::encode_string(message_key),
                })
                .collect(),
        }
    }

    pub fn from_persisted(value: &PersistedDoubleRatchetSession) -> Result<Self, CoreError> {
        if value.version != DOUBLE_RATCHET_VERSION {
            return Err(CoreError::UnsupportedVersion);
        }

        let local_ratchet_secret_key = StaticSecret::from(decode_fixed_base64::<32>(&value.local_ratchet_secret_key_b64)?);
        let local_ratchet_public_key = PublicKey::from(decode_fixed_base64::<32>(&value.local_ratchet_public_key_b64)?);
        let local_ratchet_key = RatchetKeyPair {
            secret_key: local_ratchet_secret_key,
            public_key: local_ratchet_public_key,
        };
        let mut skipped_keys = BTreeMap::new();
        for skipped in &value.skipped_keys {
            if skipped_keys.len() >= MAX_SKIPPED_MESSAGE_KEYS {
                return Err(CoreError::TooManySkippedMessages);
            }
            skipped_keys.insert(
                SkippedKeyId {
                    ratchet_public_key: decode_fixed_base64::<32>(&skipped.ratchet_public_key_b64)?,
                    message_number: skipped.message_number,
                },
                decode_fixed_base64::<32>(&skipped.message_key_b64)?,
            );
        }

        Ok(Self {
            root_key: decode_fixed_base64::<32>(&value.root_key_b64)?,
            local_ratchet_key,
            remote_ratchet_key: value
                .remote_ratchet_public_key_b64
                .as_deref()
                .map(|encoded| decode_fixed_base64::<32>(encoded).map(PublicKey::from))
                .transpose()?,
            sending_chain_key: value
                .sending_chain_key_b64
                .as_deref()
                .map(decode_fixed_base64::<32>)
                .transpose()?,
            receiving_chain_key: value
                .receiving_chain_key_b64
                .as_deref()
                .map(decode_fixed_base64::<32>)
                .transpose()?,
            previous_chain_length: value.previous_chain_length,
            sent_messages: value.sent_messages,
            received_messages: value.received_messages,
            skipped_keys,
        })
    }
}

pub fn bootstrap_initiator_session(
    handshake: InitiatorHandshake,
) -> Result<DoubleRatchetSession, CoreError> {
    let local_ratchet_key = RatchetKeyPair::from_secret(handshake.sender_ephemeral_secret);
    let remote_ratchet_key = handshake.message_keys.receiver_signed_prekey;
    let (root_key, sending_chain_key) = derive_root_and_chain_key(
        &handshake.shared_secret,
        local_ratchet_key
            .secret_key
            .diffie_hellman(&remote_ratchet_key)
            .as_bytes(),
    )?;

    Ok(DoubleRatchetSession {
        root_key,
        local_ratchet_key,
        remote_ratchet_key: Some(remote_ratchet_key),
        sending_chain_key: Some(sending_chain_key),
        receiving_chain_key: None,
        previous_chain_length: 0,
        sent_messages: 0,
        received_messages: 0,
        skipped_keys: BTreeMap::new(),
    })
}

pub fn bootstrap_responder_session(
    receiver_signed_prekey: &SignedPreKey,
    opened_initial_message: &OpenedInitialMessage,
) -> Result<DoubleRatchetSession, CoreError> {
    if opened_initial_message.message_keys.receiver_signed_prekey != receiver_signed_prekey.public_key {
        return Err(CoreError::InvalidPreKeyBundle);
    }

    let local_ratchet_key = RatchetKeyPair::from_secret(StaticSecret::from(
        receiver_signed_prekey.secret_key.to_bytes(),
    ));
    let remote_ratchet_key = opened_initial_message.message_keys.sender_ephemeral_key;
    let (root_key, receiving_chain_key) = derive_root_and_chain_key(
        &opened_initial_message.shared_secret,
        local_ratchet_key
            .secret_key
            .diffie_hellman(&remote_ratchet_key)
            .as_bytes(),
    )?;

    Ok(DoubleRatchetSession {
        root_key,
        local_ratchet_key,
        remote_ratchet_key: Some(remote_ratchet_key),
        sending_chain_key: None,
        receiving_chain_key: Some(receiving_chain_key),
        previous_chain_length: 0,
        sent_messages: 0,
        received_messages: 0,
        skipped_keys: BTreeMap::new(),
    })
}

impl DoubleRatchetSession {
    fn ensure_sending_chain(&mut self) -> Result<(), CoreError> {
        if self.sending_chain_key.is_some() {
            return Ok(());
        }

        let remote_ratchet_key = self.remote_ratchet_key.ok_or(CoreError::MissingChainState)?;
        self.previous_chain_length = self.sent_messages;
        self.sent_messages = 0;
        self.local_ratchet_key = RatchetKeyPair::generate();
        let (root_key, sending_chain_key) = derive_root_and_chain_key(
            &self.root_key,
            self.local_ratchet_key
                .secret_key
                .diffie_hellman(&remote_ratchet_key)
                .as_bytes(),
        )?;
        self.root_key = root_key;
        self.sending_chain_key = Some(sending_chain_key);
        Ok(())
    }

    fn apply_remote_ratchet(&mut self, remote_ratchet_key: PublicKey) -> Result<(), CoreError> {
        self.previous_chain_length = self.sent_messages;
        self.sent_messages = 0;
        self.received_messages = 0;
        let (root_key, receiving_chain_key) = derive_root_and_chain_key(
            &self.root_key,
            self.local_ratchet_key
                .secret_key
                .diffie_hellman(&remote_ratchet_key)
                .as_bytes(),
        )?;
        self.root_key = root_key;
        self.remote_ratchet_key = Some(remote_ratchet_key);
        self.receiving_chain_key = Some(receiving_chain_key);
        self.sending_chain_key = None;
        Ok(())
    }

    fn skip_message_keys(&mut self, until_message_number: u32) -> Result<(), CoreError> {
        if until_message_number < self.received_messages {
            return Ok(());
        }

        if until_message_number == self.received_messages {
            return Ok(());
        }

        if (until_message_number - self.received_messages) as usize > MAX_SKIPPED_MESSAGE_KEYS {
            return Err(CoreError::TooManySkippedMessages);
        }

        let remote_ratchet_key = self.remote_ratchet_key.ok_or(CoreError::MissingChainState)?;
        let mut chain_key = self
            .receiving_chain_key
            .take()
            .ok_or(CoreError::MissingChainState)?;

        while self.received_messages < until_message_number {
            let (next_chain_key, message_key) = derive_chain_key_and_message_key(&chain_key)?;
            if self.skipped_keys.len() >= MAX_SKIPPED_MESSAGE_KEYS {
                return Err(CoreError::TooManySkippedMessages);
            }
            self.skipped_keys.insert(
                SkippedKeyId {
                    ratchet_public_key: *remote_ratchet_key.as_bytes(),
                    message_number: self.received_messages,
                },
                message_key,
            );
            chain_key = next_chain_key;
            self.received_messages += 1;
        }

        self.receiving_chain_key = Some(chain_key);
        Ok(())
    }

    fn try_skipped_message_key(
        &mut self,
        header: &ParsedRatchetHeader,
        envelope: &RatchetMessageEnvelope,
        associated_data: &[u8],
    ) -> Result<Option<Vec<u8>>, CoreError> {
        let key = SkippedKeyId {
            ratchet_public_key: *header.ratchet_public_key.as_bytes(),
            message_number: header.message_number,
        };
        if let Some(message_key) = self.skipped_keys.remove(&key) {
            return decrypt_ratchet_message(&message_key, header, envelope, associated_data).map(Some);
        }

        Ok(None)
    }
}

fn derive_root_and_chain_key(
    root_key: &[u8; 32],
    dh_output: &[u8],
) -> Result<([u8; 32], [u8; 32]), CoreError> {
    let hkdf = Hkdf::<Sha256>::new(Some(root_key), dh_output);
    let mut output = [0u8; 64];
    hkdf.expand(ROOT_INFO, &mut output)
        .map_err(|_| CoreError::KeyDerivationFailure)?;

    let (new_root_key, chain_key) = output.split_at(32);
    Ok((
        new_root_key
            .try_into()
            .map_err(|_| CoreError::KeyDerivationFailure)?,
        chain_key
            .try_into()
            .map_err(|_| CoreError::KeyDerivationFailure)?,
    ))
}

fn derive_chain_key_and_message_key(
    chain_key: &[u8; 32],
) -> Result<([u8; 32], [u8; 32]), CoreError> {
    let hkdf = Hkdf::<Sha256>::new(Some(chain_key), chain_key);
    let mut output = [0u8; 64];
    hkdf.expand(CHAIN_INFO, &mut output)
        .map_err(|_| CoreError::KeyDerivationFailure)?;

    let (next_chain_key, message_key) = output.split_at(32);
    Ok((
        next_chain_key
            .try_into()
            .map_err(|_| CoreError::KeyDerivationFailure)?,
        message_key
            .try_into()
            .map_err(|_| CoreError::KeyDerivationFailure)?,
    ))
}

fn parse_ratchet_header(header: &RatchetHeader) -> Result<ParsedRatchetHeader, CoreError> {
    Ok(ParsedRatchetHeader {
        ratchet_public_key: PublicKey::from(decode_fixed_base64::<32>(&header.ratchet_public_key_b64)?),
        previous_chain_length: header.previous_chain_length,
        message_number: header.message_number,
    })
}

fn build_ratchet_aad(header: &RatchetHeader, associated_data: &[u8]) -> Vec<u8> {
    let mut aad = Vec::with_capacity(
        DOUBLE_RATCHET_VERSION.len() + header.ratchet_public_key_b64.len() + associated_data.len() + 16,
    );
    aad.extend_from_slice(DOUBLE_RATCHET_VERSION.as_bytes());
    aad.extend_from_slice(header.ratchet_public_key_b64.as_bytes());
    aad.extend_from_slice(&header.previous_chain_length.to_be_bytes());
    aad.extend_from_slice(&header.message_number.to_be_bytes());
    aad.extend_from_slice(associated_data);
    aad
}

fn decrypt_ratchet_message(
    message_key: &[u8; 32],
    header: &impl RatchetHeaderLike,
    envelope: &RatchetMessageEnvelope,
    associated_data: &[u8],
) -> Result<Vec<u8>, CoreError> {
    let nonce = decode_fixed_base64::<24>(&envelope.nonce_b64)?;
    let ciphertext = Base64::decode_vec(&envelope.ciphertext_b64).map_err(|_| CoreError::InvalidBase64)?;
    if ciphertext.is_empty() {
        return Err(CoreError::InvalidRatchetMessage);
    }
    let cipher = XChaCha20Poly1305::new(message_key.into());
    let aad = build_ratchet_aad(&header.to_header(), associated_data);
    cipher
        .decrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &ciphertext,
                aad: &aad,
            },
        )
        .map_err(|_| CoreError::CryptoFailure)
}

trait RatchetHeaderLike {
    fn to_header(&self) -> RatchetHeader;
}

impl RatchetHeaderLike for RatchetHeader {
    fn to_header(&self) -> RatchetHeader {
        self.clone()
    }
}

impl RatchetHeaderLike for ParsedRatchetHeader {
    fn to_header(&self) -> RatchetHeader {
        RatchetHeader {
            ratchet_public_key_b64: Base64::encode_string(self.ratchet_public_key.as_bytes()),
            previous_chain_length: self.previous_chain_length,
            message_number: self.message_number,
        }
    }
}

fn decode_fixed_base64<const N: usize>(value: &str) -> Result<[u8; N], CoreError> {
    let decoded = Base64::decode_vec(value).map_err(|_| CoreError::InvalidBase64)?;
    decoded.try_into().map_err(|_| CoreError::InvalidRatchetMessage)
}

fn validate_session_id(session_id: &str) -> Result<(), CoreError> {
    if session_id.trim().is_empty() {
        return Err(CoreError::InvalidRatchetSessionId);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{open_initial_message, seal_initial_message, IdentityKeyPair, OneTimePreKey, PreKeyBundle, RelayContentKind};

    #[test]
    fn bootstrap_sessions_exchange_messages_bidirectionally() {
        let alice_identity = IdentityKeyPair::generate();
        let bob_identity = IdentityKeyPair::generate();
        let bob_signed_prekey = SignedPreKey::generate(&bob_identity);
        let bob_one_time_prekey = OneTimePreKey::generate();
        let bob_bundle = PreKeyBundle::from_parts(
            &bob_identity,
            &bob_signed_prekey,
            Some(&bob_one_time_prekey),
        );

        let bootstrap_plaintext = b"hello bob";
        let (initial_envelope, alice_handshake) = seal_initial_message(
            &alice_identity,
            &bob_bundle,
            bootstrap_plaintext,
            RelayContentKind::SessionBootstrap,
        )
        .unwrap();
        let opened = open_initial_message(
            &bob_identity,
            &bob_signed_prekey,
            Some(&bob_one_time_prekey),
            &initial_envelope,
        )
        .unwrap();

        let mut alice_session = bootstrap_initiator_session(alice_handshake).unwrap();
        let mut bob_session = bootstrap_responder_session(&bob_signed_prekey, &opened).unwrap();

        let bob_reply = bob_session.encrypt(b"hello alice", b"chat-1").unwrap();
        let opened_by_alice = alice_session.decrypt(&bob_reply, b"chat-1").unwrap();
        assert_eq!(opened_by_alice, b"hello alice");

        let alice_reply = alice_session.encrypt(b"ack", b"chat-1").unwrap();
        let opened_by_bob = bob_session.decrypt(&alice_reply, b"chat-1").unwrap();
        assert_eq!(opened_by_bob, b"ack");
    }

    #[test]
    fn skipped_message_key_allows_out_of_order_delivery() {
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
        let opened = open_initial_message(&bob_identity, &bob_signed_prekey, None, &initial_envelope).unwrap();
        let mut alice_session = bootstrap_initiator_session(alice_handshake).unwrap();
        let mut bob_session = bootstrap_responder_session(&bob_signed_prekey, &opened).unwrap();

        let bob_msg_1 = bob_session.encrypt(b"message-one", b"chat-1").unwrap();
        let bob_msg_2 = bob_session.encrypt(b"message-two", b"chat-1").unwrap();

        let opened_second = alice_session.decrypt(&bob_msg_2, b"chat-1").unwrap();
        let opened_first = alice_session.decrypt(&bob_msg_1, b"chat-1").unwrap();

        assert_eq!(opened_second, b"message-two");
        assert_eq!(opened_first, b"message-one");
    }

    #[test]
    fn decrypt_rejects_wrong_associated_data() {
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
        let opened = open_initial_message(&bob_identity, &bob_signed_prekey, None, &initial_envelope).unwrap();
        let mut alice_session = bootstrap_initiator_session(alice_handshake).unwrap();
        let mut bob_session = bootstrap_responder_session(&bob_signed_prekey, &opened).unwrap();

        let bob_reply = bob_session.encrypt(b"hello alice", b"chat-1").unwrap();
        let decrypted = alice_session.decrypt(&bob_reply, b"wrong-chat");

        assert!(matches!(decrypted, Err(CoreError::CryptoFailure)));
    }

    #[test]
    fn persisted_session_round_trips() {
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
        let opened = open_initial_message(&bob_identity, &bob_signed_prekey, None, &initial_envelope).unwrap();
        let alice_session = bootstrap_initiator_session(alice_handshake).unwrap();
        let persisted = alice_session.persist();
        let restored = DoubleRatchetSession::from_persisted(&persisted).unwrap();

        assert_eq!(restored.local_ratchet_public_key(), alice_session.local_ratchet_public_key());
        assert_eq!(restored.remote_ratchet_public_key(), alice_session.remote_ratchet_public_key());
        assert_eq!(opened.content_kind, RelayContentKind::SessionBootstrap);
    }

    #[test]
    fn stored_session_restores_and_continues_ratchet_flow() {
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
        let opened = open_initial_message(&bob_identity, &bob_signed_prekey, None, &initial_envelope).unwrap();
        let mut alice_session = bootstrap_initiator_session(alice_handshake).unwrap();
        let mut bob_session = bootstrap_responder_session(&bob_signed_prekey, &opened).unwrap();
        let store = InMemoryDoubleRatchetSessionStore::new();
        let session_id = "alice:bob:primary";

        let inbound = bob_session.encrypt(b"first-message", b"chat-1").unwrap();
        let opened_first = alice_session.decrypt(&inbound, b"chat-1").unwrap();
        assert_eq!(opened_first, b"first-message");

        alice_session.save_to_store(&store, session_id).unwrap();
        let mut restored = DoubleRatchetSession::load_from_store(&store, session_id)
            .unwrap()
            .unwrap();

        let outbound = restored.encrypt(b"reply-after-restore", b"chat-1").unwrap();
        let opened_by_bob = bob_session.decrypt(&outbound, b"chat-1").unwrap();
        assert_eq!(opened_by_bob, b"reply-after-restore");
    }

    #[test]
    fn session_store_delete_removes_saved_session() {
        let store = InMemoryDoubleRatchetSessionStore::new();
        let session = PersistedDoubleRatchetSession {
            version: DOUBLE_RATCHET_VERSION.to_string(),
            root_key_b64: Base64::encode_string(&[7u8; 32]),
            local_ratchet_secret_key_b64: Base64::encode_string(&[8u8; 32]),
            local_ratchet_public_key_b64: Base64::encode_string(&[9u8; 32]),
            remote_ratchet_public_key_b64: None,
            sending_chain_key_b64: None,
            receiving_chain_key_b64: None,
            previous_chain_length: 0,
            sent_messages: 0,
            received_messages: 0,
            skipped_keys: Vec::new(),
        };

        store.save_session("session-1", &session).unwrap();
        assert!(store.load_session("session-1").unwrap().is_some());

        store.delete_session("session-1").unwrap();
        assert!(store.load_session("session-1").unwrap().is_none());
    }

    #[test]
    fn session_store_rejects_blank_session_id() {
        let store = InMemoryDoubleRatchetSessionStore::new();
        let result = store.load_session("   ");

        assert!(matches!(result, Err(CoreError::InvalidRatchetSessionId)));
    }
}