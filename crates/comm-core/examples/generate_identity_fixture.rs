use base64ct::Encoding;
use chrono::{Duration, Utc};
use comm_core::{DeviceKeyPair, DeviceRecord, IdentityKeyPair, OneTimePreKey, SignedPreKey};
use serde_json::json;

fn main() {
    let identity = IdentityKeyPair::generate();
    let signed_prekey = SignedPreKey::generate(&identity);
    let one_time_prekeys = (0..5)
        .map(|_| OneTimePreKey::generate())
        .collect::<Vec<_>>();
    let device = DeviceKeyPair::generate();
    let device_record = DeviceRecord::sign(
        &identity.signing_key,
        "desktop_primary".to_string(),
        "Desktop Primary".to_string(),
        &device.public_keys(),
        Utc::now(),
        None,
    )
    .expect("failed to sign device record");

    let fixture = json!({
        "prekey_bundle": {
            "identity_signing_key_b64": base64ct::Base64::encode_string(&identity.verifying_key.to_bytes()),
            "identity_exchange_key_b64": base64ct::Base64::encode_string(identity.exchange_public_key.as_bytes()),
            "signed_prekey_b64": base64ct::Base64::encode_string(signed_prekey.public_key.as_bytes()),
            "signed_prekey_signature_b64": base64ct::Base64::encode_string(&signed_prekey.signature.to_bytes()),
            "signed_prekey_expires_at": (Utc::now() + Duration::days(7)).to_rfc3339(),
            "one_time_prekeys_b64": one_time_prekeys
                .iter()
                .map(|prekey| base64ct::Base64::encode_string(prekey.public_key.as_bytes()))
                .collect::<Vec<_>>()
        },
        "device_record": device_record
    });

    println!("{}", serde_json::to_string_pretty(&fixture).unwrap());
}