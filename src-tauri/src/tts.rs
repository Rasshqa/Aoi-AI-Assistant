// src-tauri/src/tts.rs
// Tahap 3: Local TTS (Piper / Kokoro / sherpa-onnx) + Real-Time RMS Lip-Sync
//
// Pipeline:
//   text → piper/sherpa-onnx (stdin→wav) → parse PCM → RMS chunks
//         → pw-play (PipeWire) subprocess + emit lip-sync events ke Vue
//
// Fallback playback: pw-play → paplay → aplay
// Fallback TTS: jika engine belum terinstall, simulasi lip-sync adaptif
//               berdasarkan panjang teks agar avatar tetap interaktif dan tidak crash.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use serde_json::json;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::broadcast_state;
use crate::config::AppConfig;
use crate::state::{AssistantState, StatePayload};

// ────────────────────────────────────────────────────────────────────────────
// State global untuk handle proses pw-play (dibutuhkan barge-in kill)
// ────────────────────────────────────────────────────────────────────────────

pub struct TtsPlayerState(pub Arc<Mutex<Option<Child>>>);

// ────────────────────────────────────────────────────────────────────────────
// Text Cleaner: Bersihkan markdown / code block sebelum dikirim ke TTS
// ────────────────────────────────────────────────────────────────────────────

pub fn clean_text_for_tts(raw: &str) -> String {
    // 0. Singkirkan tag thinking jika ada
    let without_thinking = crate::commands::strip_thinking_tags(raw);

    // 1. Singkirkan baris BASH:
    let without_bash = crate::bash_executor::strip_bash_command(&without_thinking);

    // 2. Singkirkan blok kode ```...```
    let mut cleaned = String::new();
    let mut in_code_block = false;
    for line in without_bash.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if !in_code_block {
            cleaned.push_str(line);
            cleaned.push(' ');
        }
    }

    // 3. Singkirkan markdown markup (*, #, `, >, ~, [], ())
    let mut sanitized = String::new();
    for c in cleaned.chars() {
        if matches!(c, '*' | '#' | '`' | '~' | '>' | '[' | ']') {
            continue;
        }
        sanitized.push(c);
    }

    let result = sanitized.trim();
    if result.is_empty() {
        "Perintah berhasil dieksekusi.".to_string()
    } else {
        result.to_string()
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Pencarian model Piper / Kokoro ONNX
// ────────────────────────────────────────────────────────────────────────────

/// Cari file model .onnx dari berbagai lokasi umum
pub fn find_piper_model() -> Option<PathBuf> {
    // 1. Cek path kustom dari ~/.config/aoi/config.json
    let cfg = crate::config::load_config();
    if !cfg.tts_model_path.trim().is_empty() {
        let p = crate::config::expand_user_path(&cfg.tts_model_path);
        if p.exists() {
            return Some(p);
        }
    }

    // 2. Override via environment variable
    if let Ok(model) = std::env::var("AOI_TTS_MODEL") {
        let p = crate::config::expand_user_path(&model);
        if p.exists() {
            return Some(p);
        }
    }

    // 3. Cek spesifik mommy_lessac.onnx di ~/.local/share/piper/
    if let Ok(home) = std::env::var("HOME") {
        let mommy = PathBuf::from(&home).join(".local/share/piper/mommy_lessac.onnx");
        if mommy.exists() {
            return Some(mommy);
        }
    }

    // 4. ~/.local/share/piper/*.onnx & ~/.local/share/kokoro/*.onnx
    if let Ok(home) = std::env::var("HOME") {
        for folder in &[".local/share/piper", ".local/share/kokoro"] {
            let dir = PathBuf::from(&home).join(folder);
            if let Some(m) = first_onnx_in(&dir) {
                return Some(m);
            }
        }
    }

    // 4. /usr/share/piper/voices/*.onnx & /usr/share/piper/*.onnx
    for sys in &["/usr/share/piper/voices", "/usr/share/piper"] {
        if let Some(m) = first_onnx_in(&PathBuf::from(sys)) {
            return Some(m);
        }
    }

    None
}

fn first_onnx_in(dir: &PathBuf) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut models: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("onnx"))
        .collect();
    models.sort();
    models.into_iter().next()
}

// ────────────────────────────────────────────────────────────────────────────
// Lip-Sync: Parse WAV PCM → RMS per chunk → jadwal (delay_ms, is_mouth_open)
// ────────────────────────────────────────────────────────────────────────────

