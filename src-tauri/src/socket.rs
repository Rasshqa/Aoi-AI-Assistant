// src-tauri/src/socket.rs
// Unix Domain Socket listener — menerima sinyal dari keybind Hyprland via socat.
// Protokol:
//   toggle_mic  (Super + V) → Mulai rekam / Selesai rekam (STT) / Barge-in
//   toggle_hide (Super + H) → Sembunyikan / Tampilkan avatar
//   confirm_yes (Alt + Y)   → Eksekusi Red Tier command yang tertunda
//   confirm_no  (Alt + N)   → Batalkan perintah Red Tier

use std::path::Path;
use std::sync::Arc;

use tauri::AppHandle;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixListener;
use tokio::process::Child;
use tokio::sync::Mutex;

use crate::bash_executor;
use crate::broadcast_state;
use crate::commands;
use crate::config;
use crate::state::{AssistantState, StatePayload};
use crate::stt;
use crate::tts;

const SOCKET_PATH: &str = "/tmp/ai-assistant.sock";

// ────────────────────────────────────────────────────────────────────────────
// Socket Server
// ────────────────────────────────────────────────────────────────────────────

pub async fn spawn_unix_socket_listener(
    app: AppHandle,
    shared: Arc<Mutex<StatePayload>>,
    tts_player: Arc<Mutex<Option<Child>>>,
    audio_recorder: Arc<Mutex<Option<Child>>>,
) {
    if Path::new(SOCKET_PATH).exists() {
        let _ = tokio::fs::remove_file(SOCKET_PATH).await;
    }

    let listener =
        UnixListener::bind(SOCKET_PATH).expect("Gagal bind Unix Socket /tmp/ai-assistant.sock");

    println!("🚀 IPC Socket aktif di: {}", SOCKET_PATH);

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let app = app.clone();
                let shared = shared.clone();
                let tts_player = tts_player.clone();
                let audio_recorder = audio_recorder.clone();

                tokio::spawn(async move {
                    let reader = BufReader::new(stream);
                    let mut lines = reader.lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        handle_command(
                            line.trim(),
                            &app,
                            &shared,
                            &tts_player,
                            &audio_recorder,
                        )
                        .await;
                    }
                });
            }
            Err(e) => eprintln!("Socket accept error: {}", e),
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Command Handler
// ────────────────────────────────────────────────────────────────────────────

