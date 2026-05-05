use base64ct::{Base64, Encoding};
use comm_core::{
    bootstrap_initiator_session, bootstrap_responder_session, open_initial_message,
    seal_initial_message, DoubleRatchetSession, EncryptedFileDoubleRatchetSessionStore,
    IdentityKeyPair, PreKeyBundle, RelayContentKind, SignedPreKey,
};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;

pub struct RatchetState {
    store: Mutex<Option<EncryptedFileDoubleRatchetSessionStore>>,
}

impl RatchetState {
    pub fn new() -> Self {
        Self {
            store: Mutex::new(None),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitStoreRequest {
    storage_path: String,
    encryption_key_b64: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitStoreResponse {
    success: bool,
    message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateIdentityResponse {
    signing_key_b64: String,
    exchange_key_b64: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeneratePreKeyBundleRequest {
    signing_key_b64: String,
    exchange_key_b64: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PreKeyBundleResponse {
    identity_signing_key_b64: String,
    identity_exchange_key_b64: String,
    signed_prekey_b64: String,
    signed_prekey_signature_b64: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BootstrapInitiatorRequest {
    alice_signing_key_b64: String,
    alice_exchange_key_b64: String,
    bob_identity_signing_key_b64: String,
    bob_identity_exchange_key_b64: String,
    bob_signed_prekey_b64: String,
    bob_signed_prekey_signature_b64: String,
    initial_message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BootstrapInitiatorResponse {
    session_id: String,
    initial_envelope_b64: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BootstrapResponderRequest {
    bob_signing_key_b64: String,
    bob_exchange_key_b64: String,
    bob_signed_prekey_b64: String,
    bob_signed_prekey_signature_b64: String,
    initial_envelope_b64: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BootstrapResponderResponse {
    session_id: String,
    decrypted_message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EncryptMessageRequest {
    session_id: String,
    plaintext: String,
    associated_data: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EncryptMessageResponse {
    ciphertext_b64: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DecryptMessageRequest {
    session_id: String,
    ciphertext_b64: String,
    associated_data: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DecryptMessageResponse {
    plaintext: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionStatusRequest {
    session_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionStatusResponse {
    exists: bool,
    message_count: Option<u32>,
}

#[tauri::command]
pub fn init_ratchet_store(
    state: State<RatchetState>,
    request: InitStoreRequest,
) -> Result<InitStoreResponse, String> {
    let encryption_key_bytes = Base64::decode_vec(&request.encryption_key_b64)
        .map_err(|e| format!("Invalid base64 encryption key: {}", e))?;

    if encryption_key_bytes.len() != 32 {
        return Err(format!(
            "Encryption key must be 32 bytes, got {}",
            encryption_key_bytes.len()
        ));
    }

    let mut encryption_key = [0u8; 32];
    encryption_key.copy_from_slice(&encryption_key_bytes);

    let store = EncryptedFileDoubleRatchetSessionStore::new(&request.storage_path, encryption_key);
    *state.store.lock().unwrap() = Some(store);

    Ok(InitStoreResponse {
        success: true,
        message: format!("Ratchet store initialized at {}", request.storage_path),
    })
}

#[tauri::command]
pub fn generate_identity() -> Result<GenerateIdentityResponse, String> {
    let identity = IdentityKeyPair::generate();
    let signing_key_bytes = identity.signing_key_bytes();
    let exchange_key_bytes = identity.exchange_key_bytes();

    Ok(GenerateIdentityResponse {
        signing_key_b64: Base64::encode_string(&signing_key_bytes),
        exchange_key_b64: Base64::encode_string(&exchange_key_bytes),
    })
}

#[tauri::command]
pub fn generate_prekey_bundle(
    request: GeneratePreKeyBundleRequest,
) -> Result<PreKeyBundleResponse, String> {
    let signing_key_bytes = Base64::decode_vec(&request.signing_key_b64)
        .map_err(|e| format!("Invalid signing key: {}", e))?;
    let exchange_key_bytes = Base64::decode_vec(&request.exchange_key_b64)
        .map_err(|e| format!("Invalid exchange key: {}", e))?;

    let identity = IdentityKeyPair::from_bytes(&signing_key_bytes, &exchange_key_bytes)
        .map_err(|e| format!("Failed to reconstruct identity: {:?}", e))?;

    let signed_prekey = SignedPreKey::generate(&identity);
    let prekey_bundle = PreKeyBundle::from_parts(&identity, &signed_prekey, None);

    Ok(PreKeyBundleResponse {
        identity_signing_key_b64: Base64::encode_string(&prekey_bundle.identity_signing_key),
        identity_exchange_key_b64: Base64::encode_string(&prekey_bundle.identity_exchange_key),
        signed_prekey_b64: Base64::encode_string(&prekey_bundle.signed_prekey),
        signed_prekey_signature_b64: Base64::encode_string(&prekey_bundle.signature),
    })
}

#[tauri::command]
pub fn bootstrap_initiator(
    state: State<RatchetState>,
    request: BootstrapInitiatorRequest,
) -> Result<BootstrapInitiatorResponse, String> {
    let store_guard = state.store.lock().unwrap();
    let store = store_guard
        .as_ref()
        .ok_or_else(|| "Ratchet store not initialized".to_string())?;

    let alice_signing = Base64::decode_vec(&request.alice_signing_key_b64)
        .map_err(|e| format!("Invalid alice signing key: {}", e))?;
    let alice_exchange = Base64::decode_vec(&request.alice_exchange_key_b64)
        .map_err(|e| format!("Invalid alice exchange key: {}", e))?;
    let alice_identity = IdentityKeyPair::from_bytes(&alice_signing, &alice_exchange)
        .map_err(|e| format!("Failed to reconstruct alice identity: {:?}", e))?;

    let bob_signing = Base64::decode_vec(&request.bob_identity_signing_key_b64)
        .map_err(|e| format!("Invalid bob signing key: {}", e))?;
    let bob_exchange = Base64::decode_vec(&request.bob_identity_exchange_key_b64)
        .map_err(|e| format!("Invalid bob exchange key: {}", e))?;
    let bob_prekey = Base64::decode_vec(&request.bob_signed_prekey_b64)
        .map_err(|e| format!("Invalid bob prekey: {}", e))?;
    let bob_signature = Base64::decode_vec(&request.bob_signed_prekey_signature_b64)
        .map_err(|e| format!("Invalid bob signature: {}", e))?;

    let bob_bundle = PreKeyBundle {
        identity_signing_key: bob_signing.try_into().unwrap(),
        identity_exchange_key: bob_exchange.try_into().unwrap(),
        signed_prekey: bob_prekey.try_into().unwrap(),
        signature: bob_signature.try_into().unwrap(),
        one_time_prekey: None,
    };

    let (initial_envelope, alice_handshake) = seal_initial_message(
        &alice_identity,
        &bob_bundle,
        request.initial_message.as_bytes(),
        RelayContentKind::SessionBootstrap,
    )
    .map_err(|e| format!("Failed to seal initial message: {:?}", e))?;

    let mut alice_session = bootstrap_initiator_session(alice_handshake)
        .map_err(|e| format!("Failed to bootstrap initiator session: {:?}", e))?;

    let session_id = format!(
        "alice_{}_bob_{}",
        hex::encode(&alice_identity.signing_key_bytes()[..8]),
        hex::encode(&bob_signing[..8])
    );

    alice_session
        .save_to_store(store, &session_id)
        .map_err(|e| format!("Failed to save session: {:?}", e))?;

    Ok(BootstrapInitiatorResponse {
        session_id,
        initial_envelope_b64: Base64::encode_string(&initial_envelope),
    })
}

#[tauri::command]
pub fn bootstrap_responder(
    state: State<RatchetState>,
    request: BootstrapResponderRequest,
) -> Result<BootstrapResponderResponse, String> {
    let store_guard = state.store.lock().unwrap();
    let store = store_guard
        .as_ref()
        .ok_or_else(|| "Ratchet store not initialized".to_string())?;

    let bob_signing = Base64::decode_vec(&request.bob_signing_key_b64)
        .map_err(|e| format!("Invalid bob signing key: {}", e))?;
    let bob_exchange = Base64::decode_vec(&request.bob_exchange_key_b64)
        .map_err(|e| format!("Invalid bob exchange key: {}", e))?;
    let bob_identity = IdentityKeyPair::from_bytes(&bob_signing, &bob_exchange)
        .map_err(|e| format!("Failed to reconstruct bob identity: {:?}", e))?;

    let bob_prekey_bytes = Base64::decode_vec(&request.bob_signed_prekey_b64)
        .map_err(|e| format!("Invalid bob prekey: {}", e))?;
    let bob_signature_bytes = Base64::decode_vec(&request.bob_signed_prekey_signature_b64)
        .map_err(|e| format!("Invalid bob signature: {}", e))?;

    let bob_signed_prekey = SignedPreKey {
        public_key: bob_prekey_bytes.try_into().unwrap(),
        signature: bob_signature_bytes.try_into().unwrap(),
    };

    let initial_envelope = Base64::decode_vec(&request.initial_envelope_b64)
        .map_err(|e| format!("Invalid initial envelope: {}", e))?;

    let opened = open_initial_message(&bob_identity, &bob_signed_prekey, None, &initial_envelope)
        .map_err(|e| format!("Failed to open initial message: {:?}", e))?;

    let mut bob_session = bootstrap_responder_session(&bob_signed_prekey, &opened)
        .map_err(|e| format!("Failed to bootstrap responder session: {:?}", e))?;

    let session_id = format!(
        "bob_{}_alice_{}",
        hex::encode(&bob_identity.signing_key_bytes()[..8]),
        hex::encode(&opened.initiator_identity_signing_key[..8])
    );

    bob_session
        .save_to_store(store, &session_id)
        .map_err(|e| format!("Failed to save session: {:?}", e))?;

    let decrypted_message = String::from_utf8(opened.plaintext)
        .map_err(|e| format!("Failed to decode plaintext: {}", e))?;

    Ok(BootstrapResponderResponse {
        session_id,
        decrypted_message,
    })
}

#[tauri::command]
pub fn encrypt_message(
    state: State<RatchetState>,
    request: EncryptMessageRequest,
) -> Result<EncryptMessageResponse, String> {
    let store_guard = state.store.lock().unwrap();
    let store = store_guard
        .as_ref()
        .ok_or_else(|| "Ratchet store not initialized".to_string())?;

    let mut session = DoubleRatchetSession::load_from_store(store, &request.session_id)
        .map_err(|e| format!("Failed to load session: {:?}", e))?
        .ok_or_else(|| format!("Session {} not found", request.session_id))?;

    let ciphertext = session
        .encrypt(
            request.plaintext.as_bytes(),
            request.associated_data.as_bytes(),
        )
        .map_err(|e| format!("Failed to encrypt: {:?}", e))?;

    session
        .save_to_store(store, &request.session_id)
        .map_err(|e| format!("Failed to save session: {:?}", e))?;

    Ok(EncryptMessageResponse {
        ciphertext_b64: Base64::encode_string(&ciphertext),
    })
}

#[tauri::command]
pub fn decrypt_message(
    state: State<RatchetState>,
    request: DecryptMessageRequest,
) -> Result<DecryptMessageResponse, String> {
    let store_guard = state.store.lock().unwrap();
    let store = store_guard
        .as_ref()
        .ok_or_else(|| "Ratchet store not initialized".to_string())?;

    let mut session = DoubleRatchetSession::load_from_store(store, &request.session_id)
        .map_err(|e| format!("Failed to load session: {:?}", e))?
        .ok_or_else(|| format!("Session {} not found", request.session_id))?;

    let ciphertext = Base64::decode_vec(&request.ciphertext_b64)
        .map_err(|e| format!("Invalid ciphertext: {}", e))?;

    let plaintext_bytes = session
        .decrypt(&ciphertext, request.associated_data.as_bytes())
        .map_err(|e| format!("Failed to decrypt: {:?}", e))?;

    session
        .save_to_store(store, &request.session_id)
        .map_err(|e| format!("Failed to save session: {:?}", e))?;

    let plaintext = String::from_utf8(plaintext_bytes)
        .map_err(|e| format!("Failed to decode plaintext: {}", e))?;

    Ok(DecryptMessageResponse { plaintext })
}

#[tauri::command]
pub fn check_session_status(
    state: State<RatchetState>,
    request: SessionStatusRequest,
) -> Result<SessionStatusResponse, String> {
    let store_guard = state.store.lock().unwrap();
    let store = store_guard
        .as_ref()
        .ok_or_else(|| "Ratchet store not initialized".to_string())?;

    let session_opt = DoubleRatchetSession::load_from_store(store, &request.session_id)
        .map_err(|e| format!("Failed to load session: {:?}", e))?;

    match session_opt {
        Some(session) => Ok(SessionStatusResponse {
            exists: true,
            message_count: Some(session.send_message_number()),
        }),
        None => Ok(SessionStatusResponse {
            exists: false,
            message_count: None,
        }),
    }
}
