use base64ct::{Base64, Encoding};
use chacha20poly1305::{aead::{Aead, Payload}, KeyInit, XChaCha20Poly1305, XNonce};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use hkdf::Hkdf;
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::{CoreError, RelayContentKind};

const X3DH_INFO: &[u8] = b"simy-x3dh-v1";
const X3DH_F: [u8; 32] = [0xFF; 32];
const INITIAL_MESSAGE_VERSION: &str = "simy-x3dh-initial-v1";

pub struct IdentityKeyPair {
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
    pub exchange_secret_key: StaticSecret,
    pub exchange_public_key: PublicKey,
}

impl core::fmt::Debug for IdentityKeyPair {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("IdentityKeyPair")
            .field("verifying_key", &self.verifying_key)
            .field("exchange_public_key", &self.exchange_public_key)
            .field("signing_key", &"<redacted>")
            .field("exchange_secret_key", &"<redacted>")
            .finish()
    }
}

impl IdentityKeyPair {
    pub fn generate() -> Self {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();
        let exchange_secret_key = StaticSecret::random_from_rng(&mut csprng);
        let exchange_public_key = PublicKey::from(&exchange_secret_key);

        Self {
            signing_key,
            verifying_key,
            exchange_secret_key,
            exchange_public_key,
        }
    }

