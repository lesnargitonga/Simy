use comm_core::{
    open_initial_message, seal_initial_message, IdentityKeyPair, OneTimePreKey, PreKeyBundle,
    RelayContentKind, SignedPreKey,
};

fn main() {
    let sender_identity = IdentityKeyPair::generate();
    let receiver_identity = IdentityKeyPair::generate();
    let receiver_signed_prekey = SignedPreKey::generate(&receiver_identity);
    let receiver_one_time_prekey = OneTimePreKey::generate();
    let receiver_bundle = PreKeyBundle::from_parts(
        &receiver_identity,
        &receiver_signed_prekey,
        Some(&receiver_one_time_prekey),
    );

    let plaintext = b"hello from the X3DH bootstrap envelope";
    let (envelope, initiator_handshake) = seal_initial_message(
        &sender_identity,
        &receiver_bundle,
        plaintext,
        RelayContentKind::SessionBootstrap,
    )
    .expect("failed to seal initial message");
    let opened = open_initial_message(
        &receiver_identity,
        &receiver_signed_prekey,
        Some(&receiver_one_time_prekey),
        &envelope,
    )
    .expect("failed to open initial message");

    println!("Envelope:\n{}", serde_json::to_string_pretty(&envelope).unwrap());
    println!();
    println!(
        "Opened plaintext: {}",
        String::from_utf8(opened.plaintext).expect("plaintext must be valid UTF-8 for this example")
    );
    println!(
        "Shared secret matches: {}",
        opened.shared_secret == initiator_handshake.shared_secret
    );
}