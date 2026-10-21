// Mencegah munculnya window terminal tambahan saat rilis
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

const SOCKET_PATH: &str = "/tmp/ai-assistant.sock";
const RYOKU_STATUS_PATH: &str = "/tmp/ai_state.json";

/// State utama dari asisten AI
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AssistantState {
    Idle,
    Listening,
    Thinking,
    Speaking,
    Confirm,
    Error,
}

/// Struktur data yang dikirim ke Frontend Vue & Status Bar Ryoku
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatePayload {
    pub state: AssistantState,
    pub message: String,
    pub pending_command: Option<String>,
    pub minimized: bool,
}

pub struct AppState(pub Arc<Mutex<StatePayload>>);

/// Fungsi untuk memperbarui state, memberi tahu Vue UI, dan menulis file untuk bar Ryoku
fn broadcast_state(app: &AppHandle, shared_state: &Arc<Mutex<StatePayload>>) {
    if let Ok(current) = shared_state.lock() {
        // 1. Kirim event real-time ke Vue Frontend
        let _ = app.emit("assistant-state-changed", current.clone());

        // 2. Tulis status ke /tmp/ai_state.json agar bisa dibaca oleh bar Ryoku
        if let Ok(json_str) = serde_json::to_string(&*current) {
            let _ = fs::write(RYOKU_STATUS_PATH, json_str);
        }
    }
}

/// Logika "Smart Keybind" ketika user menekan Super + V di Hyprland
fn handle_smart_toggle(app: &AppHandle, shared_state: &Arc<Mutex<StatePayload>>) {
    {
        let mut current = shared_state.lock().unwrap();
        match current.state {
            AssistantState::Idle | AssistantState::Error => {
                // Dari diam -> mulai mendengarkan
                current.state = AssistantState::Listening;
                current.message = "Mendengarkan... (Tekan Super+V lagi untuk kirim/batal)".into();
            }
            AssistantState::Listening => {
                // Dari mendengarkan -> selesai bicara (Tahap 1: simulasi ke Thinking atau kembali ke Idle)
                current.state = AssistantState::Idle;
                current.message = "Mikrofon dimatikan (Standby).".into();
            }
            AssistantState::Thinking | AssistantState::Speaking => {
                // BARGE-IN (Interupsi)! Hentikan proses bicara/mikir dan langsung mendengarkan lagi
                current.state = AssistantState::Listening;
                current.message = "⚡ Interupsi! Silakan bicara...".into();
            }
            AssistantState::Confirm => {
                // Jika sedang minta konfirmasi bash tapi ditekan Super+V, batalkan aksi
                current.state = AssistantState::Idle;
                current.pending_command = None;
                current.message = "Eksekusi perintah dibatalkan.".into();
            }
        }
    }
    broadcast_state(app, shared_state);
}

/// Command Tauri yang bisa dipanggil langsung dari klik tombol di Vue
#[tauri::command]
fn get_current_state(state: State<'_, AppState>) -> StatePayload {
    state.0.lock().unwrap().clone()
}

#[tauri::command]
fn set_manual_state(
    new_state: AssistantState,
    message: String,
    command: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) {
    {
        let mut current = state.0.lock().unwrap();
        current.state = new_state;
        current.message = message;
        current.pending_command = command;
    }
    broadcast_state(&app, &state.0);
}

#[tauri::command]
fn toggle_minimize(app: AppHandle, state: State<'_, AppState>) {
    {
        let mut current = state.0.lock().unwrap();
        current.minimized = !current.minimized;
    }
    broadcast_state(&app, &state.0);
}

/// Menjalankan Unix Domain Socket Server di background (/tmp/ai-assistant.sock)
fn spawn_unix_socket_listener(app: AppHandle, shared_state: Arc<Mutex<StatePayload>>) {
    std::thread::spawn(move || {
        // Hapus file socket lama jika masih tersisa
        if Path::new(SOCKET_PATH).exists() {
            let _ = fs::remove_file(SOCKET_PATH);
        }

        let listener = UnixListener::bind(SOCKET_PATH).expect("Gagal membuat Unix Socket");
        println!("🚀 IPC Socket aktif di: {}", SOCKET_PATH);

        for stream in listener.incoming() {
            if let Ok(stream) = stream {
                let reader = BufReader::new(stream);
                for line in reader.lines().map_while(Result::ok) {
                    let cmd = line.trim();
                    match cmd {
                        "toggle_mic" => handle_smart_toggle(&app, &shared_state),
                        "toggle_hide" => {
                            {
                                let mut s = shared_state.lock().unwrap();
                                s.minimized = !s.minimized;
                            }
                            broadcast_state(&app, &shared_state);
                        }
                        "confirm_yes" => {
                            {
                                let mut s = shared_state.lock().unwrap();
                                if s.state == AssistantState::Confirm {
                                    s.state = AssistantState::Speaking;
                                    s.message = format!(
                                        "✅ Menjalankan: {}",
                                        s.pending_command.take().unwrap_or_default()
                                    );
                                }
                            }
                            broadcast_state(&app, &shared_state);
                        }
                        "confirm_no" => {
                            {
                                let mut s = shared_state.lock().unwrap();
                                if s.state == AssistantState::Confirm {
                                    s.state = AssistantState::Idle;
                                    s.pending_command = None;
                                    s.message = "❌ Perintah ditolak oleh pengguna.".into();
                                }
                            }
                            broadcast_state(&app, &shared_state);
                        }
                        // Command khusus testing dari terminal: state:speaking, state:thinking, dll.
                        _ if cmd.starts_with("state:") => {
                            let target = cmd.replace("state:", "");
                            let mut s = shared_state.lock().unwrap();
                            match target.as_str() {
                                "idle" => {
                                    s.state = AssistantState::Idle;
                                    s.message = "Standby...".into();
                                }
                                "listening" => {
                                    s.state = AssistantState::Listening;
                                    s.message = "Mendengarkan suaramu...".into();
                                }
                                "thinking" => {
                                    s.state = AssistantState::Thinking;
                                    s.message = "Sedang memikirkan jawaban...".into();
                                }
                                "speaking" => {
                                    s.state = AssistantState::Speaking;
                                    s.message = "Halo bos! Ada yang bisa kubantu di CachyOS hari ini?".into();
                                }
                                "confirm" => {
                                    s.state = AssistantState::Confirm;
                                    s.message = "Izinkan saya menjalankan perintah sistem ini?".into();
                                    s.pending_command = Some("sudo pacman -Syu".into());
                                }
                                "error" => {
                                    s.state = AssistantState::Error;
                                    s.message = "Waduh, koneksi API gagal atau terjadi error!".into();
                                }
                                _ => {}
                            }
                            drop(s);
                            broadcast_state(&app, &shared_state);
                        }
                        _ => println!("Perintah socket tidak dikenal: {}", cmd),
                    }
                }
            }
        }
    });
}

fn main() {
    let initial_state = Arc::new(Mutex::new(StatePayload {
        state: AssistantState::Idle,
        message: "Halo! Tekan Super+V untuk mulai ngobrol.".into(),
        pending_command: None,
        minimized: false,
    }));

    tauri::Builder::default()
        .manage(AppState(initial_state.clone()))
        .setup(move |app| {
            let handle = app.handle().clone();
            // Tulis state awal ke /tmp/ai_state.json
            broadcast_state(&handle, &initial_state);
            // Jalankan Unix Socket Listener untuk Hyprland
            spawn_unix_socket_listener(handle, initial_state.clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_current_state,
            set_manual_state,
            toggle_minimize
        ])
        .run(tauri::generate_context!())
        .expect("Error saat menjalankan aplikasi Tauri");
}