async fn handle_command(
    cmd: &str,
    app: &AppHandle,
    shared: &Arc<Mutex<StatePayload>>,
    tts_player: &Arc<Mutex<Option<Child>>>,
    audio_recorder: &Arc<Mutex<Option<Child>>>,
) {
    match cmd {
        "toggle_mic" => {
            handle_smart_toggle(app, shared, tts_player, audio_recorder).await;
        }

        "toggle_hide" => {
            shared.lock().await.minimized ^= true;
            broadcast_state(app, shared).await;
        }

        "confirm_yes" => {
            let pending = {
                let mut s = shared.lock().await;
                if s.state == AssistantState::Confirm {
                    let cmd = s.pending_command.take();
                    s.state = AssistantState::Thinking;
                    s.message = "✅ Menjalankan perintah...".into();
                    cmd
                } else {
                    None
                }
            };
            broadcast_state(app, shared).await;

            if let Some(bash_cmd) = pending {
                let result = bash_executor::execute_green(&bash_cmd).await;
                let mut s = shared.lock().await;
                match result {
                    Ok(out) => {
                        s.state = AssistantState::Speaking;
                        s.message = format!("✅ Selesai:\n{}", out.trim());
                        let app_h = app.clone();
                        let state_arc = shared.clone();
                        let tts_arc = tts_player.clone();
                        tokio::spawn(async move {
                            tts::speak(
                                "Perintah berhasil dieksekusi.".into(),
                                app_h,
                                state_arc,
                                tts_arc,
                            )
                            .await;
                        });
                    }
                    Err(e) => {
                        s.state = AssistantState::Error;
                        s.message = format!("❌ Error: {}", e);
                    }
                }
                drop(s);
                broadcast_state(app, shared).await;
            }
        }

        "confirm_no" => {
            let mut s = shared.lock().await;
            if s.state == AssistantState::Confirm {
                s.state = AssistantState::Idle;
                s.pending_command = None;
                s.message = "❌ Perintah dibatalkan oleh pengguna.".into();
            }
            drop(s);
            broadcast_state(app, shared).await;
        }

        // Testing manual state via CLI
        _ if cmd.starts_with("state:") => {
            let target = cmd.trim_start_matches("state:");
            let mut s = shared.lock().await;
            match target {
                "idle" => {
                    s.state = AssistantState::Idle;
                    s.message = "Standby... Tekan Super+V untuk bicara.".into();
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
                    s.message = "Halo! Ada yang bisa aku bantu di CachyOS?".into();
                }
                "confirm" => {
                    s.state = AssistantState::Confirm;
                    s.message = "Izinkan saya menjalankan perintah ini?".into();
                    s.pending_command = Some("sudo pacman -Syu".into());
                }
                "error" => {
                    s.state = AssistantState::Error;
                    s.message = "Waduh, koneksi API gagal!".into();
                }
                _ => {}
            }
            drop(s);
            broadcast_state(app, shared).await;
        }

        _ => eprintln!("Perintah socket tidak dikenal: {}", cmd),
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Smart Keybind Handler (Super + V)
// ────────────────────────────────────────────────────────────────────────────

pub async fn handle_smart_toggle(
    app: &AppHandle,
    shared: &Arc<Mutex<StatePayload>>,
    tts_player: &Arc<Mutex<Option<Child>>>,
    audio_recorder: &Arc<Mutex<Option<Child>>>,
) {
    let current_state = shared.lock().await.state.clone();

    match current_state {
        // ── Dari diam / error → Mulai rekam mikrofon PipeWire
        AssistantState::Idle | AssistantState::Error => {
            match stt::start_recording(audio_recorder).await {
                Ok(()) => {
                    let mut s = shared.lock().await;
                    s.state = AssistantState::Listening;
                    s.message = "🎙️ Mendengarkan... (Tekan Super+V lagi untuk kirim)".into();
                }
                Err(e) => {
                    let mut s = shared.lock().await;
                    s.state = AssistantState::Error;
                    s.message = format!("❌ Mic error: {}", e);
                }
            }
            broadcast_state(app, shared).await;
        }

        // ── Dari mendengarkan → Selesai bicara, transkripsi & kirim ke LLM
        AssistantState::Listening => {
            // Hentikan rekaman audio
            let record_res = stt::stop_recording(audio_recorder).await;

            match record_res {
                Ok(wav_path) => {
                    {
                        let mut s = shared.lock().await;
                        s.state = AssistantState::Thinking;
                        s.message = "🧠 Mentranskripsi suara...".into();
                    }
                    broadcast_state(app, shared).await;

                    let app_clone = app.clone();
                    let shared_clone = shared.clone();
                    let tts_clone = tts_player.clone();

                    tokio::spawn(async move {
                        let cfg = config::load_config();
                        match stt::transcribe(&wav_path, &cfg).await {
                            Ok(user_text) => {
                                println!("Transkripsi: {}", user_text);
                                let _ = commands::run_ai_pipeline(
                                    user_text,
                                    app_clone,
                                    shared_clone,
                                    tts_clone,
                                )
                                .await;
                            }
                            Err(e) => {
                                eprintln!("STT error: {}", e);
                                let mut s = shared_clone.lock().await;
                                s.state = AssistantState::Error;
                                s.message = format!("❌ STT: {}", e);
                                drop(s);
                                broadcast_state(&app_clone, &shared_clone).await;
                            }
                        }
                    });
                }
                Err(e) => {
                    let mut s = shared.lock().await;
                    s.state = AssistantState::Idle;
                    s.message = format!("Mikrofon standby: {}", e);
                    drop(s);
                    broadcast_state(app, shared).await;
                }
            }
        }

        // ── BARGE-IN (Interupsi): Langsung hentikan TTS dan mulai rekam ulang
        AssistantState::Thinking | AssistantState::Speaking => {
            tts::stop(tts_player).await;

            match stt::start_recording(audio_recorder).await {
                Ok(()) => {
                    let mut s = shared.lock().await;
                    s.state = AssistantState::Listening;
                    s.message = "⚡ Interupsi! Silakan bicara...".into();
                }
                Err(e) => {
                    let mut s = shared.lock().await;
                    s.state = AssistantState::Error;
                    s.message = format!("❌ Gagal mulai rekam: {}", e);
                }
            }
            broadcast_state(app, shared).await;
        }

        // ── Batalkan konfirmasi jika menekan Super+V saat confirm
        AssistantState::Confirm => {
            let mut s = shared.lock().await;
            s.state = AssistantState::Idle;
            s.pending_command = None;
            s.message = "Eksekusi perintah dibatalkan.".into();
            drop(s);
            broadcast_state(app, shared).await;
        }
    }
}
