// src-tauri/src/hyprland.rs
// Modul konteks Hyprland & Vision — membaca lingkungan desktop aktif pengguna
// untuk dikirim sebagai konteks tambahan ke LLM.
//
// Fungsi:
//   - get_active_window()  → hyprctl activewindow -j
//   - get_clipboard()      → wl-paste
//   - take_screenshot()    → grim -g <geometry> /tmp/aoi_screenshot.png
//   - build_context()      → gabung semua jadi string konteks untuk LLM prompt

use std::process::Stdio;
use tokio::process::Command;

/// Data jendela aktif dari hyprctl
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct ActiveWindow {
    pub class: String,
    pub title: String,
    pub pid: i64,
    pub workspace: Option<WorkspaceInfo>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct WorkspaceInfo {
    pub id: i64,
    pub name: String,
}

/// Konteks lengkap lingkungan desktop yang dikumpulkan sebelum call LLM
#[derive(Debug, Clone, serde::Serialize)]
pub struct HyprlandContext {
    pub active_window_class: String,
    pub active_window_title: String,
    pub clipboard_text: Option<String>,
    pub screenshot_path: Option<String>,
}

// ────────────────────────────────────────────────────────────────────────────
// Fungsi publik
// ────────────────────────────────────────────────────────────────────────────

/// Baca jendela aktif Hyprland via `hyprctl activewindow -j`
pub async fn get_active_window() -> Option<ActiveWindow> {
    let output = Command::new("hyprctl")
        .args(["activewindow", "-j"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    serde_json::from_slice(&output.stdout).ok()
}

/// Baca isi clipboard saat ini via `wl-paste` (wl-clipboard package) — Aman untuk UTF-8/Emoji
pub async fn get_clipboard() -> Option<String> {
    let output = Command::new("wl-paste")
        .arg("--no-newline")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .ok()?;

    if !output.status.success() || output.stdout.is_empty() {
        return None;
    }

    String::from_utf8(output.stdout).ok().map(|s| {
        let char_count = s.chars().count();
        if char_count > 1500 {
            let truncated: String = s.chars().take(1500).collect();
            format!("{}... [dipotong]", truncated)
        } else {
            s
        }
    })
}

/// Ambil screenshot seluruh layar pertama via `grim`, simpan ke /tmp/aoi_screenshot.png
/// Tahap 3: akan dikirim ke LLM vision model (Gemini Pro Vision)
pub async fn take_screenshot() -> Option<String> {
    let path = "/tmp/aoi_screenshot.png";

    let status = Command::new("grim")
        .arg(path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .ok()?;

    if status.success() {
        Some(path.to_string())
    } else {
        None
    }
}

/// Kumpulkan semua konteks dan format menjadi string siap inject ke prompt LLM.
/// `include_screenshot` hanya aktif jika user secara eksplisit meminta vision.
pub async fn build_context(include_screenshot: bool) -> HyprlandContext {
    let window = get_active_window().await;
    let clipboard = get_clipboard().await;
    let screenshot = if include_screenshot {
        take_screenshot().await
    } else {
        None
    };

    HyprlandContext {
        active_window_class: window
            .as_ref()
            .map(|w| w.class.clone())
            .unwrap_or_else(|| "unknown".into()),
        active_window_title: window
            .as_ref()
            .map(|w| w.title.clone())
            .unwrap_or_else(|| "unknown".into()),
        clipboard_text: clipboard,
        screenshot_path: screenshot,
    }
}

/// Format context menjadi blok teks untuk disisipi ke system/user prompt LLM
pub fn format_context_for_prompt(ctx: &HyprlandContext) -> String {
    let mut parts = Vec::new();

    parts.push(format!(
        "[Konteks Desktop]\n- Jendela aktif: {} ({})",
        ctx.active_window_title, ctx.active_window_class
    ));

    if let Some(ref clip) = ctx.clipboard_text {
        if !clip.trim().is_empty() {
            parts.push(format!("- Clipboard:\n```\n{}\n```", clip));
        }
    }

    if ctx.screenshot_path.is_some() {
        parts.push("- Screenshot layar sudah diambil dan siap dianalisis.".into());
    }

    parts.join("\n")
}
