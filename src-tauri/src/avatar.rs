// src-tauri/src/avatar.rs
// Modul Pengelola Custom Avatar Sprite 2D & Sistem Multi-Preset
// Menyimpan, membaca, mengekstrak ZIP, dan mengelola preset karakter di:
//   ~/.config/aoi/avatar_presets/<preset_id>/
// Mendukung format PNG, WebP, SVG, JPG.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use base64::prelude::*;
use serde::{Deserialize, Serialize};

pub const FRAME_NAMES: &[&str] = &[
    "idle_open",
    "idle_blink",
    "listening",
    "thinking",
    "speak_open",
    "speak_closed",
    "confirm",
    "error",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvatarPresetMeta {
    pub id: String,
    pub name: String,
    pub thumbnail: String, // Data URL base64 dari frame idle_open atau frame pertama
    pub frame_count: usize,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PresetManifest {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub frames: HashMap<String, String>, // nama_frame -> nama_file atau data_url
}

pub fn get_legacy_avatar_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".config/aoi/avatar")
}

pub fn get_presets_root_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".config/aoi/avatar_presets")
}

pub fn get_preset_dir(preset_id: &str) -> PathBuf {
    get_presets_root_dir().join(preset_id)
}

/// Inisialisasi struktur folder presets dan migrasi data legacy avatar jika ada
pub fn ensure_presets_initialized() {
    let root = get_presets_root_dir();
    let _ = fs::create_dir_all(&root);

    // Cek apakah ada avatar lama di ~/.config/aoi/avatar/ yang belum dimigrasi
    let legacy_dir = get_legacy_avatar_dir();
    let migrated_preset_dir = root.join("custom_migrated");

    if legacy_dir.exists() && !migrated_preset_dir.exists() {
        // Cek apakah legacy_dir memiliki gambar
        let mut has_images = false;
        if let Ok(entries) = fs::read_dir(&legacy_dir) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                    if ["png", "webp", "jpg", "jpeg", "svg"].contains(&ext.to_lowercase().as_str()) {
                        has_images = true;
                        break;
                    }
                }
            }
        }

        if has_images {
            let _ = fs::create_dir_all(&migrated_preset_dir);
            // Salin semua file dari legacy_dir ke migrated_preset_dir
            if let Ok(entries) = fs::read_dir(&legacy_dir) {
                for entry in entries.flatten() {
                    let dest = migrated_preset_dir.join(entry.file_name());
                    let _ = fs::copy(entry.path(), dest);
                }
            }

            let manifest = PresetManifest {
                id: "custom_migrated".to_string(),
                name: "Avatar Kustom Saya".to_string(),
                author: "User".to_string(),
                frames: HashMap::new(),
            };
            let _ = save_preset_manifest(&migrated_preset_dir, &manifest);

            // Set preset aktif ke custom_migrated jika config masih default
            let mut cfg = crate::config::load_config();
            if cfg.active_avatar_preset == "default" {
                cfg.active_avatar_preset = "custom_migrated".to_string();
                let _ = crate::config::save_config(&cfg);
            }
        }
    }
}

/// Dapatkan daftar semua preset avatar yang tersedia
pub fn list_avatar_presets() -> Result<Vec<AvatarPresetMeta>, String> {
    ensure_presets_initialized();
    let root = get_presets_root_dir();
    let cfg = crate::config::load_config();
    let active_id = cfg.active_avatar_preset;

    let mut list = Vec::new();

    // 1. Preset Bawaan: Anime SVG
    list.push(AvatarPresetMeta {
        id: "default".to_string(),
        name: "Aoi Classic (Anime SVG)".to_string(),
        thumbnail: "".to_string(), // Frontend menampilkan thumbnail SVG built-in
        frame_count: 8,
        is_active: active_id == "default",
    });

    // 2. Scan folder presets di ~/.config/aoi/avatar_presets/*
    if let Ok(entries) = fs::read_dir(&root) {
        let mut subdirs: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir() && p.file_name().and_then(|s| s.to_str()) != Some("default"))
            .collect();
        subdirs.sort();

        for dir in subdirs {
            let folder_name = dir.file_name().unwrap_or_default().to_string_lossy().to_string();
            let manifest = load_preset_manifest(&dir);
            let display_name = if !manifest.name.trim().is_empty() {
                manifest.name
            } else {
                folder_name.clone()
            };

            // Hitung frame yang tersedia dan ambil thumbnail
            let pack = load_preset_frames_from_dir(&dir);
            let count = pack.len();
            let thumb = pack.get("idle_open")
                .or_else(|| pack.get("idle_blink"))
                .or_else(|| pack.values().next())
                .cloned()
                .unwrap_or_default();

            list.push(AvatarPresetMeta {
                id: folder_name.clone(),
                name: display_name,
                thumbnail: thumb,
                frame_count: count,
                is_active: active_id == folder_name,
            });
        }
    }

    Ok(list)
}

