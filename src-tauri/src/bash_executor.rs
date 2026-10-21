// src-tauri/src/bash_executor.rs
// Two-Tier Command Execution System
//
// ┌─────────────────────────────────────────────────────────────────────┐
// │  GREEN TIER  → Aman, langsung dieksekusi tanpa konfirmasi           │
// │  RED TIER    → Destruktif/berbahaya, wajib konfirmasi Alt+Y/Alt+N  │
// └─────────────────────────────────────────────────────────────────────┘
//
// Regex classifier memfilter kata kunci berbahaya SEBELUM menyentuh shell.
// Semua eksekusi menggunakan `sh -c` dengan timeout 30 detik.

use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

// ────────────────────────────────────────────────────────────────────────────
// Classifier: Tentukan tier dari sebuah perintah
// ────────────────────────────────────────────────────────────────────────────

/// Kembalikan `true` jika perintah termasuk Red Tier (butuh konfirmasi)
pub fn is_red_tier(command: &str) -> bool {
    // Kata kunci yang wajib memicu konfirmasi pengguna
    let red_patterns = [
        r"sudo\b",
        r"\brm\b",
        r"\bmv\b",
        r"\bdd\b",
        r"\bmkfs\b",
        r"\bformat\b",
        r"\bfdisk\b",
        r"\bparted\b",
        r"\bshred\b",
        r"\bpacman\b",
        r"\bparu\b",
        r"\byay\b",
        r"\bsystemctl\b",
        r"\bjournalctl\b",
        r"\bkill\b",
        r"\bpkill\b",
        r"\bkillall\b",
        r"\bchmod\b",
        r"\bchown\b",
        r"\bchattr\b",
        r"\biptables\b",
        r"\bufw\b",
        r"\bcurl\b.*\|\s*bash",   // curl | bash — sangat berbahaya
        r"\bwget\b.*\|\s*sh",     // wget | sh
        r">\s*/dev/",             // redirect ke /dev/
        r">\s*/etc/",             // overwrite config sistem
        r">\s*/boot/",
        r">\s*/usr/",
        r"\btruncate\b",
        r"--no-preserve-root",
    ];

    for pattern in &red_patterns {
        if let Ok(re) = regex_lite(pattern, command) {
            if re {
                return true;
            }
        }
    }
    false
}

/// Cek apakah `command` mengandung pattern (manual regex-lite tanpa dep regex)
fn regex_lite(pattern: &str, text: &str) -> Result<bool, ()> {
    // Implementasi sederhana: strip \b dan cek kata
    // Untuk production, tambahkan crate `regex` ke Cargo.toml
    let core = pattern
        .trim_start_matches(r"\b")
        .trim_end_matches(r"\b")
        .replace(r"\b", " ")
        .replace(r"\s*", " ")
        .replace(r".*", "");

    // Handle pattern yang mengandung | (pipe patterns)
    if pattern.contains(r"\|") {
        // curl | bash, wget | sh — cek substring kombinasi
        if text.contains("curl") && text.contains("bash") { return Ok(true); }
        if text.contains("wget") && text.contains("sh") { return Ok(true); }
        return Ok(false);
    }

    // Handle redirect patterns
    if pattern.starts_with('>') {
        let target = core.trim_start_matches("> /").trim_start_matches(">/");
        return Ok(text.contains(&format!("/{}", target)) || text.contains(&format!("> /{}", target)));
    }

    // Cek apakah kata kunci muncul sebagai kata penuh (dikelilingi spasi/awal/akhir)
    let word = core.trim();
    let found = text.split_whitespace().any(|w| {
        let w = w.trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_');
        w == word
    }) || text.contains(&format!(" {} ", word))
        || text.starts_with(&format!("{} ", word))
        || text == word;

    Ok(found)
}

// ────────────────────────────────────────────────────────────────────────────
// Parser: Ekstrak BASH:<command> dari output LLM
// ────────────────────────────────────────────────────────────────────────────

/// Cek apakah LLM response mengandung tool call BASH:<command>
/// Format: "...teks...\nBASH:ls -la /home\n...teks..."
pub fn extract_bash_command(llm_response: &str) -> Option<String> {
    for line in llm_response.lines() {
        let line = line.trim();
        if line.starts_with("BASH:") {
            let cmd = line.trim_start_matches("BASH:").trim().to_string();
            if !cmd.is_empty() {
                return Some(cmd);
            }
        }
    }
    None
}

/// Pisahkan teks biasa dari bagian BASH: di output LLM
pub fn strip_bash_command(llm_response: &str) -> String {
    llm_response
        .lines()
        .filter(|l| !l.trim().starts_with("BASH:"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

// ────────────────────────────────────────────────────────────────────────────
// Eksekutor
// ────────────────────────────────────────────────────────────────────────────

/// Eksekusi perintah Green Tier langsung (tanpa konfirmasi).
/// Timeout: 30 detik. Output di-capture dan dikembalikan.
pub async fn execute_green(command: &str) -> Result<String, String> {
    let result = timeout(
        Duration::from_secs(30),
        Command::new("sh")
            .arg("-c")
            .arg(command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await;

    match result {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            if output.status.success() {
                Ok(if stdout.is_empty() { "✅ Selesai (tidak ada output).".into() } else { stdout })
            } else {
                Err(if stderr.is_empty() {
                    format!("Perintah gagal dengan exit code: {:?}", output.status.code())
                } else {
                    stderr
                })
            }
        }
        Ok(Err(e)) => Err(format!("Gagal menjalankan proses: {}", e)),
        Err(_) => Err("Timeout: perintah berjalan lebih dari 30 detik.".into()),
    }
}

/// Hasil klasifikasi perintah LLM — dipakai oleh `commands.rs`
#[derive(Debug, serde::Serialize)]
pub struct CommandClassification {
    pub command: String,
    pub tier: CommandTier,
}

#[derive(Debug, serde::Serialize)]
pub enum CommandTier {
    Green,
    Red,
}

/// Klasifikasi perintah dan kembalikan tier-nya
pub fn classify(command: &str) -> CommandClassification {
    CommandClassification {
        command: command.to_string(),
        tier: if is_red_tier(command) {
            CommandTier::Red
        } else {
            CommandTier::Green
        },
    }
}
