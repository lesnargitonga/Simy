use base64ct::{Base64, Encoding};
use comm_core::{IdentityKeyPair, OneTimePreKey, SignedPreKey};

fn main() {
    let identity = IdentityKeyPair::generate();
    let signed_prekey = SignedPreKey::generate(&identity);
    let one_time_prekeys = (0..5)
        .map(|_| OneTimePreKey::generate())
        .collect::<Vec<_>>();

    let one_time_prekeys_json = one_time_prekeys
        .iter()
        .map(|prekey| format!("    \"{}\"", Base64::encode_string(prekey.public_key.as_bytes())))
        .collect::<Vec<_>>()
        .join(",\n");

    println!(
        "{{\n  \"identity_signing_key_b64\": \"{}\",\n  \"identity_exchange_key_b64\": \"{}\",\n  \"signed_prekey_b64\": \"{}\",\n  \"signed_prekey_signature_b64\": \"{}\",\n  \"one_time_prekeys_b64\": [\n{}\n  ]\n}}",
        Base64::encode_string(&identity.verifying_key.to_bytes()),
        Base64::encode_string(identity.exchange_public_key.as_bytes()),
        Base64::encode_string(signed_prekey.public_key.as_bytes()),
        Base64::encode_string(&signed_prekey.signature.to_bytes()),
        one_time_prekeys_json,
    );
}