/// Hitung jadwal lip-sync dari WAV PCM data.
/// Returns: Vec<(delay_ms_dari_awal, is_mouth_open)>
/// chunk_ms: durasi tiap frame animasi (default 140ms)
pub fn compute_lipsync_schedule(wav: &[u8], chunk_ms: u64) -> Vec<(u64, bool)> {
    if wav.len() < 44 {
        return vec![];
    }

    let sample_rate =
        u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]) as u64;
    let bits_per_sample =
        u16::from_le_bytes([wav[34], wav[35]]) as u64;
    let bytes_per_sample = (bits_per_sample / 8).max(1);

    let data_offset = wav
        .windows(4)
        .position(|w| w == b"data")
        .map(|pos| pos + 8)
        .unwrap_or(44)
        .min(wav.len());

    let pcm = &wav[data_offset..];
    let samples_per_chunk = ((sample_rate * chunk_ms) / 1000) as usize;
    let bytes_per_chunk = (samples_per_chunk * bytes_per_sample as usize).max(2);

    let threshold = match bits_per_sample {
        16 => 1100.0f64,
        8  => 12.0,
        _  => 1100.0,
    };

    let mut out = Vec::new();
    let mut delay = 0u64;

    for chunk in pcm.chunks(bytes_per_chunk) {
        let rms = match bits_per_sample {
            16 => {
                let sum: f64 = chunk
                    .chunks_exact(2)
                    .map(|b| {
                        let s = i16::from_le_bytes([b[0], b[1]]) as f64;
                        s * s
                    })
                    .sum();
                let n = (chunk.len() / 2).max(1);
                (sum / n as f64).sqrt()
            }
            _ => {
                let sum: f64 = chunk
                    .iter()
                    .map(|&b| {
                        let s = b as f64 - 128.0;
                        s * s
                    })
                    .sum();
                (sum / chunk.len().max(1) as f64).sqrt()
            }
        };

        out.push((delay, rms > threshold));
        delay += chunk_ms;
    }

    out
}

// ────────────────────────────────────────────────────────────────────────────
// TTS Pipeline
// ────────────────────────────────────────────────────────────────────────────

/// Main entry: text → TTS synthesis → PipeWire playback + lip-sync events → Idle.
pub async fn speak(
    text: String,
    app: AppHandle,
    assistant_state: Arc<Mutex<StatePayload>>,
    player_state: Arc<Mutex<Option<Child>>>,
) {
    let wav_path = "/tmp/aoi_tts.wav";
    let cleaned = clean_text_for_tts(&text);
    let config = crate::config::load_config();

    // ── 1. Sintesis audio sesuai engine yang aktif (Edge-TTS, ElevenLabs, OpenAI Speech, atau Piper)
    if let Err(e) = synthesize_speech(&cleaned, &config, wav_path).await {
        eprintln!("TTS synthesis gagal ({}), beralih ke simulated lip-sync.", e);
        simulate_speech(&cleaned, &app, &assistant_state, &player_state).await;
        return;
    }

    // ── 3. Baca WAV + hitung jadwal lip-sync dari amplitudo RMS
    let wav_data = match tokio::fs::read(wav_path).await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Gagal baca WAV: {}", e);
            finish_speech(&app, &assistant_state).await;
            return;
        }
    };

    let schedule = compute_lipsync_schedule(&wav_data, 140);

    // ── 4. Mulai playback audio PipeWire
    let player = match start_playback(wav_path).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Audio playback error: {}", e);
            finish_speech(&app, &assistant_state).await;
            return;
        }
    };

    *player_state.lock().await = Some(player);

    // ── 5. Emit real-time lip-sync events mengikuti RMS audio
    let start = tokio::time::Instant::now();

    for (delay_ms, is_open) in &schedule {
        let target = start + tokio::time::Duration::from_millis(*delay_ms);
        let now = tokio::time::Instant::now();
        if now < target {
            tokio::time::sleep(target - now).await;
        }

        // Jika user melakukan barge-in (Super+V), hentikan loop
        if player_state.lock().await.is_none() {
            break;
        }

        let _ = app.emit("lip-sync-update", json!({ "is_mouth_open": is_open }));
    }

    // ── 6. Tunggu proses player selesai jika belum di-kill
    {
        let mut guard = player_state.lock().await;
        if let Some(mut child) = guard.take() {
            let _ = child.wait().await;
        }
    }

    // ── 7. Tutup mulut & kembalikan state ke Idle (pertahankan pesan yang sudah tampil)
    finish_speech(&app, &assistant_state).await;
}

