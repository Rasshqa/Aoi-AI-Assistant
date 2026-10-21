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
}

impl Default for AppConfig {
    fn default() -> Self {
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
            tts_model_path: std::env::var("AOI_TTS_MODEL").unwrap_or_default(),
            whisper_model_path: std::env::var("AOI_WHISPER_MODEL").unwrap_or_default(),
            avatar_folder_path: String::new(),
        }
    }
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
