// src-tauri/src/commands.rs
// Semua Tauri commands yang bisa di-invoke dari Vue frontend via `invoke(...)`.
//
// Commands:
//   get_current_state    → ambil snapshot state saat ini
//   set_manual_state     → paksa ubah state (untuk testing/dev toolbar)
//   toggle_minimize      → toggle minimize avatar
//   ask_llm              → kirim pesan user ke LLM, tangani tool calls, update state, trigger TTS
//   get_hyprland_context → ambil konteks desktop aktif (window + clipboard)
//   execute_command      → eksekusi Green Tier command dari frontend
//   speak_text           → memicu engine TTS lokal secara langsung
//   get_config           → baca konfigurasi (~/.config/aoi/config.json)
//   save_config          → simpan konfigurasi (~/.config/aoi/config.json)

use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{AppHandle, State};

use crate::state::{AppState, AssistantState, StatePayload};
use crate::broadcast_state;
use crate::llm_client::{self, LlmConfig};
use crate::hyprland;
use crate::bash_executor;
use crate::tts::{self, TtsPlayerState};
use crate::config::{self, AppConfig};

// ────────────────────────────────────────────────────────────────────────────
// State Commands
// ────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_current_state(state: State<'_, AppState>) -> Result<StatePayload, String> {
    Ok(state.0.lock().await.clone())
}

#[tauri::command]
pub async fn set_manual_state(
    new_state: AssistantState,
    message: String,
    command: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    {
        let mut s = state.0.lock().await;
        s.state = new_state;
        s.message = message;
        s.pending_command = command;
    }
    broadcast_state(&app, &state.0).await;
    Ok(())
}

#[tauri::command]
pub async fn toggle_minimize(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    {
        let mut s = state.0.lock().await;
        s.minimized = !s.minimized;
    }
    broadcast_state(&app, &state.0).await;
    Ok(())
}

#[tauri::command]
pub async fn trigger_mic(
    app: AppHandle,
    state: State<'_, AppState>,
    tts_state: State<'_, TtsPlayerState>,
    audio_recorder: State<'_, crate::stt::AudioRecorderState>,
) -> Result<(), String> {
    crate::socket::handle_smart_toggle(&app, &state.0, &tts_state.0, &audio_recorder.0).await;
    Ok(())
}

// ────────────────────────────────────────────────────────────────────────────
// Config Commands
// ────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_config() -> Result<AppConfig, String> {
    Ok(config::load_config())
}

#[tauri::command]
pub async fn save_config(cfg: AppConfig) -> Result<(), String> {
    config::save_config(&cfg)
}

// ────────────────────────────────────────────────────────────────────────────
// Avatar Sprite Commands
// ────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn save_avatar_sprite(frame_name: String, data_url: String) -> Result<String, String> {
    crate::avatar::save_avatar_sprite(&frame_name, &data_url)
}

#[tauri::command]
pub async fn save_avatar_pack(pack: std::collections::HashMap<String, String>) -> Result<(), String> {
    crate::avatar::save_avatar_pack(pack)
}

#[tauri::command]
pub async fn load_avatar_pack(folder_override: Option<String>) -> Result<std::collections::HashMap<String, String>, String> {
    crate::avatar::load_avatar_pack(folder_override)
}

#[tauri::command]
pub async fn reset_avatar_pack() -> Result<(), String> {
    crate::avatar::reset_avatar_pack()
}

// ────────────────────────────────────────────────────────────────────────────
// LLM Pipeline — shared antara chat input dan input suara STT
// ────────────────────────────────────────────────────────────────────────────