// ────────────────────────────────────────────────────────────────────────────
// Simulated Speech Fallback (Jika Piper/TTS belum terinstall)
// ────────────────────────────────────────────────────────────────────────────

async fn simulate_speech(
    text: &str,
    app: &AppHandle,
    assistant_state: &Arc<Mutex<StatePayload>>,
    player_state: &Arc<Mutex<Option<Child>>>,
) {
    let word_count = text.split_whitespace().count().max(1);
    // Estimasi membaca: ~160 kata/menit = ~375ms per kata
    // Dibatasi antara 1.5 detik s/d 7 detik
    let total_ms = ((word_count as u64) * 375).clamp(1500, 7000);
    let frames = total_ms / 140;

    let mut mouth_open = false;
    for i in 0..frames {
        // Cek interupsi barge-in
        {
            let s = assistant_state.lock().await;
            if s.state != AssistantState::Speaking {
                break;
            }
        }
        if player_state.lock().await.is_some() {
            // jika ada sinyal kill
        }

        // Variasi buka-tutup mulut yang lebih natural
        if i % 2 == 0 {
            mouth_open = !mouth_open;
        }
        let _ = app.emit("lip-sync-update", json!({ "is_mouth_open": mouth_open }));
        tokio::time::sleep(tokio::time::Duration::from_millis(140)).await;
    }

    finish_speech(app, assistant_state).await;
}

// ────────────────────────────────────────────────────────────────────────────
// Helpers
// ────────────────────────────────────────────────────────────────────────────

/// Selesai berbicara: tutup mulut, ubah state ke Idle, jaga pesan dialog tetap tampil
async fn finish_speech(
    app: &AppHandle,
    assistant_state: &Arc<Mutex<StatePayload>>,
) {
    let _ = app.emit("lip-sync-update", json!({ "is_mouth_open": false }));
    {
        let mut s = assistant_state.lock().await;
        if s.state == AssistantState::Speaking {
            s.state = AssistantState::Idle;
        }
    }
    broadcast_state(app, assistant_state).await;
}

/// Dispatcher utama sintesis suara berdasarkan engine yang dipilih di konfigurasi
pub async fn synthesize_speech(
    text: &str,
    config: &AppConfig,
    wav_output: &str,
) -> Result<(), String> {
    match config.tts_engine.as_str() {
        "edge_tts" => generate_edge_tts(text, config, wav_output).await,
        "elevenlabs" => generate_elevenlabs(text, config, wav_output).await,
        "fish_audio" => generate_fish_audio(text, config, wav_output).await,
        "openai_speech" => generate_openai_speech(text, config, wav_output).await,
        "piper" => {
            let model = find_piper_model()
                .ok_or_else(|| "Model Piper tidak ditemukan di ~/.local/share/piper/".to_string())?;
            generate_wav(text, &model, wav_output).await
        }
        _ => {
            // Default coba Edge-TTS terlebih dahulu, jika gagal coba Piper
            if let Ok(()) = generate_edge_tts(text, config, wav_output).await {
                Ok(())
            } else if let Some(model) = find_piper_model() {
                generate_wav(text, &model, wav_output).await
            } else {
                Err("Tidak ada TTS engine yang dapat digunakan.".into())
            }
        }
    }
}

