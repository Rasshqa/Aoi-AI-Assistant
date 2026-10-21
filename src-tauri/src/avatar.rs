// src-tauri/src/avatar.rs
// Modul Pengelola Custom Avatar Sprite 2D
// Menyimpan dan membaca frame gambar kustom di ~/.config/aoi/avatar/
// Mendukung format PNG, WebP, SVG, JPG.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use base64::prelude::*;

const FRAME_NAMES: &[&str] = &[
    "idle_open",
    "idle_blink",
    "listening",
    "thinking",
    "speak_open",
    "speak_closed",
    "confirm",
    "error",
];

pub fn get_avatar_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".config/aoi/avatar")
}

/// Simpan satu frame sprite (dari data URL base64) ke disk dan perbarui manifest.json
pub fn save_avatar_sprite(frame_name: &str, data_url: &str) -> Result<String, String> {
    if !FRAME_NAMES.contains(&frame_name) {
        return Err(format!("Nama frame '{}' tidak valid.", frame_name));
    }

    let dir = get_avatar_dir();
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Gagal membuat direktori avatar {:?}: {}", dir, e))?;

    // Pisahkan header data URL jika ada (e.g. "data:image/png;base64,...")
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

    // Update manifest.json agar memuat data_url lengkap
    let mut manifest = load_manifest(&dir);
    manifest.insert(frame_name.to_string(), data_url.to_string());
    save_manifest(&dir, &manifest)?;

    Ok(data_url.to_string())
}

/// Simpan sekumpulan frame kustom sekaligus
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

/// Muat seluruh frame kustom yang tersimpan
pub fn load_avatar_pack(folder_override: Option<String>) -> Result<HashMap<String, String>, String> {
    let dir = if let Some(ref path_str) = folder_override {
        if !path_str.trim().is_empty() {
            PathBuf::from(path_str.trim())
        } else {
            get_avatar_dir()
        }
    } else {
        get_avatar_dir()
    };

    if !dir.exists() {
        return Ok(HashMap::new());
    }

    // 1. Cek apakah ada manifest.json yang menyimpan data URL
    let manifest = load_manifest(&dir);
    if !manifest.is_empty() {
        return Ok(manifest);
    }

    // 2. Scan file gambar manual di folder jika manifest belum ada
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

    Ok(map)
}

/// Reset kustomisasi avatar ke SVG Anime bawaan
pub fn reset_avatar_pack() -> Result<(), String> {
    let dir = get_avatar_dir();
    let manifest_path = dir.join("manifest.json");
    if manifest_path.exists() {
        let _ = fs::remove_file(manifest_path);
    }

    // Hapus semua file frame di folder avatar
    if dir.exists() {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    let _ = fs::remove_file(p);
                }
            }
        }
    }

    Ok(())
}

fn load_manifest(dir: &Path) -> HashMap<String, String> {
    let path = dir.join("manifest.json");
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&content) {
                return map;
            }
        }
    }
    HashMap::new()
}

fn save_manifest(dir: &Path, map: &HashMap<String, String>) -> Result<(), String> {
    let path = dir.join("manifest.json");
    let json_str = serde_json::to_string_pretty(map)
        .map_err(|e| format!("Gagal serialisasi manifest: {}", e))?;
    fs::write(&path, json_str)
        .map_err(|e| format!("Gagal menulis manifest: {}", e))?;
    Ok(())
}