pub async fn run_ai_pipeline(
    user_message: String,
    app: AppHandle,
    state_arc: Arc<Mutex<StatePayload>>,
    tts_arc: Arc<Mutex<Option<tokio::process::Child>>>,
) -> Result<String, String> {
    // ── 1. Transisi ke Thinking
    {
        let mut s = state_arc.lock().await;
        s.state = AssistantState::Thinking;
        s.message = "Sedang memikirkan jawaban...".into();
    }
    broadcast_state(&app, &state_arc).await;

    // ── 2. Kumpulkan konteks Hyprland (window aktif & clipboard)
    let ctx = hyprland::build_context(false).await;
    let context_str = hyprland::format_context_for_prompt(&ctx);

    // ── 3. Bangun pesan dengan konteks desktop
    let enriched_message = if context_str.trim().is_empty() {
        user_message.clone()
    } else {
        format!("{}\n\n{}", user_message, context_str)
    };

    // ── 4. Konfigurasi LLM dari file config / env
    let config = build_llm_config();

    let history = vec![llm_client::ChatMessage {
        role: "user".into(),
        content: enriched_message,
    }];

    // ── 5. Panggil LLM provider
    let llm_result = llm_client::chat(&config, &history).await;

    match llm_result {
        Err(e) => {
            let mut s = state_arc.lock().await;
            s.state = AssistantState::Error;
            s.message = format!("❌ Error LLM: {}", e);
            drop(s);
            broadcast_state(&app, &state_arc).await;
            Err(e)
        }
        Ok(response) => {
            // ── 6. Cek tool call BASH:<perintah>
            let bash_cmd = bash_executor::extract_bash_command(&response);
            let text_reply = bash_executor::strip_bash_command(&response);

            if let Some(cmd) = bash_cmd {
                let classification = bash_executor::classify(&cmd);

                match classification.tier {
                    bash_executor::CommandTier::Green => {
                        // Langsung eksekusi (Green Tier)
                        let exec_result = bash_executor::execute_green(&cmd).await;
                        let mut s = state_arc.lock().await;
                        match exec_result {
                            Ok(output) => {
                                s.state = AssistantState::Speaking;
                                let reply = if text_reply.is_empty() {
                                    format!("✅ Selesai!\n```\n{}\n```", output.trim())
                                } else {
                                    format!("{}\n\n✅ Output:\n```\n{}\n```", text_reply, output.trim())
                                };
                                s.message = reply.clone();
                                drop(s);
                                broadcast_state(&app, &state_arc).await;

                                // Trigger TTS
                                let speech_text = if !text_reply.is_empty() {
                                    text_reply.clone()
                                } else {
                                    "Perintah berhasil dieksekusi.".to_string()
                                };
                                let app_h = app.clone();
                                let s_clone = state_arc.clone();
                                let t_clone = tts_arc.clone();
                                tokio::spawn(async move {
                                    tts::speak(speech_text, app_h, s_clone, t_clone).await;
                                });

                                Ok(reply)
                            }
                            Err(e) => {
                                s.state = AssistantState::Error;
                                s.message = format!("❌ Gagal: {}", e);
                                drop(s);
                                broadcast_state(&app, &state_arc).await;
                                Err(e)
                            }
                        }
                    }
                    bash_executor::CommandTier::Red => {
                        // Butuh konfirmasi (Red Tier)
                        let mut s = state_arc.lock().await;
                        s.state = AssistantState::Confirm;
                        s.pending_command = Some(cmd.clone());
                        s.message = if text_reply.is_empty() {
                            format!("Izinkan saya menjalankan perintah ini?")
                        } else {
                            text_reply.clone()
                        };
                        drop(s);
                        broadcast_state(&app, &state_arc).await;
                        Ok(format!(
                            "{}\n\n⚠️ Perintah memerlukan konfirmasi: `{}`",
                            text_reply, cmd
                        ))
                    }
                }
            } else {
                // ── Balasan teks biasa → Speaking + TTS + RMS Lip-Sync
                let mut s = state_arc.lock().await;
                s.state = AssistantState::Speaking;
                s.message = response.clone();
                drop(s);
                broadcast_state(&app, &state_arc).await;

                // Trigger TTS
                let speech_text = tts::clean_text_for_tts(&response);
                let app_h = app.clone();
                let s_clone = state_arc.clone();
                let t_clone = tts_arc.clone();
                tokio::spawn(async move {
                    tts::speak(speech_text, app_h, s_clone, t_clone).await;
                });

                Ok(response)
            }
        }
    }
}

#[tauri::command]
pub async fn ask_llm(
    user_message: String,
    app: AppHandle,
    state: State<'_, AppState>,
    tts_state: State<'_, TtsPlayerState>,
) -> Result<String, String> {
    run_ai_pipeline(user_message, app, state.0.clone(), tts_state.0.clone()).await
}

// ────────────────────────────────────────────────────────────────────────────
// Direct TTS Command
// ────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn speak_text(
    text: String,
    app: AppHandle,
    state: State<'_, AppState>,
    tts_state: State<'_, TtsPlayerState>,
) -> Result<(), String> {
    {
        let mut s = state.0.lock().await;
        s.state = AssistantState::Speaking;
        s.message = text.clone();
    }
    broadcast_state(&app, &state.0).await;

    let app_h = app.clone();
    let state_arc = state.0.clone();
    let tts_arc = tts_state.0.clone();
    tokio::spawn(async move {
        tts::speak(text, app_h, state_arc, tts_arc).await;
    });

    Ok(())
}

// ────────────────────────────────────────────────────────────────────────────
// Hyprland Context Command
// ────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_hyprland_context() -> Result<hyprland::HyprlandContext, String> {
    Ok(hyprland::build_context(false).await)
}

