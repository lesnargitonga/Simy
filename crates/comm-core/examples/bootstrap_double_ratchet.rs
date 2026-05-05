use comm_core::{
    bootstrap_initiator_session, bootstrap_responder_session, open_initial_message,
    seal_initial_message, IdentityKeyPair, OneTimePreKey, PreKeyBundle, RelayContentKind,
    SignedPreKey,
};

fn main() {
    let alice_identity = IdentityKeyPair::generate();
    let bob_identity = IdentityKeyPair::generate();
    let bob_signed_prekey = SignedPreKey::generate(&bob_identity);
    let bob_one_time_prekey = OneTimePreKey::generate();
    let bob_bundle = PreKeyBundle::from_parts(
        &bob_identity,
        &bob_signed_prekey,
        Some(&bob_one_time_prekey),
    );

    let (initial_envelope, alice_handshake) = seal_initial_message(
        &alice_identity,
        &bob_bundle,
        b"alice bootstrap message",
        RelayContentKind::SessionBootstrap,
    )
    .expect("failed to create bootstrap envelope");
    let opened = open_initial_message(
        &bob_identity,
        &bob_signed_prekey,
        Some(&bob_one_time_prekey),
        &initial_envelope,
    )
    .expect("failed to open bootstrap envelope");

    let mut alice_session =
        bootstrap_initiator_session(alice_handshake).expect("failed to bootstrap alice session");
    let mut bob_session = bootstrap_responder_session(&bob_signed_prekey, &opened)
        .expect("failed to bootstrap bob session");

    let bob_reply = bob_session
        .encrypt(b"bob reply one", b"example-chat")
        .expect("failed to encrypt bob reply");
    let alice_plaintext = alice_session
        .decrypt(&bob_reply, b"example-chat")
        .expect("failed to decrypt bob reply");

    let alice_reply = alice_session
        .encrypt(b"alice reply two", b"example-chat")
        .expect("failed to encrypt alice reply");
    let bob_plaintext = bob_session
        .decrypt(&alice_reply, b"example-chat")
        .expect("failed to decrypt alice reply");

    println!(
        "Alice decrypted: {}",
        String::from_utf8(alice_plaintext).expect("demo plaintext should be utf8")
    );
    println!(
        "Bob decrypted: {}",
        String::from_utf8(bob_plaintext).expect("demo plaintext should be utf8")
    );
}