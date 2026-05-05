use base64ct::{Base64, Encoding};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use x25519_dalek::PublicKey;

use crate::{CoreError, IdentityKeyPair};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DevicePublicKeys {
    pub signing_key_b64: String,
    pub exchange_key_b64: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceRecord {
    pub version: String,
    pub device_id: String,
    pub device_label: String,
    pub device_signing_key_b64: String,
    pub device_exchange_key_b64: String,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub signature_b64: String,
}

pub struct DeviceKeyPair {
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
    pub exchange_secret_key: x25519_dalek::StaticSecret,
    pub exchange_public_key: PublicKey,
}

impl core::fmt::Debug for DeviceKeyPair {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("DeviceKeyPair")
            .field("verifying_key", &self.verifying_key)
            .field("exchange_public_key", &self.exchange_public_key)
            .field("signing_key", &"<redacted>")
            .field("exchange_secret_key", &"<redacted>")
            .finish()
    }
}

impl DeviceKeyPair {
    pub fn generate() -> Self {
        let identity = IdentityKeyPair::generate();
        Self {
            signing_key: identity.signing_key,
            verifying_key: identity.verifying_key,
            exchange_secret_key: identity.exchange_secret_key,
            exchange_public_key: identity.exchange_public_key,
        }
    }

    pub fn public_keys(&self) -> DevicePublicKeys {
        DevicePublicKeys {
            signing_key_b64: Base64::encode_string(&self.verifying_key.to_bytes()),
            exchange_key_b64: Base64::encode_string(self.exchange_public_key.as_bytes()),
        }
    }
}

impl DeviceRecord {
    pub fn sign(
        identity_signing_key: &SigningKey,
        device_id: String,
        device_label: String,
        device_keys: &DevicePublicKeys,
        created_at: DateTime<Utc>,
        revoked_at: Option<DateTime<Utc>>,
    ) -> Result<Self, CoreError> {
        validate_device_id(&device_id)?;
        validate_device_label(&device_label)?;
        let signing_key = decode_ed25519_public_key(&device_keys.signing_key_b64)?;
        let exchange_key = decode_x25519_public_key(&device_keys.exchange_key_b64)?;

        let payload = device_record_signing_payload(
            &device_id,
            &device_label,
            &signing_key,
            &exchange_key,
            created_at,
            revoked_at,
        );
        let signature = identity_signing_key.sign(&payload);

        Ok(Self {
            version: "simy-device-record-v1".to_string(),
            device_id,
            device_label,
            device_signing_key_b64: device_keys.signing_key_b64.clone(),
            device_exchange_key_b64: device_keys.exchange_key_b64.clone(),
            created_at,
            revoked_at,
            signature_b64: Base64::encode_string(&signature.to_bytes()),
        })
    }

    pub fn verify(&self, identity_signing_key: &VerifyingKey) -> Result<(), CoreError> {
        if self.version != "simy-device-record-v1" {
            return Err(CoreError::UnsupportedVersion);
        }
        validate_device_id(&self.device_id)?;
        validate_device_label(&self.device_label)?;
        let device_signing_key = decode_ed25519_public_key(&self.device_signing_key_b64)?;
        let device_exchange_key = decode_x25519_public_key(&self.device_exchange_key_b64)?;
        let signature = decode_signature(&self.signature_b64)?;
        let payload = device_record_signing_payload(
            &self.device_id,
            &self.device_label,
            &device_signing_key,
            &device_exchange_key,
            self.created_at,
            self.revoked_at,
        );
        identity_signing_key
            .verify(&payload, &signature)
            .map_err(|_| CoreError::InvalidSignature)
    }

    pub fn is_revoked(&self) -> bool {
        self.revoked_at.is_some()
    }
}

pub fn validate_device_id(value: &str) -> Result<(), CoreError> {
    if value.len() < 8 || value.len() > 128 {
        return Err(CoreError::InvalidDeviceRecord);
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(CoreError::InvalidDeviceRecord);
    }
    Ok(())
}

pub fn validate_device_label(value: &str) -> Result<(), CoreError> {
    if value.trim().is_empty() || value.len() > 128 {
        return Err(CoreError::InvalidDeviceRecord);
    }
    Ok(())
}

fn device_record_signing_payload(
    device_id: &str,
    device_label: &str,
    device_signing_key: &VerifyingKey,
    device_exchange_key: &PublicKey,
    created_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
) -> Vec<u8> {
    let mut payload = Vec::with_capacity(256);
    payload.extend_from_slice(b"simy-device-record-v1");
    payload.extend_from_slice(device_id.as_bytes());
    payload.push(0);
    payload.extend_from_slice(device_label.as_bytes());
    payload.push(0);
    payload.extend_from_slice(&device_signing_key.to_bytes());
    payload.extend_from_slice(device_exchange_key.as_bytes());
    payload.extend_from_slice(created_at.to_rfc3339().as_bytes());
    if let Some(revoked_at) = revoked_at {
        payload.push(1);
        payload.extend_from_slice(revoked_at.to_rfc3339().as_bytes());
    } else {
        payload.push(0);
    }
    payload
}

fn decode_ed25519_public_key(value: &str) -> Result<VerifyingKey, CoreError> {
    let bytes = Base64::decode_vec(value).map_err(|_| CoreError::InvalidBase64)?;
    let bytes: [u8; 32] = bytes.try_into().map_err(|_| CoreError::InvalidPublicKey)?;
    VerifyingKey::from_bytes(&bytes).map_err(|_| CoreError::InvalidPublicKey)
}

fn decode_x25519_public_key(value: &str) -> Result<PublicKey, CoreError> {
    let bytes = Base64::decode_vec(value).map_err(|_| CoreError::InvalidBase64)?;
    let bytes: [u8; 32] = bytes.try_into().map_err(|_| CoreError::InvalidPublicKey)?;
    Ok(PublicKey::from(bytes))
}

fn decode_signature(value: &str) -> Result<Signature, CoreError> {
    let bytes = Base64::decode_vec(value).map_err(|_| CoreError::InvalidBase64)?;
    let bytes: [u8; 64] = bytes.try_into().map_err(|_| CoreError::InvalidSignature)?;
    Ok(Signature::from_bytes(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_record_sign_and_verify_round_trip() {
        let identity = IdentityKeyPair::generate();
        let device = DeviceKeyPair::generate();
        let record = DeviceRecord::sign(
            &identity.signing_key,
            "desktop_primary".to_string(),
            "Desktop Primary".to_string(),
            &device.public_keys(),
            Utc::now(),
            None,
        )
        .unwrap();

        assert!(record.verify(&identity.verifying_key).is_ok());
        assert!(!record.is_revoked());
    }

    #[test]
    fn device_record_rejects_tampering() {
        let identity = IdentityKeyPair::generate();
        let device = DeviceKeyPair::generate();
        let mut record = DeviceRecord::sign(
            &identity.signing_key,
            "desktop_primary".to_string(),
            "Desktop Primary".to_string(),
            &device.public_keys(),
            Utc::now(),
            None,
        )
        .unwrap();
        record.device_label = "Tampered".to_string();

        assert!(matches!(record.verify(&identity.verifying_key), Err(CoreError::InvalidSignature)));
    }
}