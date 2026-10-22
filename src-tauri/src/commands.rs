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
// Multi-Preset Avatar Commands
// ────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_avatar_presets() -> Result<Vec<crate::avatar::AvatarPresetMeta>, String> {
    crate::avatar::list_avatar_presets()
}

#[tauri::command]
pub async fn switch_avatar_preset(preset_id: String) -> Result<std::collections::HashMap<String, String>, String> {
    crate::avatar::switch_avatar_preset(&preset_id)
}

#[tauri::command]
pub async fn create_avatar_preset(name: String) -> Result<crate::avatar::AvatarPresetMeta, String> {
    crate::avatar::create_avatar_preset(&name)
}

#[tauri::command]
pub async fn delete_avatar_preset(preset_id: String) -> Result<(), String> {
    crate::avatar::delete_avatar_preset(&preset_id)
}

#[tauri::command]
pub async fn import_avatar_preset_zip(name: Option<String>, zip_base64: String) -> Result<crate::avatar::AvatarPresetMeta, String> {
    crate::avatar::import_avatar_preset_zip(name, &zip_base64)
}

#[tauri::command]
pub async fn export_avatar_preset_zip(preset_id: String) -> Result<String, String> {
    crate::avatar::export_avatar_preset_zip(&preset_id)
}

#[tauri::command]
pub async fn test_tts_voice(
    sample_text: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
    tts_state: State<'_, TtsPlayerState>,
) -> Result<(), String> {
    let cfg = crate::config::load_config();
    let text = sample_text.unwrap_or_else(|| {
        format!("Halo {}! Aku {}, suara baruku sudah aktif dan siap membantumu.", cfg.user_name, cfg.assistant_name)
    });

    {
        let mut s = state.0.lock().await;
        s.state = AssistantState::Speaking;
        s.message = text.clone();
    }
    broadcast_state(&app, &state.0).await;

    let app_h = app.clone();
    let s_clone = state.0.clone();
    let t_clone = tts_state.0.clone();
    tokio::spawn(async move {
        tts::speak(text, app_h, s_clone, t_clone).await;
    });

    Ok(())
}

// ────────────────────────────────────────────────────────────────────────────
// Voice Cloning Studio Commands
// ────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_cloned_voices() -> Result<Vec<crate::voice_clone::ClonedVoiceMeta>, String> {
    crate::voice_clone::list_cloned_voices()
}

#[tauri::command]
pub async fn clone_voice_from_file(
    name: String,
    engine: String,
    file_base64: String,
    filename: String,
    api_key: Option<String>,
) -> Result<crate::voice_clone::ClonedVoiceMeta, String> {
    crate::voice_clone::clone_voice_from_file(
        &name,
        &engine,
        &file_base64,
        &filename,
        api_key.as_deref(),
    ).await
}

#[tauri::command]
pub async fn save_cloned_voice_manual(
    name: String,
    engine: String,
    remote_voice_id: String,
) -> Result<crate::voice_clone::ClonedVoiceMeta, String> {
    crate::voice_clone::save_cloned_voice_manual(&name, &engine, &remote_voice_id)
}

#[tauri::command]
pub async fn set_active_cloned_voice(voice_id: String) -> Result<(), String> {
    crate::voice_clone::set_active_cloned_voice(&voice_id)
}

#[tauri::command]
pub async fn delete_cloned_voice(voice_id: String) -> Result<(), String> {
    crate::voice_clone::delete_cloned_voice(&voice_id)
}

