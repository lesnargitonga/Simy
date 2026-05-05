mod ratchet;

use ratchet::RatchetState;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(RatchetState::new())
        .invoke_handler(tauri::generate_handler![
            greet,
            ratchet::init_ratchet_store,
            ratchet::generate_identity,
            ratchet::generate_prekey_bundle,
            ratchet::bootstrap_initiator,
            ratchet::bootstrap_responder,
            ratchet::encrypt_message,
            ratchet::decrypt_message,
            ratchet::check_session_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
