// src-tauri/src/voice_clone.rs
// Modul Studio Kloning Suara Kustom (Voice Cloning Studio)
// Mendukung pembuatan profil suara kloning dari file audio (.mp3, .wav, .m4a, .ogg)
// atau rekaman mikrofon langsung, terintegrasi dengan ElevenLabs VoiceLab, Fish Audio,
// dan server GPU mandiri / Google Colab GPT-SoVITS.

use std::fs;
use std::path::PathBuf;
use base64::prelude::*;
use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClonedVoiceMeta {
    pub id: String,
    pub name: String,
    pub engine: String, // "elevenlabs" | "fish_audio" | "custom_server"
    pub remote_voice_id: String,
    pub sample_filename: Option<String>,
    pub created_at: u64,
    pub is_active: bool,
}

pub fn get_cloned_voices_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".config/aoi/cloned_voices")
}

pub fn ensure_cloned_voices_initialized() {
    let dir = get_cloned_voices_dir();
    let _ = fs::create_dir_all(&dir);
}

/// Muat daftar semua profil suara hasil kloning
pub fn list_cloned_voices() -> Result<Vec<ClonedVoiceMeta>, String> {
    ensure_cloned_voices_initialized();
    let dir = get_cloned_voices_dir();
    let manifest_path = dir.join("voices_manifest.json");

    let cfg = crate::config::load_config();
    let active_id = cfg.active_cloned_voice_id;

    if manifest_path.exists() {
        if let Ok(content) = fs::read_to_string(&manifest_path) {
            if let Ok(mut list) = serde_json::from_str::<Vec<ClonedVoiceMeta>>(&content) {
                for item in &mut list {
                    item.is_active = !active_id.is_empty() && item.id == active_id;
                }
                return Ok(list);
            }
        }
    }

    Ok(Vec::new())
}

/// Simpan daftar profil suara kloning ke manifest
fn save_manifest(list: &[ClonedVoiceMeta]) -> Result<(), String> {
    let dir = get_cloned_voices_dir();
    let _ = fs::create_dir_all(&dir);
    let manifest_path = dir.join("voices_manifest.json");
    let json_str = serde_json::to_string_pretty(list)
        .map_err(|e| format!("Gagal serialisasi voices_manifest: {}", e))?;
    fs::write(&manifest_path, json_str)
        .map_err(|e| format!("Gagal menulis voices_manifest.json: {}", e))?;
    Ok(())
}

/// Aktifkan salah satu profil suara kloning
pub fn set_active_cloned_voice(voice_id: &str) -> Result<(), String> {
    let mut list = list_cloned_voices()?;
    let mut found = false;
    let mut selected_engine = String::new();
    let mut selected_remote_id = String::new();

    for item in &mut list {
        if item.id == voice_id {
            item.is_active = true;
            found = true;
            selected_engine = item.engine.clone();
            selected_remote_id = item.remote_voice_id.clone();
        } else {
            item.is_active = false;
        }
    }

    if !found && !voice_id.is_empty() {
        return Err(format!("Profil suara '{}' tidak ditemukan.", voice_id));
    }

    save_manifest(&list)?;

    // Perbarui konfigurasi utama aplikasi
    let mut cfg = crate::config::load_config();
    cfg.active_cloned_voice_id = voice_id.to_string();

    if found {
        if selected_engine == "elevenlabs" {
            cfg.tts_engine = "elevenlabs".to_string();
            cfg.elevenlabs_voice_id = selected_remote_id;
        } else if selected_engine == "fish_audio" {
            cfg.tts_engine = "fish_audio".to_string();
        } else if selected_engine == "custom_server" {
            cfg.tts_engine = "openai_speech".to_string();
        }
    }

    let _ = crate::config::save_config(&cfg);
    Ok(())
}

