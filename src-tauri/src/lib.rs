// src-tauri/src/lib.rs
// Entry point library — mengorkestrasi semua modul Aoi AI Assistant.

mod state;
mod socket;
pub mod llm_client;
mod hyprland;
mod bash_executor;
pub mod commands;
mod tts;
pub mod config;
pub mod stt;
pub mod avatar;

use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{AppHandle, Emitter};

pub use state::{AppState, AssistantState, StatePayload};
pub use tts::TtsPlayerState;
pub use stt::AudioRecorderState;
pub use llm_client::ConversationMemory;

pub struct MemoryState(pub Arc<Mutex<ConversationMemory>>);

// ────────────────────────────────────────────────────────────────────────────
// Broadcast: sinkronisasi state ke Vue UI + /tmp/ai_state.json (Ryoku bar)
// ────────────────────────────────────────────────────────────────────────────

pub async fn broadcast_state(app: &AppHandle, shared: &Arc<Mutex<StatePayload>>) {
    let current = shared.lock().await.clone();
    let _ = app.emit("assistant-state-changed", &current);
    if let Ok(json) = serde_json::to_string(&current) {
        let _ = tokio::fs::write("/tmp/ai_state.json", json).await;
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Tauri Entry Point
// ────────────────────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let initial_state = Arc::new(Mutex::new(StatePayload {
        state: AssistantState::Idle,
        message: "Halo! Tekan Super+V atau ketik untuk mulai ngobrol.".into(),
        pending_command: None,
        minimized: false,
    }));

    // Handle proses TTS audio player
    let tts_player: Arc<Mutex<Option<tokio::process::Child>>> =
        Arc::new(Mutex::new(None));

    // Handle proses perekaman mikrofon PipeWire
    let audio_recorder: Arc<Mutex<Option<tokio::process::Child>>> =
        Arc::new(Mutex::new(None));

    // Handle riwayat obrolan (ConversationMemory)
    let chat_memory: Arc<Mutex<ConversationMemory>> =
        Arc::new(Mutex::new(ConversationMemory::new()));

    tauri::Builder::default()
        .manage(AppState(initial_state.clone()))
        .manage(TtsPlayerState(tts_player.clone()))
        .manage(AudioRecorderState(audio_recorder.clone()))
        .manage(MemoryState(chat_memory.clone()))
        .setup(move |app| {
            let handle = app.handle().clone();
            let state_clone = initial_state.clone();
            let tts_clone = tts_player.clone();
            let rec_clone = audio_recorder.clone();

            tauri::async_runtime::spawn(async move {
                // Broadcast state awal
                broadcast_state(&handle, &state_clone).await;
                // Jalankan IPC socket listener di background
                socket::spawn_unix_socket_listener(handle, state_clone, tts_clone, rec_clone).await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_current_state,
            commands::set_manual_state,
            commands::toggle_minimize,
            commands::ask_llm,
            commands::get_hyprland_context,
            commands::execute_command,
            commands::speak_text,
            commands::get_config,
            commands::save_config,
            commands::trigger_mic,
            commands::save_avatar_sprite,
            commands::save_avatar_pack,
            commands::load_avatar_pack,
            commands::reset_avatar_pack,
            commands::confirm_pending_command,
            commands::cancel_pending_command,
            commands::clear_chat_memory,
        ])
        .run(tauri::generate_context!())
        .expect("error saat menjalankan Aoi AI Assistant");
}
