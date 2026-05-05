import { invoke } from "@tauri-apps/api/core";

export interface InitStoreRequest {
  storage_path: string;
  encryption_key_b64: string;
}

export interface InitStoreResponse {
  success: boolean;
  message: string;
}

export interface GenerateIdentityResponse {
  signing_key_b64: string;
  exchange_key_b64: string;
}

export interface GeneratePreKeyBundleRequest {
  signing_key_b64: string;
  exchange_key_b64: string;
}

export interface PreKeyBundleResponse {
  identity_signing_key_b64: string;
  identity_exchange_key_b64: string;
  signed_prekey_b64: string;
  signed_prekey_signature_b64: string;
}

export interface BootstrapInitiatorRequest {
  alice_signing_key_b64: string;
  alice_exchange_key_b64: string;
  bob_identity_signing_key_b64: string;
  bob_identity_exchange_key_b64: string;
  bob_signed_prekey_b64: string;
  bob_signed_prekey_signature_b64: string;
  initial_message: string;
}

export interface BootstrapInitiatorResponse {
  session_id: string;
  initial_envelope_b64: string;
}

export interface BootstrapResponderRequest {
  bob_signing_key_b64: string;
  bob_exchange_key_b64: string;
  bob_signed_prekey_b64: string;
  bob_signed_prekey_signature_b64: string;
  initial_envelope_b64: string;
}

export interface BootstrapResponderResponse {
  session_id: string;
  decrypted_message: string;
}

export interface EncryptMessageRequest {
  session_id: string;
  plaintext: string;
  associated_data: string;
}

export interface EncryptMessageResponse {
  ciphertext_b64: string;
}

export interface DecryptMessageRequest {
  session_id: string;
  ciphertext_b64: string;
  associated_data: string;
}

export interface DecryptMessageResponse {
  plaintext: string;
}

export interface SessionStatusRequest {
  session_id: string;
}

export interface SessionStatusResponse {
  exists: boolean;
  message_count?: number;
}

export async function initRatchetStore(
  request: InitStoreRequest
): Promise<InitStoreResponse> {
  return invoke("init_ratchet_store", { request });
}

export async function generateIdentity(): Promise<GenerateIdentityResponse> {
  return invoke("generate_identity");
}

export async function generatePreKeyBundle(
  request: GeneratePreKeyBundleRequest
): Promise<PreKeyBundleResponse> {
  return invoke("generate_prekey_bundle", { request });
}

export async function bootstrapInitiator(
  request: BootstrapInitiatorRequest
): Promise<BootstrapInitiatorResponse> {
  return invoke("bootstrap_initiator", { request });
}

export async function bootstrapResponder(
  request: BootstrapResponderRequest
): Promise<BootstrapResponderResponse> {
  return invoke("bootstrap_responder", { request });
}

export async function encryptMessage(
  request: EncryptMessageRequest
): Promise<EncryptMessageResponse> {
  return invoke("encrypt_message", { request });
}

export async function decryptMessage(
  request: DecryptMessageRequest
): Promise<DecryptMessageResponse> {
  return invoke("decrypt_message", { request });
}

export async function checkSessionStatus(
  request: SessionStatusRequest
): Promise<SessionStatusResponse> {
  return invoke("check_session_status", { request });
}