/// Hapus profil suara kloning
pub fn delete_cloned_voice(voice_id: &str) -> Result<(), String> {
    let mut list = list_cloned_voices()?;
    let initial_len = list.len();
    let mut sample_to_remove: Option<String> = None;

    list.retain(|item| {
        if item.id == voice_id {
            sample_to_remove = item.sample_filename.clone();
            false
        } else {
            true
        }
    });

    if list.len() == initial_len {
        return Err("Profil suara tidak ditemukan.".to_string());
    }

    // Hapus file audio sampel jika ada
    if let Some(filename) = sample_to_remove {
        let sample_path = get_cloned_voices_dir().join(&filename);
        if sample_path.exists() {
            let _ = fs::remove_file(sample_path);
        }
    }

    save_manifest(&list)?;

    // Reset config jika yang dihapus sedang aktif
    let mut cfg = crate::config::load_config();
    if cfg.active_cloned_voice_id == voice_id {
        cfg.active_cloned_voice_id = String::new();
        cfg.tts_engine = "edge_tts".to_string();
        let _ = crate::config::save_config(&cfg);
    }

    Ok(())
}

/// Tambah suara kloning secara manual (misal jika sudah punya Voice ID sebelumnya)
pub fn save_cloned_voice_manual(
    name: &str,
    engine: &str,
    remote_voice_id: &str,
) -> Result<ClonedVoiceMeta, String> {
    if name.trim().is_empty() || remote_voice_id.trim().is_empty() {
        return Err("Nama suara dan Voice ID tidak boleh kosong.".into());
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let id = format!("clone_{}", timestamp);

    let meta = ClonedVoiceMeta {
        id: id.clone(),
        name: name.trim().to_string(),
        engine: engine.to_string(),
        remote_voice_id: remote_voice_id.trim().to_string(),
        sample_filename: None,
        created_at: timestamp,
        is_active: true,
    };

    let mut list = list_cloned_voices()?;
    list.retain(|x| x.id != id);
    list.push(meta.clone());
    save_manifest(&list)?;

    let _ = set_active_cloned_voice(&id);

    Ok(meta)
}

/// Kloning suara baru dengan mengunggah berkas audio sampel (WAV / MP3)
pub async fn clone_voice_from_file(
    name: &str,
    engine: &str,
    file_base64: &str,
    filename: &str,
    api_key_override: Option<&str>,
) -> Result<ClonedVoiceMeta, String> {
    ensure_cloned_voices_initialized();

    if name.trim().is_empty() {
        return Err("Nama suara tidak boleh kosong.".into());
    }

    // Decode base64 audio
    let clean_b64 = if let Some(idx) = file_base64.find(";base64,") {
        &file_base64[idx + 8..]
    } else {
        file_base64
    };

    let audio_bytes = BASE64_STANDARD
        .decode(clean_b64.trim())
        .map_err(|e| format!("Gagal decode file audio base64: {}", e))?;

    if audio_bytes.is_empty() {
        return Err("File audio kosong.".into());
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Simpan file audio sampel ke disk
    let ext = std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mp3")
        .to_lowercase();

    let sample_name = format!("sample_{}.{}", timestamp, ext);
    let sample_path = get_cloned_voices_dir().join(&sample_name);
    fs::write(&sample_path, &audio_bytes)
        .map_err(|e| format!("Gagal menyimpan audio sampel: {}", e))?;

    let cfg = crate::config::load_config();
    let client = reqwest::Client::new();

    let remote_voice_id = match engine {
        "elevenlabs" => {
            let key = api_key_override
                .filter(|k| !k.trim().is_empty())
                .unwrap_or(&cfg.elevenlabs_api_key);

            if key.trim().is_empty() {
                return Err("ElevenLabs API Key belum diisi di Pengaturan.".into());
            }

            let mime_type = match ext.as_str() {
                "wav" => "audio/wav",
                "ogg" => "audio/ogg",
                "m4a" => "audio/mp4",
                _ => "audio/mpeg",
            };

            let part = Part::bytes(audio_bytes)
                .file_name(filename.to_string())
                .mime_str(mime_type)
                .map_err(|e| format!("MIME type error: {}", e))?;

            let form = Form::new()
                .text("name", name.trim().to_string())
                .text("description", "Cloned via Aoi Assistant VoiceLab".to_string())
                .part("files", part);

            let resp = client
                .post("https://api.elevenlabs.io/v1/voices/add")
                .header("xi-api-key", key.trim())
                .multipart(form)
                .send()
                .await
                .map_err(|e| format!("Request ke ElevenLabs gagal: {}", e))?;

            if !resp.status().is_success() {
                let err_text = resp.text().await.unwrap_or_default();
                return Err(format!("ElevenLabs VoiceLab error: {}", err_text));
            }

            let json: serde_json::Value = resp
                .json()
                .await
                .map_err(|e| format!("Gagal parse respon JSON ElevenLabs: {}", e))?;

            json["voice_id"]
                .as_str()
                .ok_or_else(|| "ElevenLabs tidak mengembalikan voice_id".to_string())?
                .to_string()
        }
        "fish_audio" => {
            let key = api_key_override
                .filter(|k| !k.trim().is_empty())
                .unwrap_or(&cfg.fish_audio_api_key);

            if key.trim().is_empty() {
                return Err("Fish Audio API Key belum diisi di Pengaturan.".into());
            }

            let mime_type = match ext.as_str() {
                "wav" => "audio/wav",
                _ => "audio/mpeg",
            };

            let part = Part::bytes(audio_bytes)
                .file_name(filename.to_string())
                .mime_str(mime_type)
                .map_err(|e| format!("MIME error: {}", e))?;

            let form = Form::new()
                .text("title", name.trim().to_string())
                .part("voices", part);

            let resp = client
                .post("https://api.fish.audio/model")
                .header("Authorization", format!("Bearer {}", key.trim()))
                .multipart(form)
                .send()
                .await
                .map_err(|e| format!("Request ke Fish Audio gagal: {}", e))?;

            if !resp.status().is_success() {
                let err_text = resp.text().await.unwrap_or_default();
                return Err(format!("Fish Audio API error: {}", err_text));
            }

            let json: serde_json::Value = resp
                .json()
                .await
                .map_err(|e| format!("Gagal parse respon JSON Fish Audio: {}", e))?;

            json["_id"]
                .as_str()
                .or_else(|| json["id"].as_str())
                .ok_or_else(|| "Fish Audio tidak mengembalikan model ID".to_string())?
                .to_string()
        }
        "custom_server" => {
            // Gunakan path audio sampel lokal sebagai referensi zero-shot Colab / server GPU
            sample_path.to_string_lossy().to_string()
        }
        _ => {
            return Err(format!("Engine '{}' tidak didukung untuk kloning suara.", engine));
        }
    };

    let id = format!("clone_{}", timestamp);
    let meta = ClonedVoiceMeta {
        id: id.clone(),
        name: name.trim().to_string(),
        engine: engine.to_string(),
        remote_voice_id,
        sample_filename: Some(sample_name),
        created_at: timestamp,
        is_active: true,
    };

    let mut list = list_cloned_voices()?;
    list.push(meta.clone());
    save_manifest(&list)?;

    let _ = set_active_cloned_voice(&id);

    Ok(meta)
}

/// Ambil file sampel audio dalam format Base64 untuk preview di frontend
pub fn get_cloned_voice_sample_base64(voice_id: &str) -> Result<String, String> {
    let list = list_cloned_voices()?;
    let item = list
        .into_iter()
        .find(|v| v.id == voice_id)
        .ok_or_else(|| "Profil suara tidak ditemukan.".to_string())?;

    if let Some(sample_name) = item.sample_filename {
        let path = get_cloned_voices_dir().join(&sample_name);
        if path.exists() {
            let bytes = fs::read(&path)
                .map_err(|e| format!("Gagal membaca file sampel: {}", e))?;
            return Ok(BASE64_STANDARD.encode(&bytes));
        }
    }
    Err("File sampel audio tidak ditemukan di penyimpanan lokal.".to_string())
}
