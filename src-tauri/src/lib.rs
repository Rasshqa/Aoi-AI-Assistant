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
pub mod voice_clone;

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

            if let Some(win) = tauri::Manager::get_webview_window(app, "main") {
                println!(">>> WIN FOUND: {}", win.label());
                let _ = win.set_visible_on_all_workspaces(true);
                let _ = win.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)));

                #[cfg(target_os = "linux")]
                {
                    use gtk::prelude::*;
                    use webkit2gtk::WebViewExt;

                    if let Ok(gtk_win) = win.gtk_window() {
                        gtk_win.set_decorated(false);
                        gtk_win.set_border_width(0);
                        gtk_win.set_app_paintable(true);
                        if let Some(screen) = gtk::prelude::GtkWindowExt::screen(&gtk_win) {
                            if let Some(visual) = screen.rgba_visual() {
                                gtk_win.set_visual(Some(&visual));
                            }
                        }
                        gtk_win.connect_draw(|win, cr| {
                            cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
                            cr.set_operator(gtk::cairo::Operator::Source);
                            let _ = cr.paint();
                            cr.set_operator(gtk::cairo::Operator::Over);
                            for child in win.children() {
                                win.propagate_draw(&child, cr);
                            }
                            gtk::glib::Propagation::Stop
                        });

                        let provider = gtk::CssProvider::new();
                        let _ = provider.load_from_data(
                            b"window, decoration, .background, .titlebar, headerbar, viewport, box, webview, widget, * {\n\
                                background-color: transparent !important;\n\
                                background: none !important;\n\
                                border: none !important;\n\
                                border-width: 0px !important;\n\
                                box-shadow: none !important;\n\
                                border-radius: 0px !important;\n\
                                outline: none !important;\n\
                                margin: 0px !important;\n\
                                padding: 0px !important;\n\
                            }\n"
                        );
                        gtk_win.style_context().add_provider(
                            &provider,
                            gtk::STYLE_PROVIDER_PRIORITY_USER,
                        );
                    }

                    let res = win.with_webview(|webview| {
                        let wv = webview.inner();
                        let rgba = gdk::RGBA::new(0.0, 0.0, 0.0, 0.0);
                        wv.set_background_color(&rgba);
                    });
                    println!(">>> WITH_WEBVIEW RESULT: {:?}", res);
                }
            }

            if let Some(settings_win) = tauri::Manager::get_webview_window(app, "settings") {
                #[cfg(target_os = "linux")]
                {
                    use webkit2gtk::WebViewExt;
                    let _ = settings_win.with_webview(|webview| {
                        let wv = webview.inner();
                        let rgba = gdk::RGBA::new(9.0 / 255.0, 9.0 / 255.0, 11.0 / 255.0, 1.0);
                        wv.set_background_color(&rgba);
                    });
                }
            }

            tauri::async_runtime::spawn(async move {
                // Posisikan window di pojok kanan bawah monitor aktif Hyprland agar proporsional
                tokio::time::sleep(tokio::time::Duration::from_millis(350)).await;
                if let Ok(output) = tokio::process::Command::new("hyprctl")
                    .args(["monitors", "-j"])
                    .output()
                    .await
                {
                    if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                        if let Some(monitors) = json.as_array() {
                            let focused = monitors.iter().find(|m| m["focused"].as_bool().unwrap_or(false))
                                .or_else(|| monitors.first());
                            if let Some(m) = focused {
                                let w = m["width"].as_i64().unwrap_or(1920);
                                let h = m["height"].as_i64().unwrap_or(1080);
                                let target_x = (w - 340 - 24).max(0);
                                let target_y = (h - 440 - 24).max(0);

                                // 1. Set posisi langsung via Tauri Window API
                                if let Some(main_win) = tauri::Manager::get_webview_window(&handle, "main") {
                                    let _ = main_win.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                                        x: target_x as i32,
                                        y: target_y as i32,
                                    }));
                                }

                                // 2. Fallback via hyprctl standar
                                let move_arg = format!("exact {} {},class:^(ai-hypr-assistant)$", target_x, target_y);
                                let _ = tokio::process::Command::new("hyprctl")
                                    .args(["dispatch", "movewindowpixel", &move_arg])
                                    .output()
                                    .await;
                            }
                        }
                    }
                }

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
            commands::update_window_mode,
            commands::list_avatar_presets,
            commands::switch_avatar_preset,
            commands::create_avatar_preset,
            commands::delete_avatar_preset,
            commands::import_avatar_preset_zip,
            commands::export_avatar_preset_zip,
            commands::test_tts_voice,
            commands::list_cloned_voices,
            commands::clone_voice_from_file,
            commands::save_cloned_voice_manual,
            commands::set_active_cloned_voice,
            commands::delete_cloned_voice,
            commands::get_cloned_voice_sample,
            commands::open_settings_window,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "settings" {
                    let _ = window.hide();
                    api.prevent_close();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error saat menjalankan Aoi AI Assistant");
}