    pub fn public_bundle(&self) -> IdentityPublicKeys {
        IdentityPublicKeys {
            signing_key: self.verifying_key,
            exchange_key: self.exchange_public_key,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdentityPublicKeys {
    pub signing_key: VerifyingKey,
    pub exchange_key: PublicKey,
}

pub struct SignedPreKey {
    pub secret_key: StaticSecret,
    pub public_key: PublicKey,
    pub signature: Signature,
}

impl core::fmt::Debug for SignedPreKey {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("SignedPreKey")
            .field("public_key", &self.public_key)
            .field("signature", &self.signature)
            .field("secret_key", &"<redacted>")
            .finish()
    }
}

impl SignedPreKey {
    pub fn generate(identity: &IdentityKeyPair) -> Self {
        let mut csprng = OsRng;
        let secret_key = StaticSecret::random_from_rng(&mut csprng);
        let public_key = PublicKey::from(&secret_key);
        let signature = identity.signing_key.sign(public_key.as_bytes());

        Self {
            secret_key,
            public_key,
            signature,
        }
    }

    pub fn verify(&self, identity_public_key: &VerifyingKey) -> Result<(), CoreError> {
        identity_public_key
            .verify(self.public_key.as_bytes(), &self.signature)
            .map_err(|_| CoreError::InvalidSignature)
    }
}

pub struct OneTimePreKey {
    pub secret_key: StaticSecret,
    pub public_key: PublicKey,
}

impl core::fmt::Debug for OneTimePreKey {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("OneTimePreKey")
            .field("public_key", &self.public_key)
            .field("secret_key", &"<redacted>")
            .finish()
    }
}

impl OneTimePreKey {
    pub fn generate() -> Self {
        let mut csprng = OsRng;
        let secret_key = StaticSecret::random_from_rng(&mut csprng);
        let public_key = PublicKey::from(&secret_key);

        Self {
            secret_key,
            public_key,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PreKeyBundle {
    pub identity_pub: IdentityPublicKeys,
    pub signed_prekey_pub: PublicKey,
    pub signed_prekey_sig: Signature,
    pub one_time_prekey_pub: Option<PublicKey>,
}

impl PreKeyBundle {
    pub fn from_parts(
        identity: &IdentityKeyPair,
        signed_prekey: &SignedPreKey,
        one_time_prekey: Option<&OneTimePreKey>,
    ) -> Self {
        Self {
            identity_pub: identity.public_bundle(),
            signed_prekey_pub: signed_prekey.public_key,
            signed_prekey_sig: signed_prekey.signature,
            one_time_prekey_pub: one_time_prekey.map(|key| key.public_key),
        }
    }

    pub fn verify(&self) -> Result<(), CoreError> {
        self.identity_pub
            .signing_key
            .verify(self.signed_prekey_pub.as_bytes(), &self.signed_prekey_sig)
            .map_err(|_| CoreError::InvalidSignature)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InitialMessageKeys {
    pub sender_identity: IdentityPublicKeys,
    pub sender_ephemeral_key: PublicKey,
    pub receiver_signed_prekey: PublicKey,
    pub receiver_one_time_prekey: Option<PublicKey>,
}

pub struct InitiatorHandshake {
    pub message_keys: InitialMessageKeys,
    pub shared_secret: [u8; 32],
    pub(crate) sender_ephemeral_secret: StaticSecret,
}

impl core::fmt::Debug for InitiatorHandshake {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("InitiatorHandshake")
            .field("message_keys", &self.message_keys)
            .field("shared_secret", &"<redacted>")
            .field("sender_ephemeral_secret", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct X3dhInitialMessageEnvelope {
    pub version: String,
    pub content_kind: RelayContentKind,
    pub sender_identity_signing_key_b64: String,
    pub sender_identity_exchange_key_b64: String,
    pub sender_ephemeral_key_b64: String,
    pub receiver_signed_prekey_b64: String,
    pub receiver_one_time_prekey_b64: Option<String>,
    pub nonce_b64: String,
    pub ciphertext_b64: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenedInitialMessage {
    pub content_kind: RelayContentKind,
    pub sender_identity: IdentityPublicKeys,
    pub message_keys: InitialMessageKeys,
    pub plaintext: Vec<u8>,
    pub shared_secret: [u8; 32],
}

pub fn initiate_x3dh(
    sender_identity: &IdentityKeyPair,
    receiver_bundle: &PreKeyBundle,
) -> Result<InitiatorHandshake, CoreError> {
    receiver_bundle.verify()?;

    let mut csprng = OsRng;
    let sender_ephemeral_secret = StaticSecret::random_from_rng(&mut csprng);
    let sender_ephemeral_key = PublicKey::from(&sender_ephemeral_secret);

    let dh1 = sender_identity
        .exchange_secret_key
        .diffie_hellman(&receiver_bundle.signed_prekey_pub);
    let dh2 = sender_ephemeral_secret.diffie_hellman(&receiver_bundle.identity_pub.exchange_key);
    let mut ikm = Vec::with_capacity(if receiver_bundle.one_time_prekey_pub.is_some() {
        32 * 5
    } else {
        32 * 4
    });
    ikm.extend_from_slice(&X3DH_F);
    ikm.extend_from_slice(dh1.as_bytes());
    ikm.extend_from_slice(dh2.as_bytes());

    let dh3 = sender_ephemeral_secret.diffie_hellman(&receiver_bundle.signed_prekey_pub);
    ikm.extend_from_slice(dh3.as_bytes());

    if let Some(one_time_prekey_pub) = receiver_bundle.one_time_prekey_pub {
        let dh4 = sender_ephemeral_secret.diffie_hellman(&one_time_prekey_pub);
        ikm.extend_from_slice(dh4.as_bytes());
    }

    let shared_secret = derive_shared_secret(&ikm)?;

    Ok(InitiatorHandshake {
        message_keys: InitialMessageKeys {
            sender_identity: sender_identity.public_bundle(),
            sender_ephemeral_key,
            receiver_signed_prekey: receiver_bundle.signed_prekey_pub,
            receiver_one_time_prekey: receiver_bundle.one_time_prekey_pub,
        },
        shared_secret,
        sender_ephemeral_secret,
    })
}

pub fn respond_x3dh(
    receiver_identity: &IdentityKeyPair,
    receiver_signed_prekey: &SignedPreKey,
    receiver_one_time_prekey: Option<&OneTimePreKey>,
    initial_message: &InitialMessageKeys,
) -> Result<[u8; 32], CoreError> {
    let mut ikm = Vec::with_capacity(if initial_message.receiver_one_time_prekey.is_some() {
        32 * 5
    } else {
        32 * 4
    });
    ikm.extend_from_slice(&X3DH_F);

    let dh1 = receiver_signed_prekey
        .secret_key
        .diffie_hellman(&initial_message.sender_identity.exchange_key);
    let dh2 = receiver_identity
        .exchange_secret_key
        .diffie_hellman(&initial_message.sender_ephemeral_key);
    let dh3 = receiver_signed_prekey
        .secret_key
        .diffie_hellman(&initial_message.sender_ephemeral_key);

    ikm.extend_from_slice(dh1.as_bytes());
    ikm.extend_from_slice(dh2.as_bytes());
    ikm.extend_from_slice(dh3.as_bytes());

    match (
        initial_message.receiver_one_time_prekey,
        receiver_one_time_prekey,
    ) {
        (Some(expected_public_key), Some(one_time_prekey)) => {
            if one_time_prekey.public_key != expected_public_key {
                return Err(CoreError::InvalidPreKeyBundle);
            }
            let dh4 = one_time_prekey
                .secret_key
                .diffie_hellman(&initial_message.sender_ephemeral_key);
            ikm.extend_from_slice(dh4.as_bytes());
        }
        (Some(_), None) => return Err(CoreError::MissingOneTimePreKey),
        (None, Some(_)) | (None, None) => {}
    }

    derive_shared_secret(&ikm)
}

pub fn seal_initial_message(
    sender_identity: &IdentityKeyPair,
    receiver_bundle: &PreKeyBundle,
    plaintext: &[u8],
    content_kind: RelayContentKind,
) -> Result<(X3dhInitialMessageEnvelope, InitiatorHandshake), CoreError> {
    let handshake = initiate_x3dh(sender_identity, receiver_bundle)?;
    let mut nonce = [0u8; 24];
    let mut csprng = OsRng;
    use rand_core::RngCore;
    csprng.fill_bytes(&mut nonce);

    let cipher = XChaCha20Poly1305::new((&handshake.shared_secret).into());
    let aad = build_initial_message_aad(&handshake.message_keys, &content_kind);
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: &aad,
            },
        )
        .map_err(|_| CoreError::CryptoFailure)?;

    Ok((
        X3dhInitialMessageEnvelope {
            version: INITIAL_MESSAGE_VERSION.to_string(),
            content_kind,
            sender_identity_signing_key_b64: Base64::encode_string(
                &handshake.message_keys.sender_identity.signing_key.to_bytes(),
            ),
            sender_identity_exchange_key_b64: Base64::encode_string(
                handshake.message_keys.sender_identity.exchange_key.as_bytes(),
            ),
            sender_ephemeral_key_b64: Base64::encode_string(
                handshake.message_keys.sender_ephemeral_key.as_bytes(),
            ),
            receiver_signed_prekey_b64: Base64::encode_string(
                handshake.message_keys.receiver_signed_prekey.as_bytes(),
            ),
            receiver_one_time_prekey_b64: handshake
                .message_keys
                .receiver_one_time_prekey
                .map(|value| Base64::encode_string(value.as_bytes())),
            nonce_b64: Base64::encode_string(&nonce),
            ciphertext_b64: Base64::encode_string(&ciphertext),
            created_at: Utc::now(),
        },
        handshake,
    ))
}

pub fn open_initial_message(
    receiver_identity: &IdentityKeyPair,
    receiver_signed_prekey: &SignedPreKey,
    receiver_one_time_prekey: Option<&OneTimePreKey>,
    envelope: &X3dhInitialMessageEnvelope,
) -> Result<OpenedInitialMessage, CoreError> {
    if envelope.version != INITIAL_MESSAGE_VERSION {
        return Err(CoreError::UnsupportedVersion);
    }

    let message_keys = parse_initial_message_keys(envelope)?;
    if message_keys.receiver_signed_prekey != receiver_signed_prekey.public_key {
        return Err(CoreError::InvalidPreKeyBundle);
    }

    let shared_secret = respond_x3dh(
        receiver_identity,
        receiver_signed_prekey,
        receiver_one_time_prekey,
        &message_keys,
    )?;
    let nonce = decode_fixed_base64::<24>(&envelope.nonce_b64)?;
    let ciphertext = Base64::decode_vec(&envelope.ciphertext_b64).map_err(|_| CoreError::InvalidBase64)?;
    if ciphertext.is_empty() {
        return Err(CoreError::InvalidEnvelope);
    }

    let cipher = XChaCha20Poly1305::new((&shared_secret).into());
    let aad = build_initial_message_aad(&message_keys, &envelope.content_kind);
    let plaintext = cipher
        .decrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &ciphertext,
                aad: &aad,
            },
        )
        .map_err(|_| CoreError::CryptoFailure)?;

    Ok(OpenedInitialMessage {
        content_kind: envelope.content_kind.clone(),
        sender_identity: message_keys.sender_identity,
        message_keys,
        plaintext,
        shared_secret,
    })
}

fn derive_shared_secret(input_key_material: &[u8]) -> Result<[u8; 32], CoreError> {
    let hkdf = Hkdf::<Sha256>::new(None, input_key_material);
    let mut output = [0u8; 32];
    hkdf.expand(X3DH_INFO, &mut output)
        .map_err(|_| CoreError::KeyDerivationFailure)?;
    Ok(output)
}

fn parse_initial_message_keys(
    envelope: &X3dhInitialMessageEnvelope,
) -> Result<InitialMessageKeys, CoreError> {
    let sender_identity_signing_key = decode_ed25519_public_key(&envelope.sender_identity_signing_key_b64)?;
    let sender_identity_exchange_key = decode_x25519_public_key(&envelope.sender_identity_exchange_key_b64)?;
    let sender_ephemeral_key = decode_x25519_public_key(&envelope.sender_ephemeral_key_b64)?;
    let receiver_signed_prekey = decode_x25519_public_key(&envelope.receiver_signed_prekey_b64)?;
    let receiver_one_time_prekey = envelope
        .receiver_one_time_prekey_b64
        .as_deref()
        .map(decode_x25519_public_key)
        .transpose()?;

    Ok(InitialMessageKeys {
        sender_identity: IdentityPublicKeys {
            signing_key: sender_identity_signing_key,
            exchange_key: sender_identity_exchange_key,
        },
        sender_ephemeral_key,
        receiver_signed_prekey,
        receiver_one_time_prekey,
    })
}

fn build_initial_message_aad(
    message_keys: &InitialMessageKeys,
    content_kind: &RelayContentKind,
) -> Vec<u8> {
    let mut aad = Vec::with_capacity(32 * 4 + 2 + 32 + INITIAL_MESSAGE_VERSION.len());
    aad.extend_from_slice(INITIAL_MESSAGE_VERSION.as_bytes());
    aad.push(relay_content_kind_tag(content_kind));
    aad.extend_from_slice(&message_keys.sender_identity.signing_key.to_bytes());
    aad.extend_from_slice(message_keys.sender_identity.exchange_key.as_bytes());
    aad.extend_from_slice(message_keys.sender_ephemeral_key.as_bytes());
    aad.extend_from_slice(message_keys.receiver_signed_prekey.as_bytes());
    match message_keys.receiver_one_time_prekey {
        Some(value) => {
            aad.push(1);
            aad.extend_from_slice(value.as_bytes());
        }
        None => aad.push(0),
    }
    aad
}

fn relay_content_kind_tag(kind: &RelayContentKind) -> u8 {
    match kind {
        RelayContentKind::DirectMessage => 1,
        RelayContentKind::SessionBootstrap => 2,
        RelayContentKind::MediaPointer => 3,
        RelayContentKind::FeedPost => 4,
    }
}

fn decode_ed25519_public_key(value: &str) -> Result<VerifyingKey, CoreError> {
    let bytes = decode_fixed_base64::<32>(value)?;
    VerifyingKey::from_bytes(&bytes).map_err(|_| CoreError::InvalidPublicKey)
}

fn decode_x25519_public_key(value: &str) -> Result<PublicKey, CoreError> {
    Ok(PublicKey::from(decode_fixed_base64::<32>(value)?))
}

fn decode_fixed_base64<const N: usize>(value: &str) -> Result<[u8; N], CoreError> {
    let decoded = Base64::decode_vec(value).map_err(|_| CoreError::InvalidBase64)?;
    decoded.try_into().map_err(|_| CoreError::InvalidEnvelope)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_prekey_signature_verifies() {
        let identity = IdentityKeyPair::generate();
        let signed_prekey = SignedPreKey::generate(&identity);

        assert!(signed_prekey.verify(&identity.verifying_key).is_ok());
    }

    #[test]
    fn prekey_bundle_verification_rejects_tampering() {
        let identity = IdentityKeyPair::generate();
        let signed_prekey = SignedPreKey::generate(&identity);
        let attacker = IdentityKeyPair::generate();
        let bundle = PreKeyBundle {
            identity_pub: attacker.public_bundle(),
            signed_prekey_pub: signed_prekey.public_key,
            signed_prekey_sig: signed_prekey.signature,
            one_time_prekey_pub: None,
        };

        assert!(matches!(bundle.verify(), Err(CoreError::InvalidSignature)));
    }

    #[test]
    fn x3dh_round_trip_with_one_time_prekey() {
        let sender_identity = IdentityKeyPair::generate();
        let receiver_identity = IdentityKeyPair::generate();
        let receiver_signed_prekey = SignedPreKey::generate(&receiver_identity);
        let receiver_one_time_prekey = OneTimePreKey::generate();
        let bundle = PreKeyBundle::from_parts(
            &receiver_identity,
            &receiver_signed_prekey,
            Some(&receiver_one_time_prekey),
        );

        let initiator = initiate_x3dh(&sender_identity, &bundle).unwrap();
        let responder = respond_x3dh(
            &receiver_identity,
            &receiver_signed_prekey,
            Some(&receiver_one_time_prekey),
            &initiator.message_keys,
        )
        .unwrap();

        assert_eq!(initiator.shared_secret, responder);
    }

    #[test]
    fn x3dh_round_trip_without_one_time_prekey() {
        let sender_identity = IdentityKeyPair::generate();
        let receiver_identity = IdentityKeyPair::generate();
        let receiver_signed_prekey = SignedPreKey::generate(&receiver_identity);
        let bundle = PreKeyBundle::from_parts(&receiver_identity, &receiver_signed_prekey, None);

        let initiator = initiate_x3dh(&sender_identity, &bundle).unwrap();
        let responder = respond_x3dh(
            &receiver_identity,
            &receiver_signed_prekey,
            None,
            &initiator.message_keys,
        )
        .unwrap();

        assert_eq!(initiator.shared_secret, responder);
    }

    #[test]
    fn responder_rejects_missing_one_time_prekey() {
        let sender_identity = IdentityKeyPair::generate();
        let receiver_identity = IdentityKeyPair::generate();
        let receiver_signed_prekey = SignedPreKey::generate(&receiver_identity);
        let receiver_one_time_prekey = OneTimePreKey::generate();
        let bundle = PreKeyBundle::from_parts(
            &receiver_identity,
            &receiver_signed_prekey,
            Some(&receiver_one_time_prekey),
        );

        let initiator = initiate_x3dh(&sender_identity, &bundle).unwrap();
        let responder = respond_x3dh(
            &receiver_identity,
            &receiver_signed_prekey,
            None,
            &initiator.message_keys,
        );

        assert!(matches!(responder, Err(CoreError::MissingOneTimePreKey)));
    }

    #[test]
    fn initial_message_envelope_round_trips_with_one_time_prekey() {
        let sender_identity = IdentityKeyPair::generate();
        let receiver_identity = IdentityKeyPair::generate();
        let receiver_signed_prekey = SignedPreKey::generate(&receiver_identity);
        let receiver_one_time_prekey = OneTimePreKey::generate();
        let bundle = PreKeyBundle::from_parts(
            &receiver_identity,
            &receiver_signed_prekey,
            Some(&receiver_one_time_prekey),
        );

        let plaintext = b"bootstrap hello".to_vec();
        let (envelope, initiator) = seal_initial_message(
            &sender_identity,
            &bundle,
            &plaintext,
            RelayContentKind::SessionBootstrap,
        )
        .unwrap();
        let opened = open_initial_message(
            &receiver_identity,
            &receiver_signed_prekey,
            Some(&receiver_one_time_prekey),
            &envelope,
        )
        .unwrap();

        assert_eq!(opened.plaintext, plaintext);
        assert_eq!(opened.shared_secret, initiator.shared_secret);
        assert_eq!(opened.content_kind, RelayContentKind::SessionBootstrap);
        assert_eq!(opened.sender_identity, sender_identity.public_bundle());
    }

    #[test]
    fn initial_message_envelope_round_trips_without_one_time_prekey() {
        let sender_identity = IdentityKeyPair::generate();
        let receiver_identity = IdentityKeyPair::generate();
        let receiver_signed_prekey = SignedPreKey::generate(&receiver_identity);
        let bundle = PreKeyBundle::from_parts(&receiver_identity, &receiver_signed_prekey, None);

        let plaintext = b"bootstrap hello without opk".to_vec();
        let (envelope, initiator) = seal_initial_message(
            &sender_identity,
            &bundle,
            &plaintext,
            RelayContentKind::DirectMessage,
        )
        .unwrap();
        let opened = open_initial_message(
            &receiver_identity,
            &receiver_signed_prekey,
            None,
            &envelope,
        )
        .unwrap();

        assert_eq!(opened.plaintext, plaintext);
        assert_eq!(opened.shared_secret, initiator.shared_secret);
        assert_eq!(opened.content_kind, RelayContentKind::DirectMessage);
    }

    #[test]
    fn initial_message_rejects_wrong_receiver_signed_prekey() {
        let sender_identity = IdentityKeyPair::generate();
        let receiver_identity = IdentityKeyPair::generate();
        let receiver_signed_prekey = SignedPreKey::generate(&receiver_identity);
        let wrong_signed_prekey = SignedPreKey::generate(&receiver_identity);
        let bundle = PreKeyBundle::from_parts(&receiver_identity, &receiver_signed_prekey, None);
        let (envelope, _) = seal_initial_message(
            &sender_identity,
            &bundle,
            b"hello",
            RelayContentKind::SessionBootstrap,
        )
        .unwrap();

        assert!(matches!(
            open_initial_message(&receiver_identity, &wrong_signed_prekey, None, &envelope),
            Err(CoreError::InvalidPreKeyBundle)
        ));
    }
}