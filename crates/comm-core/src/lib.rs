use base64ct::{Base64, Encoding};
use chacha20poly1305::{
    aead::{Aead, Payload},
    KeyInit, XChaCha20Poly1305, XNonce,
};
use chrono::{DateTime, Utc};
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub mod x3dh;
pub mod double_ratchet;
pub mod device;
#[cfg(feature = "ratchet-store-fs")]
pub mod ratchet_store_fs;

pub use device::*;
pub use double_ratchet::*;
#[cfg(feature = "ratchet-store-fs")]
pub use ratchet_store_fs::*;
pub use x3dh::*;

pub const MEDIA_PADDING_BUCKETS: [u64; 5] = [
    1_048_576,
    5_242_880,
    10_485_760,
    26_214_400,
    52_428_800,
];

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid base64 public key")]
    InvalidBase64,
    #[error("invalid public key bytes")]
    InvalidPublicKey,
    #[error("media size must be greater than zero")]
    InvalidMediaSize,
    #[error("chunk size must be greater than zero")]
    InvalidChunkSize,
    #[error("media exceeds supported padding buckets")]
    MediaTooLarge,
    #[error("invalid media encryption key material")]
    InvalidKeyMaterial,
    #[error("invalid media nonce material")]
    InvalidNonceMaterial,
    #[error("media encryption failed")]
    CryptoFailure,
    #[error("chunk integrity verification failed")]
    IntegrityMismatch,
    #[error("invalid chunk sequence")]
    InvalidChunkSequence,
    #[error("invalid initial message envelope")]
    InvalidEnvelope,
    #[error("invalid ratchet message")]
    InvalidRatchetMessage,
    #[error("invalid device record")]
    InvalidDeviceRecord,
    #[error("invalid signed prekey signature")]
    InvalidSignature,
    #[error("invalid prekey bundle")]
    InvalidPreKeyBundle,
    #[error("missing required one-time prekey")]
    MissingOneTimePreKey,
    #[error("missing ratchet chain state")]
    MissingChainState,
    #[error("too many skipped messages")]
    TooManySkippedMessages,
    #[error("unsupported protocol version")]
    UnsupportedVersion,
    #[error("key derivation failed")]
    KeyDerivationFailure,
    #[error("invalid ratchet session id")]
    InvalidRatchetSessionId,
    #[error("ratchet session storage failure")]
    StorageFailure,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicIdentity {
    pub algorithm: String,
    pub public_key_b64: String,
    pub fingerprint: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct IdentityMaterial {
    signing_key: SigningKey,
    created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RelayContentKind {
    DirectMessage,
    SessionBootstrap,
    MediaPointer,
    FeedPost,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MediaEncryptionMaterial {
    pub algorithm: String,
    pub key_b64: String,
    pub base_nonce_b64: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlobPaddingPlan {
    pub original_size_bytes: u64,
    pub padded_size_bytes: u64,
    pub chunk_size_bytes: u32,
    pub chunk_count: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EncryptedBlobPointer {
    pub object_key: String,
    pub bucket: String,
    pub media_type: String,
    pub original_size_bytes: u64,
    pub padded_size_bytes: u64,
    pub chunk_size_bytes: u32,
    pub encryption_key_b64: String,
    pub nonce_b64: String,
    pub sha256_b64: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EncryptedMediaChunk {
    pub index: u32,
    pub ciphertext_b64: String,
    pub ciphertext_sha256_b64: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EncryptedMediaBlob {
    pub algorithm: String,
    pub original_size_bytes: u64,
    pub padded_size_bytes: u64,
    pub chunk_size_bytes: u32,
    pub ciphertext_sha256_b64: String,
    pub chunks: Vec<EncryptedMediaChunk>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FeedAudience {
    TrustedCircle,
    CustomGroup,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FeedReplyPolicy {
    NoReplies,
    ContactsOnly,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeedPostDescriptor {
    pub post_id: String,
    pub audience: FeedAudience,
    pub reply_policy: FeedReplyPolicy,
    pub caption_ciphertext_b64: Option<String>,
    pub media: Vec<EncryptedBlobPointer>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EncryptedContentEnvelope {
    pub kind: RelayContentKind,
    pub body_ciphertext_b64: String,
    pub media: Vec<EncryptedBlobPointer>,
    pub feed_post: Option<FeedPostDescriptor>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl IdentityMaterial {
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        Self {
            signing_key,
            created_at: Utc::now(),
        }
    }

    pub fn public_identity(&self) -> PublicIdentity {
        let verifying_key = self.signing_key.verifying_key();
        PublicIdentity {
            algorithm: "ed25519".to_string(),
            public_key_b64: Base64::encode_string(verifying_key.as_bytes()),
            fingerprint: fingerprint(verifying_key.as_bytes()),
            created_at: self.created_at,
        }
    }

    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }

    pub fn signing_key_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }
}

impl Drop for IdentityMaterial {
    fn drop(&mut self) {
        self.signing_key = SigningKey::from_bytes(&[0u8; 32]);
    }
}

pub fn fingerprint(public_key: &[u8]) -> String {
    let digest = Sha256::digest(public_key);
    let hex = hex::encode_upper(digest);
    let short = &hex[..60];

    short
        .as_bytes()
        .chunks(5)
        .map(|chunk| String::from_utf8_lossy(chunk).to_string())
        .collect::<Vec<_>>()
        .join("-")
}

pub fn decode_public_identity(value: &str) -> Result<VerifyingKey, CoreError> {
    let bytes = Base64::decode_vec(value).map_err(|_| CoreError::InvalidBase64)?;
    let array: [u8; 32] = bytes.try_into().map_err(|_| CoreError::InvalidPublicKey)?;
    VerifyingKey::from_bytes(&array).map_err(|_| CoreError::InvalidPublicKey)
}

pub fn generate_media_encryption_material() -> MediaEncryptionMaterial {
    let mut key = [0u8; 32];
    let mut base_nonce = [0u8; 24];
    OsRng.fill_bytes(&mut key);
    OsRng.fill_bytes(&mut base_nonce);

    MediaEncryptionMaterial {
        algorithm: "xchacha20poly1305".to_string(),
        key_b64: Base64::encode_string(&key),
        base_nonce_b64: Base64::encode_string(&base_nonce),
    }
}

pub fn choose_padding_bucket(original_size_bytes: u64) -> Result<u64, CoreError> {
    if original_size_bytes == 0 {
        return Err(CoreError::InvalidMediaSize);
    }

    MEDIA_PADDING_BUCKETS
        .iter()
        .copied()
        .find(|bucket| original_size_bytes <= *bucket)
        .ok_or(CoreError::MediaTooLarge)
}

pub fn build_blob_padding_plan(
    original_size_bytes: u64,
    chunk_size_bytes: u32,
) -> Result<BlobPaddingPlan, CoreError> {
    if chunk_size_bytes == 0 {
        return Err(CoreError::InvalidChunkSize);
    }

    let padded_size_bytes = choose_padding_bucket(original_size_bytes)?;
    let chunk_count = padded_size_bytes.div_ceil(u64::from(chunk_size_bytes)) as u32;

    Ok(BlobPaddingPlan {
        original_size_bytes,
        padded_size_bytes,
        chunk_size_bytes,
        chunk_count,
    })
}

pub fn encrypt_media_bytes(
    plaintext: &[u8],
    chunk_size_bytes: u32,
    material: &MediaEncryptionMaterial,
) -> Result<EncryptedMediaBlob, CoreError> {
    let plan = build_blob_padding_plan(plaintext.len() as u64, chunk_size_bytes)?;
    let (key, base_nonce) = decode_media_material(material)?;
    let cipher = XChaCha20Poly1305::new((&key).into());

    let mut padded_plaintext = plaintext.to_vec();
    padded_plaintext.resize(plan.padded_size_bytes as usize, 0u8);

    let mut chunks = Vec::with_capacity(plan.chunk_count as usize);
    let mut overall_hasher = Sha256::new();
    for (index, chunk) in padded_plaintext
        .chunks(chunk_size_bytes as usize)
        .enumerate()
    {
        let index = index as u32;
        let nonce_bytes = derive_chunk_nonce(base_nonce, index);
        let aad = build_chunk_aad(index, plan.chunk_count, plan.original_size_bytes);
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce_bytes),
                Payload {
                    msg: chunk,
                    aad: &aad,
                },
            )
            .map_err(|_| CoreError::CryptoFailure)?;
        overall_hasher.update(&ciphertext);
        chunks.push(EncryptedMediaChunk {
            index,
            ciphertext_sha256_b64: sha256_b64(&ciphertext),
            ciphertext_b64: Base64::encode_string(&ciphertext),
        });
    }

    Ok(EncryptedMediaBlob {
        algorithm: material.algorithm.clone(),
        original_size_bytes: plan.original_size_bytes,
        padded_size_bytes: plan.padded_size_bytes,
        chunk_size_bytes,
        ciphertext_sha256_b64: Base64::encode_string(&overall_hasher.finalize()),
        chunks,
    })
}

pub fn decrypt_media_bytes(
    blob: &EncryptedMediaBlob,
    material: &MediaEncryptionMaterial,
) -> Result<Vec<u8>, CoreError> {
    let plan = build_blob_padding_plan(blob.original_size_bytes, blob.chunk_size_bytes)?;
    if plan.padded_size_bytes != blob.padded_size_bytes || plan.chunk_count as usize != blob.chunks.len() {
        return Err(CoreError::InvalidChunkSequence);
    }

    let (key, base_nonce) = decode_media_material(material)?;
    let cipher = XChaCha20Poly1305::new((&key).into());
    let mut overall_hasher = Sha256::new();
    let mut plaintext = Vec::with_capacity(blob.padded_size_bytes as usize);

    for (expected_index, chunk) in blob.chunks.iter().enumerate() {
        let expected_index = expected_index as u32;
        if chunk.index != expected_index {
            return Err(CoreError::InvalidChunkSequence);
        }

        let ciphertext = Base64::decode_vec(&chunk.ciphertext_b64).map_err(|_| CoreError::InvalidBase64)?;
        if sha256_b64(&ciphertext) != chunk.ciphertext_sha256_b64 {
            return Err(CoreError::IntegrityMismatch);
        }

        overall_hasher.update(&ciphertext);
        let nonce_bytes = derive_chunk_nonce(base_nonce, chunk.index);
        let aad = build_chunk_aad(chunk.index, plan.chunk_count, blob.original_size_bytes);
        let decrypted = cipher
            .decrypt(
                XNonce::from_slice(&nonce_bytes),
                Payload {
                    msg: &ciphertext,
                    aad: &aad,
                },
            )
            .map_err(|_| CoreError::CryptoFailure)?;
        plaintext.extend_from_slice(&decrypted);
    }

    if Base64::encode_string(&overall_hasher.finalize()) != blob.ciphertext_sha256_b64 {
        return Err(CoreError::IntegrityMismatch);
    }

    plaintext.truncate(blob.original_size_bytes as usize);
    Ok(plaintext)
}

impl EncryptedBlobPointer {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.object_key.is_empty() || self.bucket.is_empty() || self.media_type.is_empty() {
            return Err(CoreError::InvalidMediaSize);
        }

        let plan = build_blob_padding_plan(self.original_size_bytes, self.chunk_size_bytes)?;
        if plan.padded_size_bytes != self.padded_size_bytes {
            return Err(CoreError::InvalidMediaSize);
        }

        for value in [
            &self.encryption_key_b64,
            &self.nonce_b64,
            &self.sha256_b64,
        ] {
            let decoded = Base64::decode_vec(value).map_err(|_| CoreError::InvalidBase64)?;
            if decoded.is_empty() {
                return Err(CoreError::InvalidBase64);
            }
        }

        Ok(())
    }
}

fn decode_media_material(
    material: &MediaEncryptionMaterial,
) -> Result<([u8; 32], [u8; 24]), CoreError> {
    let key = Base64::decode_vec(&material.key_b64).map_err(|_| CoreError::InvalidBase64)?;
    let nonce = Base64::decode_vec(&material.base_nonce_b64).map_err(|_| CoreError::InvalidBase64)?;

    let key: [u8; 32] = key.try_into().map_err(|_| CoreError::InvalidKeyMaterial)?;
    let nonce: [u8; 24] = nonce.try_into().map_err(|_| CoreError::InvalidNonceMaterial)?;
    Ok((key, nonce))
}

fn derive_chunk_nonce(base_nonce: [u8; 24], index: u32) -> [u8; 24] {
    let mut nonce = base_nonce;
    nonce[20..24].copy_from_slice(&index.to_be_bytes());
    nonce
}

fn build_chunk_aad(index: u32, total_chunks: u32, original_size_bytes: u64) -> [u8; 16] {
    let mut aad = [0u8; 16];
    aad[..4].copy_from_slice(&index.to_be_bytes());
    aad[4..8].copy_from_slice(&total_chunks.to_be_bytes());
    aad[8..16].copy_from_slice(&original_size_bytes.to_be_bytes());
    aad
}

fn sha256_b64(bytes: &[u8]) -> String {
    Base64::encode_string(&Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_identity_has_stable_public_bundle_shape() {
        let identity = IdentityMaterial::generate();
        let public = identity.public_identity();

        assert_eq!(public.algorithm, "ed25519");
        assert!(!public.public_key_b64.is_empty());
        assert!(public.fingerprint.contains('-'));
    }

    #[test]
    fn public_identity_round_trips() {
        let identity = IdentityMaterial::generate();
        let public = identity.public_identity();
        let verifying_key = decode_public_identity(&public.public_key_b64).unwrap();

        assert_eq!(verifying_key.to_bytes(), identity.public_key_bytes());
    }

    #[test]
    fn padding_bucket_uses_next_supported_size() {
        assert_eq!(choose_padding_bucket(600_000).unwrap(), 1_048_576);
        assert_eq!(choose_padding_bucket(2_000_000).unwrap(), 5_242_880);
        assert!(matches!(choose_padding_bucket(60_000_000), Err(CoreError::MediaTooLarge)));
    }

    #[test]
    fn blob_padding_plan_counts_chunks() {
        let plan = build_blob_padding_plan(1_400_000, 262_144).unwrap();

        assert_eq!(plan.padded_size_bytes, 5_242_880);
        assert_eq!(plan.chunk_count, 20);
    }

    #[test]
    fn blob_pointer_validation_rejects_wrong_padding() {
        let pointer = EncryptedBlobPointer {
            object_key: "media/demo/abc123".to_string(),
            bucket: "simy-media".to_string(),
            media_type: "image/jpeg".to_string(),
            original_size_bytes: 900_000,
            padded_size_bytes: 5_242_880,
            chunk_size_bytes: 262_144,
            encryption_key_b64: Base64::encode_string(&[7u8; 32]),
            nonce_b64: Base64::encode_string(&[9u8; 24]),
            sha256_b64: Base64::encode_string(&[3u8; 32]),
            expires_at: Utc::now(),
        };

        assert!(matches!(pointer.validate(), Err(CoreError::InvalidMediaSize)));
    }

    #[test]
    fn blob_pointer_validation_accepts_consistent_descriptor() {
        let pointer = EncryptedBlobPointer {
            object_key: "media/demo/abc123".to_string(),
            bucket: "simy-media".to_string(),
            media_type: "image/jpeg".to_string(),
            original_size_bytes: 900_000,
            padded_size_bytes: 1_048_576,
            chunk_size_bytes: 262_144,
            encryption_key_b64: Base64::encode_string(&[7u8; 32]),
            nonce_b64: Base64::encode_string(&[9u8; 24]),
            sha256_b64: Base64::encode_string(&[3u8; 32]),
            expires_at: Utc::now(),
        };

        assert!(pointer.validate().is_ok());
    }

    #[test]
    fn media_encryption_round_trips() {
        let material = generate_media_encryption_material();
        let plaintext = b"simy encrypted media round trip".repeat(20);
        let encrypted = encrypt_media_bytes(&plaintext, 262_144, &material).unwrap();
        let decrypted = decrypt_media_bytes(&encrypted, &material).unwrap();

        assert_eq!(decrypted, plaintext);
        assert_eq!(encrypted.padded_size_bytes, 1_048_576);
    }

    #[test]
    fn tampered_chunk_is_rejected() {
        let material = generate_media_encryption_material();
        let plaintext = b"tamper test".repeat(32);
        let mut encrypted = encrypt_media_bytes(&plaintext, 262_144, &material).unwrap();
        encrypted.chunks[0].ciphertext_b64 = Base64::encode_string(b"tampered");

        assert!(matches!(
            decrypt_media_bytes(&encrypted, &material),
            Err(CoreError::IntegrityMismatch) | Err(CoreError::CryptoFailure)
        ));
    }
}
