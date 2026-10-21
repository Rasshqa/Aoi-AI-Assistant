// src-tauri/src/llm_client.rs
// Multi-provider LLM client — agnostik untuk Gemini, OpenAI-compatible (Groq/OpenRouter), dan Ollama.
//
// Cara kerja:
//   1. Baca konfigurasi provider dari env atau config file (~/.config/aoi/config.json)
//   2. Bangun request payload sesuai format masing-masing provider
//   3. Streaming response via reqwest, kumpulkan teks, kembalikan sebagai String
//
// Memory: conversation history disimpan in-memory (Vec<ChatMessage>), max 20 pesan terakhir.

use serde::{Deserialize, Serialize};
use std::env;

// ────────────────────────────────────────────────────────────────────────────
// Tipe Data
// ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,   // "user" | "assistant" | "system"
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub api_key: Option<String>,
    pub base_url: Option<String>, // override untuk OpenRouter / Ollama lokal
    pub model: String,
    pub system_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    Gemini,
    OpenAI,   // juga berlaku untuk Groq & OpenRouter (format sama)
    Ollama,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: LlmProvider::Gemini,
            api_key: env::var("GEMINI_API_KEY").ok(),
            base_url: None,
            model: "gemini-2.0-flash".into(),
            system_prompt: concat!(
                "Kamu adalah Aoi, asisten AI desktop pendamping bergaya anime yang ringan dan cerdas, ",
                "berjalan di CachyOS (Arch Linux) dengan Hyprland Wayland compositor. ",
                "Kamu membantu pengguna dengan tugas sehari-hari, coding, troubleshooting sistem Linux, ",
                "dan administrasi Arch Linux. Jawab dengan ramah, ringkas, dan teknis jika diperlukan. ",
                "Jika kamu perlu menjalankan perintah shell, gunakan format tool_call: BASH:<perintah>"
            ).into(),
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Shared HTTP client (lazy-init via std::sync::OnceLock)
// ────────────────────────────────────────────────────────────────────────────

fn http_client() -> &'static reqwest::Client {
    use std::sync::OnceLock;
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("Gagal membuat HTTP client")
    })
}

// ────────────────────────────────────────────────────────────────────────────
// Entry point utama
// ────────────────────────────────────────────────────────────────────────────

