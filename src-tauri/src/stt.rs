// src-tauri/src/stt.rs
// Modul Telinga Lokal (STT): Perekaman PipeWire + Transkripsi Whisper
//
// Pipeline:
//   1. start_recording() → `pw-record --rate 16000 --channels 1 --format s16 /tmp/aoi_mic.wav`
//   2. stop_recording()  → Hentikan perekaman, pastikan file WAV utuh
//   3. transcribe()      → Eksekusi whisper.cpp lokal (atau Groq/OpenAI Cloud Whisper fallback)

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::config::AppConfig;

pub const MIC_WAV_PATH: &str = "/tmp/aoi_mic.wav";

pub struct AudioRecorderState(pub Arc<Mutex<Option<Child>>>);

// ────────────────────────────────────────────────────────────────────────────
// Perekaman Mikrofon (PipeWire Native)
// ────────────────────────────────────────────────────────────────────────────

/// Mulai merekam mikrofon ke 16kHz mono 16-bit WAV (format standar Whisper)
pub async fn start_recording(recorder: &Arc<Mutex<Option<Child>>>) -> Result<(), String> {
    let mut guard = recorder.lock().await;

    // Jika sedang merekam, hentikan dulu yang lama
    if let Some(mut existing) = guard.take() {
        let _ = existing.kill().await;
    }

    // Bersihkan file lama jika ada
    if Path::new(MIC_WAV_PATH).exists() {
        let _ = tokio::fs::remove_file(MIC_WAV_PATH).await;
    }

    // 1. Coba pw-record (PipeWire natif CachyOS)
    if let Ok(child) = Command::new("pw-record")
        .args([
            "--rate", "16000",
            "--channels", "1",
            "--format", "s16",
            MIC_WAV_PATH,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        *guard = Some(child);
        return Ok(());
    }

    // 2. Fallback: parec
    if let Ok(child) = Command::new("parec")
        .args([
            "--rate=16000",
            "--channels=1",
            "--format=s16le",
            "--file-format=wav",
            MIC_WAV_PATH,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        *guard = Some(child);
        return Ok(());
    }

    // 3. Fallback: arecord (ALSA)
    if let Ok(child) = Command::new("arecord")
        .args([
            "-r", "16000",
            "-c", "1",
            "-f", "S16_LE",
            "-t", "wav",
            MIC_WAV_PATH,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        *guard = Some(child);
        return Ok(());
    }

    Err("Tidak ditemukan alat perekam audio (pw-record / parec / arecord).".into())
}

/// Hentikan perekaman dengan SIGINT agar header WAV ditulis dengan bersih oleh PipeWire
pub async fn stop_recording(recorder: &Arc<Mutex<Option<Child>>>) -> Result<String, String> {
    let mut guard = recorder.lock().await;
    let mut child = guard.take().ok_or("Tidak ada sesi perekaman yang aktif.")?;

    // Kirim SIGINT (interrupt) menggunakan command `kill -s INT <pid>` agar pw-record menutup header WAV
    if let Some(pid) = child.id() {
        let _ = Command::new("kill")
            .args(["-s", "INT", &pid.to_string()])
            .status()
            .await;
    }

    // Tunggu maksimal 600ms agar proses flush & keluar secara alami, jika belum berhenti baru kill()
    match tokio::time::timeout(tokio::time::Duration::from_millis(600), child.wait()).await {
        Ok(_) => {}
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
    }

    // Beri jeda 50ms agar filesystem selesai menulis
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    if !Path::new(MIC_WAV_PATH).exists() {
        return Err("File audio /tmp/aoi_mic.wav tidak berhasil dibuat.".into());
    }

    let meta = tokio::fs::metadata(MIC_WAV_PATH)
        .await
        .map_err(|e| e.to_string())?;

    if meta.len() < 100 {
        return Err("Audio rekaman kosong atau terlalu pendek.".into());
    }

    Ok(MIC_WAV_PATH.to_string())
}

// ────────────────────────────────────────────────────────────────────────────
// Transkripsi Audio (Whisper STT)
// ────────────────────────────────────────────────────────────────────────────

/// Deteksi binary whisper di sistem
pub async fn find_whisper_binary() -> Option<&'static str> {
    let candidates = ["whisper-cli", "whisper-cpp", "whisper-main", "whisper"];
    for bin in candidates {
        if which_exists(bin).await {
            return Some(bin);
        }
    }
    None
}

/// Cari model ggml whisper
pub fn find_whisper_model(custom_path: &str) -> Option<PathBuf> {
    // 1. Path kustom dari config
    if !custom_path.is_empty() {
        let p = crate::config::expand_user_path(custom_path);
        if p.exists() {
            return Some(p);
        }
    }

    // 2. Env variable
    if let Ok(env_p) = std::env::var("AOI_WHISPER_MODEL") {
        let p = crate::config::expand_user_path(&env_p);
        if p.exists() {
            return Some(p);
        }
    }

    // 3. ~/.local/share/whisper/*.bin
    if let Ok(home) = std::env::var("HOME") {
        let dir = PathBuf::from(&home).join(".local/share/whisper");
        if let Some(m) = first_bin_in(&dir) {
            return Some(m);
        }
    }

    // 4. /usr/share/whisper-cpp/*.bin atau /usr/share/whisper/*.bin
    for sys in &["/usr/share/whisper-cpp", "/usr/share/whisper"] {
        if let Some(m) = first_bin_in(&PathBuf::from(sys)) {
            return Some(m);
        }
    }

    None
}

fn first_bin_in(dir: &PathBuf) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut models: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let ext = p.extension().and_then(|e| e.to_str());
            ext == Some("bin") || ext == Some("ggml")
        })
        .collect();
    models.sort();
    models.into_iter().next()
}