/// Aktifkan salah satu preset avatar dan muat frame-nya
pub fn switch_avatar_preset(preset_id: &str) -> Result<HashMap<String, String>, String> {
    let mut cfg = crate::config::load_config();
    cfg.active_avatar_preset = preset_id.to_string();
    let _ = crate::config::save_config(&cfg);

    if preset_id == "default" {
        return Ok(HashMap::new());
    }

    let dir = get_preset_dir(preset_id);
    if !dir.exists() {
        return Err(format!("Preset '{}' tidak ditemukan.", preset_id));
    }

    Ok(load_preset_frames_from_dir(&dir))
}

/// Buat preset baru yang kosong dengan nama tertentu
pub fn create_avatar_preset(name: &str) -> Result<AvatarPresetMeta, String> {
    ensure_presets_initialized();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let clean_id = format!("preset_{}", timestamp);
    let dir = get_preset_dir(&clean_id);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Gagal membuat folder preset: {}", e))?;

    let manifest = PresetManifest {
        id: clean_id.clone(),
        name: if name.trim().is_empty() { "Preset Baru".to_string() } else { name.trim().to_string() },
        author: "User".to_string(),
        frames: HashMap::new(),
    };
    save_preset_manifest(&dir, &manifest)?;

    Ok(AvatarPresetMeta {
        id: clean_id,
        name: manifest.name,
        thumbnail: "".to_string(),
        frame_count: 0,
        is_active: false,
    })
}

/// Hapus preset avatar
pub fn delete_avatar_preset(preset_id: &str) -> Result<(), String> {
    if preset_id == "default" {
        return Err("Preset bawaan tidak dapat dihapus.".to_string());
    }

    let dir = get_preset_dir(preset_id);
    if dir.exists() {
        fs::remove_dir_all(&dir)
            .map_err(|e| format!("Gagal menghapus folder preset: {}", e))?;
    }

    // Jika yang dihapus sedang aktif, kembalikan ke default
    let mut cfg = crate::config::load_config();
    if cfg.active_avatar_preset == preset_id {
        cfg.active_avatar_preset = "default".to_string();
        let _ = crate::config::save_config(&cfg);
    }

    Ok(())
}

/// Impor berkas ZIP yang di-upload dari UI (base64 encoded) menjadi preset avatar baru
pub fn import_avatar_preset_zip(name: Option<String>, zip_base64: &str) -> Result<AvatarPresetMeta, String> {
    ensure_presets_initialized();

    // Decode zip bytes
    let clean_b64 = if let Some(idx) = zip_base64.find(";base64,") {
        &zip_base64[idx + 8..]
    } else {
        zip_base64
    };

    let bytes = BASE64_STANDARD
        .decode(clean_b64.trim())
        .map_err(|e| format!("Gagal decode base64 file zip: {}", e))?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let temp_zip_path = format!("/tmp/aoi_upload_{}.zip", timestamp);
    fs::write(&temp_zip_path, bytes)
        .map_err(|e| format!("Gagal menulis berkas ZIP sementara: {}", e))?;

    let preset_id = format!("preset_{}", timestamp);
    let target_dir = get_preset_dir(&preset_id);
    let _ = fs::create_dir_all(&target_dir);

    // Ekstrak via unzip CLI
    let status = Command::new("unzip")
        .args(["-o", "-q", &temp_zip_path, "-d", target_dir.to_str().unwrap_or("")])
        .status()
        .map_err(|e| format!("Gagal menjalankan unzip: {}", e))?;

    let _ = fs::remove_file(&temp_zip_path);

    if !status.success() {
        let _ = fs::remove_dir_all(&target_dir);
        return Err("Gagal mengekstrak berkas ZIP. Pastikan format arsip valid.".to_string());
    }

    // Tangani kemungkinan file bersarang di dalam subfolder (misal saat zip folder)
    flatten_dir_if_nested(&target_dir);

    // Normalisasi nama frame gambar (misal jika ada file 'idle.png' -> 'idle_open.png')
    normalize_frame_files(&target_dir);

    let preset_name = name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| format!("Preset {}", timestamp));

    let manifest = PresetManifest {
        id: preset_id.clone(),
        name: preset_name.clone(),
        author: "User Import".to_string(),
        frames: HashMap::new(),
    };
    let _ = save_preset_manifest(&target_dir, &manifest);

    // Muat frames untuk metadata
    let pack = load_preset_frames_from_dir(&target_dir);
    let count = pack.len();
    let thumb = pack.get("idle_open")
        .or_else(|| pack.get("idle_blink"))
        .or_else(|| pack.values().next())
        .cloned()
        .unwrap_or_default();

    // Otomatis aktifkan preset baru ini
    let mut cfg = crate::config::load_config();
    cfg.active_avatar_preset = preset_id.clone();
    let _ = crate::config::save_config(&cfg);

    Ok(AvatarPresetMeta {
        id: preset_id,
        name: preset_name,
        thumbnail: thumb,
        frame_count: count,
        is_active: true,
    })
}