#[tauri::command]
pub async fn get_cloned_voice_sample(voice_id: String) -> Result<String, String> {
    crate::voice_clone::get_cloned_voice_sample_base64(&voice_id)
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

    // Gunakan tauri::Manager untuk mengambil MemoryState dari AppHandle
    let history = {
        use tauri::Manager;
        let mem_state = app.state::<crate::MemoryState>();
        let mut mem = mem_state.0.lock().await;
        // Simpan pesan asli user ke memori
        mem.push_user(user_message.clone());

        // Salin seluruh history, lalu perkaya pesan terakhir dengan konteks Hyprland
        let mut msgs = mem.messages().to_vec();
        if let Some(last) = msgs.last_mut() {
            last.content = enriched_message;
        }
        msgs
    };

    // ── 5. Panggil LLM provider
    let llm_result = llm_client::chat(&config, &history)
        .await
        .map(|resp| strip_thinking_tags(&resp));

    // Jika berhasil, simpan teks balasan bersih (tanpa tag BASH:) ke MemoryState
    if let Ok(ref response_text) = llm_result {
        use tauri::Manager;
        let mem_state = app.state::<crate::MemoryState>();
        let mut mem = mem_state.0.lock().await;
        let clean_reply = bash_executor::strip_bash_command(response_text);
        if !clean_reply.is_empty() {
            mem.push_assistant(clean_reply);
        }
    }

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

#[tauri::command]
pub async fn confirm_pending_command(
    app: AppHandle,
    state: State<'_, AppState>,
    tts_state: State<'_, TtsPlayerState>,
) -> Result<(), String> {
    let pending = {
        let mut s = state.0.lock().await;
        if s.state == AssistantState::Confirm {
            let cmd = s.pending_command.take();
            s.state = AssistantState::Thinking;
            s.message = "⚡ Menjalankan perintah...".into();
            cmd
        } else {
            None
        }
    };
    broadcast_state(&app, &state.0).await;

    if let Some(bash_cmd) = pending {
        let result = bash_executor::execute_green(&bash_cmd).await;
        let mut s = state.0.lock().await;
        match result {
            Ok(out) => {
                let char_count = out.chars().count();
                let trimmed = if char_count > 1500 {
                    format!("{}...\n[Output dipotong]", out.chars().take(1500).collect::<String>())
                } else {
                    out.trim().to_string()
                };
                s.state = AssistantState::Speaking;
                s.message = format!("✅ Selesai:\n```\n{}\n```", trimmed);
                let app_h = app.clone();
                let state_arc = state.0.clone();
                let tts_arc = tts_state.0.clone();
                tokio::spawn(async move {
                    tts::speak("Perintah selesai dijalankan.".into(), app_h, state_arc, tts_arc).await;
                });
            }
            Err(e) => {
                s.state = AssistantState::Error;
                s.message = format!("❌ Error: {}", e);
            }
        }
        drop(s);
        broadcast_state(&app, &state.0).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn cancel_pending_command(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    {
        let mut s = state.0.lock().await;
        if s.state == AssistantState::Confirm {
            s.state = AssistantState::Idle;
            s.pending_command = None;
            s.message = "❌ Perintah dibatalkan oleh pengguna.".into();
        }
    }
    broadcast_state(&app, &state.0).await;
    Ok(())
}

#[tauri::command]
pub async fn clear_chat_memory(
    memory: State<'_, crate::MemoryState>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    memory.0.lock().await.clear();
    {
        let mut s = state.0.lock().await;
        s.state = AssistantState::Idle;
        s.message = "🧹 Ingatan percakapan telah di-reset!".into();
    }
    broadcast_state(&app, &state.0).await;
    Ok(())
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
        cfg.model_name.clone()
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

    let system_prompt = build_system_prompt(&cfg);

    LlmConfig {
        provider,
        api_key,
        base_url,
        model,
        system_prompt,
    }
}

pub fn build_system_prompt(cfg: &config::AppConfig) -> String {
    let assistant_name = if !cfg.assistant_name.trim().is_empty() {
        cfg.assistant_name.trim()
    } else {
        "Aoi"
    };

    let user_name = if !cfg.user_name.trim().is_empty() {
        cfg.user_name.trim()
    } else {
        "Kak"
    };

    let personality_desc = if !cfg.custom_personality.trim().is_empty() {
        cfg.custom_personality.trim().to_string()
    } else {
        match cfg.personality_preset.as_str() {
            "tsundere" => {
                "Sifat: Tsundere imut. Sering gengsi, agak ketus di awal tapi sebenarnya sangat peduli, loyal, dan perhatian. \
                 Gunakan ekspresi khas seperti 'Hmph!', 'B-bukan karena aku peduli ya!', tapi selalu siap membantu.".to_string()
            }
            "caring" => {
                "Sifat: Lembut, dewasa, dan penuh perhatian (Onee-san / Pengayom). \
                 Berbicara dengan nada menenangkan, hangat, menyemangati, dan sangat peduli pada kenyamanan pengguna.".to_string()
            }
            "cool" => {
                "Sifat: Cool dan kalem (Kuudere). \
                 Berbicara ringkas, nada tenang, tidak banyak basa-basi, sangat cerdas, elegan, dan selalu dapat diandalkan.".to_string()
            }
            _ => {
                // "cheerful" (default)
                "Sifat: Ceria, manis, dan bersemangat (Genki Waifu). \
                 Gunakan gaya bahasa santai anak muda, ekspresif, hangat, ramah, dan menyenangkan didengar.".to_string()
            }
        }
    };

    format!(
        "Kamu adalah {name}, asisten AI desktop pendamping virtual yang hidup di sistem CachyOS (Arch Linux) dengan Hyprland Wayland compositor.\n\
         Lawan bicaramu adalah {user}. Selalu panggil dia dengan sebutan '{user}'.\n\n\
         [KEPRIBADIAN & GAYA BICARA]\n\
         {personality}\n\n\
         [ATURAN PENTING GAYA JAWABAN - WAJIB PATUH]:\n\
         1. SANGAT PENTING: JANGAN PANJANG-PANJANG! Jawabanmu langsung disuarakan lewat audio Text-to-Speech (TTS). Batasi jawaban obrolan antara 1 hingga 3 kalimat saja. Dilarang menulis esai atau paragraf panjang bertele-tele!\n\
         2. MANUSIAWI & ALAMI: Bicaralah seperti teman dekat manusia dalam Bahasa Indonesia yang santai, luwes, dan hidup. Hindari gaya bahasa kaku formal buku atau robot customer care.\n\
         3. PERINTAH TERMINAL: Jika {user} meminta perintah Linux/terminal atau kamu perlu memeriksa sistem, letakkan perintah di baris baru dengan format tepat: BASH:<perintah>\n\
            Contoh: BASH:pactl set-sink-volume @DEFAULT_SINK@ +10%\n\
            Jangan berikan penjelasan atau markdown di dalam baris BASH:, hanya perintah mentah.",
        name = assistant_name,
        user = user_name,
        personality = personality_desc
    )
}

pub fn strip_thinking_tags(raw: &str) -> String {
    let mut text = raw.to_string();
    while let Some(start) = text.find("<thinking>") {
        if let Some(end) = text[start..].find("</thinking>") {
            let actual_end = start + end + "</thinking>".len();
            text.replace_range(start..actual_end, "");
        } else {
            text.truncate(start);
            break;
        }
    }
    if let Some(end) = text.find("</thinking>") {
        text = text[end + "</thinking>".len()..].to_string();
    }
    text.trim().to_string()
}

#[tauri::command]
pub async fn update_window_mode(
    app: AppHandle,
    expanded: bool,
    avatar_size: u32,
) -> Result<(), String> {
    if let Some(win) = tauri::Manager::get_webview_window(&app, "main") {
        let (target_w, target_h) = if expanded {
            (360, 520)
        } else {
            let s = (avatar_size + 24).clamp(96, 320);
            (s, s)
        };

        // Resize the Tauri window
        let _ = win.set_size(tauri::Size::Logical(tauri::LogicalSize {
            width: target_w as f64,
            height: target_h as f64,
        }));

        #[cfg(target_os = "linux")]
        {
            use webkit2gtk::WebViewExt;
            let _ = win.with_webview(|webview| {
                let wv = webview.inner();
                let rgba = gdk::RGBA::new(0.0, 0.0, 0.0, 0.0);
                wv.set_background_color(&rgba);
            });
        }

        // Adjust Hyprland position to keep the bottom-right corner anchored
        #[cfg(target_os = "linux")]
        {
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
                            let target_x = (w - (target_w as i64) - 20).max(0);
                            let target_y = (h - (target_h as i64) - 20).max(0);
                            let dispatch_cmd = format!(
                                "hl.dsp.window.move({{ window = \"class:.*ai-hypr-assistant.*\", x = {}, y = {}, exact = true }})",
                                target_x, target_y
                            );
                            let _ = tokio::process::Command::new("hyprctl")
                                .args(["dispatch", &dispatch_cmd])
                                .output()
                                .await;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