/// Kirim conversation history ke provider yang aktif, kembalikan balasan teks asisten.
pub async fn chat(
    config: &LlmConfig,
    history: &[ChatMessage],
) -> Result<String, String> {
    match config.provider {
        LlmProvider::Gemini => gemini_chat(config, history).await,
        LlmProvider::OpenAI => openai_chat(config, history).await,
        LlmProvider::Ollama => ollama_chat(config, history).await,
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Provider: Google Gemini (generateContent REST API)
// ────────────────────────────────────────────────────────────────────────────

async fn gemini_chat(config: &LlmConfig, history: &[ChatMessage]) -> Result<String, String> {
    let api_key = config
        .api_key
        .as_deref()
        .ok_or("GEMINI_API_KEY tidak ditemukan")?;

    let model = &config.model;
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model, api_key
    );

    // Konversi history ke format Gemini (contents[])
    // System prompt masuk sebagai "user" turn pertama di Gemini
    let contents: Vec<serde_json::Value> = Vec::new();

    // Sisipkan system prompt sebagai instruksi model (Gemini v1beta mendukung systemInstruction)
    let body = serde_json::json!({
        "systemInstruction": {
            "parts": [{ "text": config.system_prompt }]
        },
        "contents": history.iter().map(|m| serde_json::json!({
            "role": if m.role == "assistant" { "model" } else { "user" },
            "parts": [{ "text": m.content }]
        })).collect::<Vec<_>>(),
        "generationConfig": {
            "temperature": 0.7,
            "maxOutputTokens": 1024,
        }
    });
    // Pastikan tidak ada warning unused
    drop(contents);

    let resp = http_client()
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("HTTP error Gemini: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Gemini API error {}: {}", status, text));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

    let text = json["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();

    if text.is_empty() {
        Err("Gemini mengembalikan respons kosong".into())
    } else {
        Ok(text)
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Provider: OpenAI-compatible (OpenAI / Groq / OpenRouter — format identik)
// ────────────────────────────────────────────────────────────────────────────

async fn openai_chat(config: &LlmConfig, history: &[ChatMessage]) -> Result<String, String> {
    let raw_base = config
        .base_url
        .as_deref()
        .unwrap_or("https://api.openai.com/v1")
        .trim()
        .trim_end_matches('/');

    let url = if raw_base.ends_with("/chat/completions") {
        raw_base.to_string()
    } else {
        format!("{}/chat/completions", raw_base)
    };

    // Sisipkan system message di awal
    let mut messages = vec![serde_json::json!({
        "role": "system",
        "content": config.system_prompt
    })];
    for m in history {
        messages.push(serde_json::json!({
            "role": m.role,
            "content": m.content
        }));
    }

    let body = serde_json::json!({
        "model": config.model,
        "messages": messages,
        "temperature": 0.7,
        "max_tokens": 1024,
    });

    let mut req = http_client().post(&url);

    // Kirim bearer auth jika API key tersedia dan tidak kosong
    if let Some(key) = config.api_key.as_deref() {
        if !key.trim().is_empty() {
            req = req.bearer_auth(key.trim());
        }
    }

    let resp = req
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("HTTP error ({}): {}", url, e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("OpenAI API error {}: {}", status, text));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let text = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    if text.is_empty() {
        Err("Provider mengembalikan respons kosong".into())
    } else {
        Ok(text)
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Provider: Ollama (lokal, tanpa API key)
// ────────────────────────────────────────────────────────────────────────────

async fn ollama_chat(config: &LlmConfig, history: &[ChatMessage]) -> Result<String, String> {
    let base_url = config
        .base_url
        .as_deref()
        .unwrap_or("http://localhost:11434");

    let url = format!("{}/api/chat", base_url);

    // Sisipkan system message
    let mut messages = vec![serde_json::json!({
        "role": "system",
        "content": config.system_prompt
    })];
    for m in history {
        messages.push(serde_json::json!({
            "role": m.role,
            "content": m.content
        }));
    }

    let body = serde_json::json!({
        "model": config.model,
        "messages": messages,
        "stream": false,
        "options": {
            "temperature": 0.7,
            "num_predict": 1024,
        }
    });

    let resp = http_client()
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("HTTP error Ollama: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Ollama error {}: {}", status, text));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let text = json["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    if text.is_empty() {
        Err("Ollama mengembalikan respons kosong".into())
    } else {
        Ok(text)
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Short-term Memory Manager (in-RAM, max 20 pesan)
// ────────────────────────────────────────────────────────────────────────────

/// Short-term conversation memory — dipakai penuh di Tahap 4 (persistent memory + context window)
#[allow(dead_code)]
pub struct ConversationMemory {
    messages: Vec<ChatMessage>,
    max_messages: usize,
}

#[allow(dead_code)]
impl ConversationMemory {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            max_messages: 20,
        }
    }

    pub fn push_user(&mut self, content: String) {
        self.messages.push(ChatMessage {
            role: "user".into(),
            content,
        });
        self.trim();
    }

    pub fn push_assistant(&mut self, content: String) {
        self.messages.push(ChatMessage {
            role: "assistant".into(),
            content,
        });
        self.trim();
    }

    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }

    fn trim(&mut self) {
        // Hapus pesan terlama (tapi jaga selalu berpasangan user-assistant)
        while self.messages.len() > self.max_messages {
            self.messages.remove(0);
        }
    }
}

impl Default for ConversationMemory {
    fn default() -> Self {
        Self::new()
    }
}