/// Ekspor preset avatar ke format ZIP dan kembalikan base64 string
pub fn export_avatar_preset_zip(preset_id: &str) -> Result<String, String> {
    let dir = get_preset_dir(preset_id);
    if !dir.exists() {
        return Err(format!("Preset '{}' tidak ditemukan.", preset_id));
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let temp_zip_path = format!("/tmp/aoi_export_{}_{}.zip", preset_id, timestamp);

    // Kumpulkan semua file gambar frame yang ada di direktori preset
    let mut files_to_zip: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ["png", "webp", "jpg", "jpeg", "svg"].contains(&ext.to_lowercase().as_str()) {
                        files_to_zip.push(path);
                    }
                }
            }
        }
    }

    if files_to_zip.is_empty() {
        return Err("Tidak ada file gambar sprite di dalam preset ini untuk diekspor.".into());
    }

    let status = Command::new("zip")
        .args(["-j", "-q", &temp_zip_path])
        .args(&files_to_zip)
        .status()
        .map_err(|e| format!("Gagal menjalankan perintah zip: {}", e))?;

    if !status.success() {
        return Err("Proses pembuatan arsip ZIP gagal.".into());
    }

    let zip_bytes = fs::read(&temp_zip_path)
        .map_err(|e| format!("Gagal membaca hasil zip: {}", e))?;
    let _ = fs::remove_file(&temp_zip_path);

    Ok(BASE64_STANDARD.encode(&zip_bytes))
}

// ─────────────────────────────────────────────────────────────────────────────
// Backward Compatibility Functions
// ─────────────────────────────────────────────────────────────────────────────

pub fn get_avatar_dir() -> PathBuf {
    let cfg = crate::config::load_config();
    if cfg.active_avatar_preset != "default" {
        get_preset_dir(&cfg.active_avatar_preset)
    } else {
        get_legacy_avatar_dir()
    }
}

pub fn save_avatar_sprite(frame_name: &str, data_url: &str) -> Result<String, String> {
    if !FRAME_NAMES.contains(&frame_name) {
        return Err(format!("Nama frame '{}' tidak valid.", frame_name));
    }

    let dir = get_avatar_dir();
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Gagal membuat direktori avatar {:?}: {}", dir, e))?;

    let (mime, raw_b64) = if let Some(idx) = data_url.find(";base64,") {
        let prefix = &data_url[..idx];
        let mime = prefix.strip_prefix("data:").unwrap_or("image/png");
        let b64 = &data_url[idx + 8..];
        (mime, b64)
    } else {
        ("image/png", data_url)
    };

    let ext = match mime {
        "image/webp" => "webp",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/svg+xml" => "svg",
        _ => "png",
    };

    let bytes = BASE64_STANDARD
        .decode(raw_b64.trim())
        .map_err(|e| format!("Gagal decode base64: {}", e))?;

    let file_path = dir.join(format!("{}.{}", frame_name, ext));
    fs::write(&file_path, bytes)
        .map_err(|e| format!("Gagal menulis file avatar {:?}: {}", file_path, e))?;

    let mut manifest = load_preset_manifest(&dir);
    if manifest.id.is_empty() {
        manifest.id = dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
    }
    if manifest.name.is_empty() {
        manifest.name = if manifest.id == "custom_migrated" {
            "Avatar Kustom Saya".to_string()
        } else {
            manifest.id.clone()
        };
    }
    manifest.frames.insert(frame_name.to_string(), data_url.to_string());
    save_preset_manifest(&dir, &manifest)?;

    Ok(data_url.to_string())
}

pub fn save_avatar_pack(pack: HashMap<String, String>) -> Result<(), String> {
    let dir = get_avatar_dir();
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Gagal membuat direktori avatar: {}", e))?;

    for (k, v) in &pack {
        if !v.is_empty() {
            let _ = save_avatar_sprite(k, v);
        }
    }
    Ok(())
}

pub fn load_avatar_pack(folder_override: Option<String>) -> Result<HashMap<String, String>, String> {
    let dir = if let Some(ref path_str) = folder_override {
        if !path_str.trim().is_empty() {
            PathBuf::from(path_str.trim())
        } else {
            get_avatar_dir()
        }
    } else {
        let cfg = crate::config::load_config();
        if cfg.active_avatar_preset == "default" {
            return Ok(HashMap::new());
        }
        get_preset_dir(&cfg.active_avatar_preset)
    };

    if !dir.exists() {
        return Ok(HashMap::new());
    }

    Ok(load_preset_frames_from_dir(&dir))
}