// ────────────────────────────────────────────────────────────────────────────
// Direct Execute Command (dari UI)
// ────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn execute_command(
    command: String,
    app: AppHandle,
    state: State<'_, AppState>,
    tts_state: State<'_, TtsPlayerState>,
) -> Result<String, String> {
    if bash_executor::is_red_tier(&command) {
        return Err("Perintah ini memerlukan konfirmasi via keybind Alt+Y".into());
    }

    let result = bash_executor::execute_green(&command).await?;

    {
        let mut s = state.0.lock().await;
        s.state = AssistantState::Speaking;
        s.message = format!("✅ Output:\n{}", result.trim());
    }
    broadcast_state(&app, &state.0).await;

    let app_h = app.clone();
    let state_arc = state.0.clone();
    let tts_arc = tts_state.0.clone();
    tokio::spawn(async move {
        tts::speak("Perintah selesai dijalankan.".into(), app_h, state_arc, tts_arc).await;
    });

    Ok(result)
}

// ────────────────────────────────────────────────────────────────────────────
// Helper: Membangun LlmConfig dari config.json + env fallback
// ────────────────────────────────────────────────────────────────────────────

pub fn build_llm_config() -> LlmConfig {
    let cfg = config::load_config();
    let p_lower = cfg.llm_provider.to_lowercase();

    let provider = match p_lower.as_str() {
        "openai" | "groq" | "openrouter" | "custom" => llm_client::LlmProvider::OpenAI,
        "ollama" => llm_client::LlmProvider::Ollama,
        _ => llm_client::LlmProvider::Gemini,
    };

    let api_key = match provider {
        llm_client::LlmProvider::Gemini => {
            if !cfg.gemini_api_key.is_empty() {
                Some(cfg.gemini_api_key.clone())
            } else {
                std::env::var("GEMINI_API_KEY").ok()
            }
        }
        llm_client::LlmProvider::OpenAI => {
            if p_lower == "custom" {
                if !cfg.custom_api_key.is_empty() {
                    Some(cfg.custom_api_key.clone())
                } else {
                    std::env::var("AOI_CUSTOM_API_KEY").ok()
                }
            } else if p_lower == "groq" && !cfg.groq_api_key.is_empty() {
                Some(cfg.groq_api_key.clone())
            } else if p_lower == "openrouter" && !cfg.openrouter_api_key.is_empty() {
                Some(cfg.openrouter_api_key.clone())
            } else if !cfg.openai_api_key.is_empty() {
                Some(cfg.openai_api_key.clone())
            } else {
                std::env::var("OPENAI_API_KEY")
                    .or_else(|_| std::env::var("GROQ_API_KEY"))
                    .or_else(|_| std::env::var("OPENROUTER_API_KEY"))
                    .ok()
            }
        }
        llm_client::LlmProvider::Ollama => None,
    };

    let base_url = if p_lower == "custom" {
        if !cfg.custom_api_endpoint.is_empty() {
            Some(cfg.custom_api_endpoint.clone())
        } else {
            std::env::var("AOI_CUSTOM_ENDPOINT").ok()
        }
    } else if p_lower == "groq" {
        Some("https://api.groq.com/openai/v1".into())
    } else if p_lower == "openrouter" {
        Some("https://openrouter.ai/api/v1".into())
    } else if provider == llm_client::LlmProvider::Ollama {
        Some(cfg.ollama_base_url.clone())
    } else {
        std::env::var("AOI_BASE_URL").ok()
    };

    let model = if !cfg.model_name.is_empty() {
        cfg.model_name
    } else {
        match provider {
            llm_client::LlmProvider::Gemini => "gemini-2.0-flash".into(),
            llm_client::LlmProvider::OpenAI => {
                if p_lower == "groq" {
                    "llama-3.3-70b-versatile".into()
                } else if p_lower == "custom" {
                    "default".into()
                } else {
                    "gpt-4o-mini".into()
                }
            }
            llm_client::LlmProvider::Ollama => "llama3.2".into(),
        }
    };

    LlmConfig {
        provider,
        api_key,
        base_url,
        model,
        system_prompt: concat!(
            "Kamu adalah Aoi, asisten AI desktop pendamping bergaya anime yang ringan dan cerdas, ",
            "berjalan di CachyOS (Arch Linux) dengan Hyprland Wayland compositor. ",
            "Kamu membantu pengguna dengan tugas sehari-hari, coding, troubleshooting sistem Linux, ",
            "dan administrasi Arch Linux. Jawab dengan ramah, ringkas, dan teknis jika diperlukan. ",
            "Jika kamu perlu menjalankan perintah shell untuk membantu user, tuliskan di baris terpisah ",
            "dengan format tepat: BASH:<perintah>\n",
            "Contoh: BASH:ls -la ~/\n",
            "Jangan pernah menambahkan penjelasan di dalam tag BASH, hanya perintahnya saja."
        ).into(),
    }
}
