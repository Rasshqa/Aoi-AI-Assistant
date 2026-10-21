// src-tauri/src/state.rs
// Definisi tipe state global asisten, dipakai di seluruh modul.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Enam state utama avatar Aoi
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

/// Payload yang dikirim ke Vue frontend & ditulis ke /tmp/ai_state.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatePayload {
    pub state: AssistantState,
    pub message: String,
    pub pending_command: Option<String>,
    pub minimized: bool,
}

/// Wrapper agar bisa dikelola Tauri sebagai managed state
pub struct AppState(pub Arc<Mutex<StatePayload>>);