pub fn reset_avatar_pack() -> Result<(), String> {
    let mut cfg = crate::config::load_config();
    cfg.active_avatar_preset = "default".to_string();
    let _ = crate::config::save_config(&cfg);
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn load_preset_frames_from_dir(dir: &Path) -> HashMap<String, String> {
    // 1. Cek manifest.json
    let manifest_map = load_manifest_map(dir);
    if !manifest_map.is_empty() {
        return manifest_map;
    }

    // 2. Scan disk files
    let mut map = HashMap::new();
    let extensions = ["png", "webp", "jpg", "jpeg", "svg"];

    for &frame in FRAME_NAMES {
        for &ext in &extensions {
            let candidate = dir.join(format!("{}.{}", frame, ext));
            if candidate.exists() {
                if let Ok(bytes) = fs::read(&candidate) {
                    let mime = match ext {
                        "webp" => "image/webp",
                        "jpg" | "jpeg" => "image/jpeg",
                        "svg" => "image/svg+xml",
                        _ => "image/png",
                    };
                    let b64 = BASE64_STANDARD.encode(&bytes);
                    let data_url = format!("data:{};base64,{}", mime, b64);
                    map.insert(frame.to_string(), data_url);
                    break;
                }
            }
        }
    }

    map
}

fn load_manifest_map(dir: &Path) -> HashMap<String, String> {
    let path = dir.join("manifest.json");
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            // Bisa format Map<String, String> atau Struct PresetManifest
            if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&content) {
                return map;
            }
            if let Ok(manifest) = serde_json::from_str::<PresetManifest>(&content) {
                return manifest.frames;
            }
        }
    }
    HashMap::new()
}

fn load_preset_manifest(dir: &Path) -> PresetManifest {
    let path = dir.join("manifest.json");
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(m) = serde_json::from_str::<PresetManifest>(&content) {
                if !m.id.is_empty() || !m.name.is_empty() || !m.frames.is_empty() {
                    return m;
                }
            }
            if let Ok(frames) = serde_json::from_str::<HashMap<String, String>>(&content) {
                let folder_id = dir.file_name().unwrap_or_default().to_string_lossy().to_string();
                return PresetManifest {
                    id: folder_id.clone(),
                    name: folder_id,
                    author: "User".to_string(),
                    frames,
                };
            }
        }
    }
    PresetManifest::default()
}

fn save_preset_manifest(dir: &Path, manifest: &PresetManifest) -> Result<(), String> {
    let path = dir.join("manifest.json");
    let json_str = serde_json::to_string_pretty(manifest)
        .map_err(|e| format!("Gagal serialisasi manifest: {}", e))?;
    fs::write(&path, json_str)
        .map_err(|e| format!("Gagal menulis manifest: {}", e))?;
    Ok(())
}

fn flatten_dir_if_nested(target_dir: &Path) {
    if let Ok(entries) = fs::read_dir(target_dir) {
        let items: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        // Jika hanya ada 1 direktori di dalam target_dir (e.g. user zip folder "my_waifu/")
        if items.len() == 1 && items[0].is_dir() {
            let nested = &items[0];
            if let Ok(sub_entries) = fs::read_dir(nested) {
                for sub in sub_entries.flatten() {
                    let dest = target_dir.join(sub.file_name());
                    let _ = fs::rename(sub.path(), dest);
                }
            }
            let _ = fs::remove_dir(nested);
        }
    }
}

fn normalize_frame_files(target_dir: &Path) {
    let aliases: &[(&[&str], &str)] = &[
        (&["idle", "stand", "normal"], "idle_open"),
        (&["blink", "kedip", "close_eyes"], "idle_blink"),
        (&["listen", "hear", "dengar", "mic"], "listening"),
        (&["think", "pikir", "wait"], "thinking"),
        (&["speak", "talking", "bicara", "say"], "speak_open"),
        (&["mouth_closed", "speak_close", "bicara_tutup"], "speak_closed"),
        (&["ok", "yes", "success", "alert", "warn"], "confirm"),
        (&["err", "fail", "angry", "marah"], "error"),
    ];

    if let Ok(entries) = fs::read_dir(target_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if !p.is_file() {
                continue;
            }
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
            let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("png");

            // Jika belum punya nama frame standar, cek apakah cocok dengan alias
            if !FRAME_NAMES.contains(&stem.as_str()) {
                for (keywords, target_name) in aliases {
                    if keywords.iter().any(|&k| stem == k || stem.starts_with(k)) {
                        let new_dest = target_dir.join(format!("{}.{}", target_name, ext));
                        if !new_dest.exists() {
                            let _ = fs::rename(&p, &new_dest);
                        }
                        break;
                    }
                }
            }
        }
    }
}
