// src-tauri/src/config.rs
// Manajemen konfigurasi persistent (~/.config/aoi/config.json)
// Mengatur API key, provider LLM, path model TTS & Whisper, serta folder avatar.

use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub llm_provider: String,         // "gemini", "groq", "openai", "openrouter", "ollama", "custom"
    pub gemini_api_key: String,
    pub groq_api_key: String,
    pub openai_api_key: String,
    pub openrouter_api_key: String,
    pub ollama_base_url: String,

    #[serde(default)]
    pub custom_api_key: String,
    #[serde(default)]
    pub custom_api_endpoint: String,  // e.g. "https://api.deepseek.com/v1" atau "http://localhost:8000/v1"

    pub model_name: String,
    pub tts_model_path: String,
    pub whisper_model_path: String,
    pub avatar_folder_path: String,

    // Fitur Kustomisasi Nama & Karakter / Personality
    #[serde(default = "default_assistant_name")]
    pub assistant_name: String,
    #[serde(default = "default_user_name")]
    pub user_name: String,
    #[serde(default = "default_personality_preset")]
    pub personality_preset: String,
    #[serde(default)]
    pub custom_personality: String,

    #[serde(default = "default_avatar_size")]
    pub avatar_size: u32,

    // Konfigurasi Voice Engine & Voice Cloning
    #[serde(default = "default_tts_engine")]
    pub tts_engine: String, // "edge_tts", "elevenlabs", "openai_speech", "piper"
    #[serde(default = "default_edge_tts_voice")]
    pub edge_tts_voice: String, // e.g. "id-ID-GadisNeural", "ja-JP-NanamiNeural"
    #[serde(default = "default_edge_tts_rate")]
    pub edge_tts_rate: String, // e.g. "+10%"
    #[serde(default)]
    pub elevenlabs_api_key: String,
    #[serde(default)]
    pub elevenlabs_voice_id: String,
    #[serde(default)]
    pub openai_speech_endpoint: String,
    #[serde(default)]
    pub openai_speech_key: String,
    #[serde(default = "default_openai_speech_voice")]
    pub openai_speech_voice: String,

    // Active Avatar Preset
    #[serde(default = "default_active_avatar_preset")]
    pub active_avatar_preset: String,

    // Active Cloned Voice & Integrations
    #[serde(default)]
    pub active_cloned_voice_id: String,
    #[serde(default)]
    pub fish_audio_api_key: String,
    #[serde(default)]
    pub colab_clone_endpoint: String,
}

fn default_tts_engine() -> String {
    "piper".to_string()
}

fn default_edge_tts_voice() -> String {
    "id-ID-GadisNeural".to_string()
}

fn default_edge_tts_rate() -> String {
    "+10%".to_string()
}

fn default_openai_speech_voice() -> String {
    "nova".to_string()
}

fn default_active_avatar_preset() -> String {
    "default".to_string()
}

fn default_avatar_size() -> u32 {
    144
}

fn default_assistant_name() -> String {
    "Aoi".to_string()
}

fn default_user_name() -> String {
    "Kak".to_string()
}

fn default_personality_preset() -> String {
    "caring".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        let default_piper_model = PathBuf::from(&home).join(".local/share/piper/mommy_lessac.onnx");
        let initial_tts_model = std::env::var("AOI_TTS_MODEL").unwrap_or_else(|_| {
            if default_piper_model.exists() {
                default_piper_model.to_string_lossy().to_string()
            } else {
                String::new()
            }
        });

        Self {
            llm_provider: std::env::var("AOI_LLM_PROVIDER").unwrap_or_else(|_| "gemini".into()),
            gemini_api_key: std::env::var("GEMINI_API_KEY").unwrap_or_default(),
            groq_api_key: std::env::var("GROQ_API_KEY").unwrap_or_default(),
            openai_api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            openrouter_api_key: std::env::var("OPENROUTER_API_KEY").unwrap_or_default(),
            ollama_base_url: std::env::var("AOI_BASE_URL").unwrap_or_else(|_| "http://localhost:11434".into()),
            custom_api_key: std::env::var("AOI_CUSTOM_API_KEY").unwrap_or_default(),
            custom_api_endpoint: std::env::var("AOI_CUSTOM_ENDPOINT").unwrap_or_default(),
            model_name: std::env::var("AOI_MODEL").unwrap_or_else(|_| "gemini-2.0-flash".into()),
            tts_model_path: initial_tts_model,
            whisper_model_path: std::env::var("AOI_WHISPER_MODEL").unwrap_or_default(),
            avatar_folder_path: String::new(),
            assistant_name: std::env::var("AOI_ASSISTANT_NAME").unwrap_or_else(|_| "Aoi".into()),
            user_name: std::env::var("AOI_USER_NAME").unwrap_or_else(|_| "Kak".into()),
            personality_preset: std::env::var("AOI_PERSONALITY_PRESET").unwrap_or_else(|_| "caring".into()),
            custom_personality: std::env::var("AOI_CUSTOM_PERSONALITY").unwrap_or_default(),
            avatar_size: 144,
            tts_engine: "piper".into(),
            edge_tts_voice: "id-ID-GadisNeural".into(),
            edge_tts_rate: "+10%".into(),
            elevenlabs_api_key: String::new(),
            elevenlabs_voice_id: String::new(),
            openai_speech_endpoint: String::new(),
            openai_speech_key: String::new(),
            openai_speech_voice: "nova".into(),
            active_avatar_preset: "default".into(),
            active_cloned_voice_id: String::new(),
            fish_audio_api_key: String::new(),
            colab_clone_endpoint: String::new(),
        }
    }
}

pub fn expand_user_path(raw: &str) -> PathBuf {
    let mut cleaned = raw.trim().trim_end_matches('.');
    cleaned = cleaned.trim();
    if cleaned.starts_with("~/") || cleaned == "~" {
        if let Ok(home) = std::env::var("HOME") {
            let sub = cleaned.strip_prefix("~/").unwrap_or("");
            return PathBuf::from(home).join(sub);
        }
    }
    PathBuf::from(cleaned)
}

pub fn get_config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".config/aoi/config.json")
}

pub fn load_config() -> AppConfig {
    let path = get_config_path();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str::<AppConfig>(&content) {
                return cfg;
            }
        }
    }
    let default = AppConfig::default();
    let _ = save_config(&default);
    default
}

pub fn save_config(config: &AppConfig) -> Result<(), String> {
    let path = get_config_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let json_str = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Gagal serialisasi config: {}", e))?;
    fs::write(&path, json_str)
        .map_err(|e| format!("Gagal menulis ke {:?}: {}", path, e))?;
    Ok(())
}