/// Sintesis vokal via Microsoft Edge-TTS (Natural, Ekspresif, Zero Local Overhead)
async fn generate_edge_tts(
    text: &str,
    config: &AppConfig,
    output_wav: &str,
) -> Result<(), String> {
    let local_bin = std::env::var("HOME")
        .map(|h| PathBuf::from(h).join(".local/bin/edge-tts"))
        .unwrap_or_default();

    let bin = if which_exists("edge-tts").await {
        PathBuf::from("edge-tts")
    } else if local_bin.exists() {
        local_bin
    } else {
        return Err("Binary edge-tts tidak ditemukan di PATH atau ~/.local/bin/edge-tts".into());
    };

    let mp3_path = "/tmp/aoi_edge_temp.mp3";
    let voice = if config.edge_tts_voice.trim().is_empty() {
        "id-ID-GadisNeural"
    } else {
        config.edge_tts_voice.trim()
    };
    let rate = if config.edge_tts_rate.trim().is_empty() {
        "+10%"
    } else {
        config.edge_tts_rate.trim()
    };

    let status = Command::new(&bin)
        .args([
            "--voice",
            voice,
            "--rate",
            rate,
            "--text",
            text,
            "--write-media",
            mp3_path,
            "--no-progress",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|e| format!("Gagal menjalankan edge-tts: {}", e))?;

    if !status.success() {
        return Err("Proses edge-tts keluar dengan error".into());
    }

    convert_mp3_to_wav(mp3_path, output_wav).await
}

/// Sintesis vokal via ElevenLabs Voice Cloning API
async fn generate_elevenlabs(
    text: &str,
    config: &AppConfig,
    output_wav: &str,
) -> Result<(), String> {
    let voice_id = if !config.elevenlabs_voice_id.trim().is_empty() {
        config.elevenlabs_voice_id.trim().to_string()
    } else if !config.active_cloned_voice_id.is_empty() {
        if let Ok(voices) = crate::voice_clone::list_cloned_voices() {
            voices.into_iter()
                .find(|v| v.id == config.active_cloned_voice_id)
                .map(|v| v.remote_voice_id)
                .unwrap_or_default()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    if voice_id.is_empty() || config.elevenlabs_api_key.trim().is_empty() {
        return Err("ElevenLabs API Key atau Voice ID belum dikonfigurasi di Pengaturan.".into());
    }

    let url = format!("https://api.elevenlabs.io/v1/text-to-speech/{}", voice_id);

    let client = reqwest::Client::new();
    let body = json!({
        "text": text,
        "model_id": "eleven_multilingual_v2",
        "voice_settings": {
            "stability": 0.5,
            "similarity_boost": 0.8
        }
    });

    let resp = client
        .post(&url)
        .header("xi-api-key", config.elevenlabs_api_key.trim())
        .header("Content-Type", "application/json")
        .header("Accept", "audio/mpeg")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request ke ElevenLabs gagal: {}", e))?;

    if !resp.status().is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        return Err(format!("ElevenLabs API Error: {}", err_text));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Gagal membaca bytes dari ElevenLabs: {}", e))?;

    let mp3_path = "/tmp/aoi_eleven_temp.mp3";
    tokio::fs::write(mp3_path, &bytes)
        .await
        .map_err(|e| format!("Gagal menyimpan stream MP3 ElevenLabs: {}", e))?;

    convert_mp3_to_wav(mp3_path, output_wav).await
}

/// Sintesis vokal via Fish Audio API (Zero-Shot Anime Voice Cloning)
async fn generate_fish_audio(
    text: &str,
    config: &AppConfig,
    output_wav: &str,
) -> Result<(), String> {
    if config.fish_audio_api_key.trim().is_empty() {
        return Err("Fish Audio API Key belum dikonfigurasi di Pengaturan.".into());
    }

    let reference_id = if !config.active_cloned_voice_id.is_empty() {
        if let Ok(voices) = crate::voice_clone::list_cloned_voices() {
            voices.into_iter()
                .find(|v| v.id == config.active_cloned_voice_id)
                .map(|v| v.remote_voice_id)
                .unwrap_or_default()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let client = reqwest::Client::new();
    let mut body = json!({
        "text": text,
        "format": "mp3"
    });

    if !reference_id.trim().is_empty() {
        body["reference_id"] = json!(reference_id.trim());
    }

    let resp = client
        .post("https://api.fish.audio/v1/tts")
        .header("Authorization", format!("Bearer {}", config.fish_audio_api_key.trim()))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request ke Fish Audio gagal: {}", e))?;

    if !resp.status().is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        return Err(format!("Fish Audio API error: {}", err_text));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Gagal membaca bytes dari Fish Audio: {}", e))?;

    let mp3_path = "/tmp/aoi_fish_temp.mp3";
    tokio::fs::write(mp3_path, &bytes)
        .await
        .map_err(|e| format!("Gagal menyimpan stream MP3 Fish Audio: {}", e))?;

    convert_mp3_to_wav(mp3_path, output_wav).await
}

/// Sintesis vokal via OpenAI-Compatible Speech API (/v1/audio/speech)
async fn generate_openai_speech(
    text: &str,
    config: &AppConfig,
    output_wav: &str,
) -> Result<(), String> {
    let endpoint = if !config.openai_speech_endpoint.trim().is_empty() {
        config.openai_speech_endpoint.trim()
    } else if !config.colab_clone_endpoint.trim().is_empty() {
        config.colab_clone_endpoint.trim()
    } else {
        "https://api.openai.com/v1/audio/speech"
    };

    let client = reqwest::Client::new();
    let voice = if config.openai_speech_voice.trim().is_empty() {
        "nova"
    } else {
        config.openai_speech_voice.trim()
    };

    let body = json!({
        "model": "tts-1",
        "input": text,
        "voice": voice
    });

    let mut req = client
        .post(endpoint)
        .header("Content-Type", "application/json")
        .json(&body);

    if !config.openai_speech_key.trim().is_empty() {
        req = req.header("Authorization", format!("Bearer {}", config.openai_speech_key.trim()));
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("Request OpenAI Speech gagal: {}", e))?;

    if !resp.status().is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        return Err(format!("OpenAI Speech Error: {}", err_text));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Gagal membaca audio bytes: {}", e))?;

    let mp3_path = "/tmp/aoi_openai_temp.mp3";
    tokio::fs::write(mp3_path, &bytes)
        .await
        .map_err(|e| format!("Gagal menyimpan MP3 audio: {}", e))?;

    convert_mp3_to_wav(mp3_path, output_wav).await
}

/// Konversi berkas audio MP3 ke WAV 22050Hz Mono menggunakan ffmpeg agar siap dianalisis RMS Lip-Sync
async fn convert_mp3_to_wav(mp3_path: &str, wav_path: &str) -> Result<(), String> {
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-i",
            mp3_path,
            "-bitexact",
            "-map_metadata",
            "-1",
            "-f",
            "wav",
            "-ar",
            "22050",
            "-ac",
            "1",
            wav_path,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|e| format!("Gagal menjalankan ffmpeg: {}", e))?;

    if !status.success() {
        return Err("Konversi format audio ffmpeg gagal".into());
    }

    Ok(())
}

/// Jalankan piper dengan teks dari stdin, output ke file WAV
async fn generate_wav(text: &str, model: &PathBuf, output: &str) -> Result<(), String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let share_bin = PathBuf::from(&home).join(".local/share/piper/bin/piper");
    let local_bin = PathBuf::from(&home).join(".local/bin/piper");

    let bin = if share_bin.exists() {
        share_bin
    } else if local_bin.exists() {
        local_bin
    } else if which_exists("piper").await {
        PathBuf::from("piper")
    } else if which_exists("piper-tts").await {
        PathBuf::from("piper-tts")
    } else {
        return Err("Binary piper / piper-tts tidak ditemukan di ~/.local/share/piper/bin/piper atau PATH".into());
    };

    let mut child = Command::new(&bin)
        .args([
            "--model",
            model.to_str().unwrap_or(""),
            "--output_file",
            output,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Gagal menjalankan {}: {}", bin.display(), e))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .await
            .map_err(|e| format!("Gagal menulis ke stdin: {}", e))?;
        stdin
            .flush()
            .await
            .map_err(|e| format!("Gagal flush stdin: {}", e))?;
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("Wait process error: {}", e))?;

    if !status.success() {
        return Err(format!("Process exit code: {:?}", status.code()));
    }

    Ok(())
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

/// Mulai playback WAV — coba pw-play, paplay, lalu aplay
async fn start_playback(wav_path: &str) -> Result<Child, String> {
    // PipeWire (Primary — CachyOS default)
    if let Ok(c) = Command::new("pw-play")
        .arg(wav_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        return Ok(c);
    }

    // PulseAudio fallback
    if let Ok(c) = Command::new("paplay")
        .arg(wav_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        return Ok(c);
    }

    // ALSA fallback
    Command::new("aplay")
        .arg(wav_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| {
            "Tidak ada audio player yang dapat memutar audio (install: pipewire)".into()
        })
}

/// Hentikan TTS yang sedang berjalan (dipanggil saat barge-in)
pub async fn stop(player_state: &Arc<Mutex<Option<Child>>>) {
    let mut guard = player_state.lock().await;
    if let Some(mut child) = guard.take() {
        let _ = child.kill().await;
    }
}