async fn which_exists(bin: &str) -> bool {
    Command::new("which")
        .arg(bin)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Transkripsi file audio menjadi teks.
/// Mencoba whisper.cpp lokal terlebih dahulu; jika tidak tersedia,
/// mencoba Cloud Whisper API (Groq / OpenAI) jika API key tersedia.
pub async fn transcribe(wav_path: &str, config: &AppConfig) -> Result<String, String> {
    // ── 1. Coba Whisper.cpp Lokal
    if let Some(bin) = find_whisper_binary().await {
        if let Some(model) = find_whisper_model(&config.whisper_model_path) {
            let output = Command::new(bin)
                .args([
                    "-m", model.to_str().unwrap_or(""),
                    "-f", wav_path,
                    "--no-timestamps",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output()
                .await
                .map_err(|e| format!("Gagal menjalankan whisper: {}", e))?;

            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .map(|l| l.trim())
                    .filter(|l| !l.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");

                if !text.is_empty() {
                    return Ok(text);
                }
            }
        }
    }

    // ── 2. Fallback: Cloud Whisper (Groq / OpenAI)
    if !config.groq_api_key.is_empty() {
        return transcribe_groq(wav_path, &config.groq_api_key).await;
    }

    if !config.openai_api_key.is_empty() {
        return transcribe_openai(wav_path, &config.openai_api_key).await;
    }

    Err(
        "Whisper STT lokal belum terpasang dan API Key Cloud belum dikonfigurasi.\n\
         Silakan pasang whisper-cpp (sudo pacman -S whisper-cpp) dan unduh model ggml ke ~/.local/share/whisper/,\n\
         atau isi GROQ_API_KEY di pengaturan untuk STT awan ultra-cepat."
            .into(),
    )
}

/// Transkripsi via Groq Whisper API (whisper-large-v3, latensi ~150ms)
async fn transcribe_groq(wav_path: &str, api_key: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let audio_bytes = tokio::fs::read(wav_path)
        .await
        .map_err(|e| format!("Gagal membaca file audio: {}", e))?;

    let file_part = reqwest::multipart::Part::bytes(audio_bytes)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;

    let form = reqwest::multipart::Form::new()
        .text("model", "whisper-large-v3")
        .part("file", file_part);

    let resp = client
        .post("https://api.groq.com/openai/v1/audio/transcriptions")
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("HTTP request error ke Groq: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let err_txt = resp.text().await.unwrap_or_default();
        return Err(format!("Groq Whisper error {}: {}", status, err_txt));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let text = json["text"].as_str().unwrap_or("").trim().to_string();

    if text.is_empty() {
        Err("Suara tidak terdengar jelas.".into())
    } else {
        Ok(text)
    }
}

/// Transkripsi via OpenAI Whisper API
async fn transcribe_openai(wav_path: &str, api_key: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let audio_bytes = tokio::fs::read(wav_path)
        .await
        .map_err(|e| format!("Gagal membaca audio: {}", e))?;

    let file_part = reqwest::multipart::Part::bytes(audio_bytes)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;

    let form = reqwest::multipart::Form::new()
        .text("model", "whisper-1")
        .part("file", file_part);

    let resp = client
        .post("https://api.openai.com/v1/audio/transcriptions")
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("HTTP request error ke OpenAI: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let err_txt = resp.text().await.unwrap_or_default();
        return Err(format!("OpenAI Whisper error {}: {}", status, err_txt));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let text = json["text"].as_str().unwrap_or("").trim().to_string();

    if text.is_empty() {
        Err("Suara tidak terdengar jelas.".into())
    } else {
        Ok(text)
    }
}
