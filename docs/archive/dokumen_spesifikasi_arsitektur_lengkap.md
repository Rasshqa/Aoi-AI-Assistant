# 📘 Dokumen Spesifikasi Proyek: AI Hypr-Assistant (Aoi)
**Platform Target:** Arch Linux (CachyOS) + Wayland (Hyprland) + Ryoku Shell  
**Arsitektur Inti:** Tauri v2 (Rust) + Vue 3 (TypeScript) + Tailwind CSS  

---

## 1. Product Requirements Document (PRD)

### 1.1 Visi & Tujuan Produk
Membangun asisten AI pendamping desktop (*desktop companion*) bergaya anime 2D yang **super ringan**, berjalan secara *native* di lingkungan Linux Wayland (CachyOS + Hyprland), mampu diajak mengobrol via suara dengan latensi rendah, memiliki kesadaran konteks terhadap aktivitas layar pengguna, serta mampu mengeksekusi perintah sistem Linux secara aman.

### 1.2 Target Pengguna & Kasus Penggunaan
* **Pengguna:** Pengguna Linux tingkat lanjut / *developer* yang menggunakan *tiling window manager* Hyprland dan membutuhkan asisten interaktif tanpa mengorbankan performa CPU/RAM/VRAM saat bekerja berat.
* **Use Cases Utama:**
  1. Mengobrol santai atau berdiskusi teknis sambil *coding* menggunakan *push-to-talk / smart toggle*.
  2. Meminta AI memeriksa *error* pada jendela aktif (`hyprctl`), teks di *clipboard* (`wl-paste`), atau tangkapan layar (`grim`).
  3. Menyuruh AI mengatur sistem (volume, *brightness*, *media player*, pindah *workspace*) atau menjalankan perintah `bash` kompleks dengan sistem konfirmasi keamanan.

### 1.3 Fitur Utama (Key Features)
| ID | Nama Fitur | Deskripsi | Prioritas |
| :--- | :--- | :--- | :--- |
| **F-01** | **Sticky Wayland Overlay** | Jendela transparan tanpa *border* yang selalu tampil di atas (*always-on-top*) dan tetap muncul saat berpindah *workspace* (`pin`), bisa digeser, di-*minimize*, atau masuk ke *Ghost Mode*. | P0 (Wajib) |
| **F-02** | **2D Frame Avatar Engine** | Sistem animasi ringan berbasis pergantian *frame* gambar (PNG/WebP/SVG) untuk *state* `idle`, `listening`, `thinking`, `speaking`, `confirm`, dan `error`, lengkap dengan kedipan mata otomatis & *lip-sync* 2-frame. | P0 (Wajib) |
| **F-03** | **Smart Keybind & Barge-In** | Satu *shortcut* global Hyprland (`Super + V`) untuk menyalakan mic, mematikan mic, atau langsung menyela (*interrupt*) pembicaraan AI di tempat berisik. | P0 (Wajib) |
| **F-04** | **Two-Tier Bash Executor** | Eksekusi perintah Linux otomatis untuk perintah aman (*Green Tier*) dan dialog konfirmasi cepat `[Alt+Y / Alt+N]` untuk perintah berisiko (*Red Tier*). | P0 (Wajib) |
| **F-05** | **Multi-Provider LLM** | Dukungan agnostik untuk Gemini API, OpenAI/Groq/OpenRouter, dan LLM lokal via Ollama. | P0 (Wajib) |
| **F-06** | **Local Lightweight Voice** | STT lokal menggunakan `whisper.cpp` dan TTS lokal ringan menggunakan `Kokoro-82M` / `Piper` (via `sherpa-onnx`). | P1 |
| **F-07** | **Hyprland & Ryoku IPC** | Membaca jendela aktif Hyprland dan menyiarkan status asisten ke bar Ryoku melalui `/tmp/ai_state.json`. | P1 |

### 1.4 Non-Functional Requirements (Target Performa)
* **Konsumsi RAM Idle:** < 80 MB (menggunakan WebKitGTK bawaan sistem via Tauri).
* **Konsumsi CPU Idle:** < 1% (hanya *timer* kedipan mata setiap 4 detik, tanpa *render loop* 60fps WebGL).
* **Responsivitas IPC Keybind:** < 15 milidetik dari penekanan tombol Hyprland hingga perubahan *state* visual.

---

## 2. Technical Requirements Document (TRD)

### 2.1 Spesifikasi Tech Stack
* **Sistem Operasi & WM:** CachyOS (Arch Linux), Hyprland (Wayland), PipeWire Audio.
* **Backend Core:** Rust (Edition 2021), Tauri v2, Tokio (Async Runtime), Serde & Serde JSON.
* **Frontend UI:** Vue 3 (Composition API + `<script setup lang="ts">`), Vite, Tailwind CSS.
* **Komunikasi Antar-Proses (IPC):**
  * Hyprland ➔ App: Unix Domain Socket (`/tmp/ai-assistant.sock`) via `socat`.
  * App ➔ Ryoku Bar: File JSON State (`/tmp/ai_state.json`).
  * Rust ➔ Vue: Tauri Event System (`app.emit` & `invoke`).
* **Utilitas Sistem Eksternal:** `socat` (pengirim sinyal socket), `grim` (screenshot Wayland), `wl-clipboard` (`wl-paste`), `hyprctl` (kontrol WM), `pactl` & `playerctl`.

### 2.2 Kebutuhan Keamanan & Sandboxing
* Perintah yang dihasilkan LLM lewat *Function Calling* harus melewati filter **Regex Classifier** di Rust sebelum menyentuh `std::process::Command`.
* Setiap perintah yang mengandung kata kunci modifikasi/destruktif (`sudo`, `rm`, `mv`, `dd`, `mkfs`, `pacman`, `paru`, `yay`, `systemctl`, `kill`, `pkill`, `chmod`, `chown`, `>`) **wajib** mengubah state asisten menjadi `Confirm` dan menahan eksekusi hingga pengguna menekan `Alt + Y`.

---

## 3. Application Flow (Alur Kerja Aplikasi)

### 3.1 Alur Interaksi Suara & Smart Keybind (`Super + V`)
```text
[User Tekan Super+V] ──► socat kirim "toggle_mic" ke /tmp/ai-assistant.sock
                                │
        ┌───────────────────────┴───────────────────────┐
        ▼                                               ▼
[Jika State == IDLE / ERROR]                  [Jika State == THINKING / SPEAKING]
1. Ubah state ➔ LISTENING                    1. BARGE-IN: Hentikan audio TTS & batalkan stream LLM
2. Mulai rekam mic (PipeWire)                 2. Ubah state ➔ LISTENING
3. Tunggu user tekan Super+V lagi             3. Mulai rekam instruksi suara baru
   atau VAD mendeteksi selesai bicara
        │
        ▼
[Transisi ke THINKING]
1. Whisper.cpp mengubah buffer audio ➔ Teks
2. Ambil konteks jendela aktif (`hyprctl activewindow -j`)
3. Kirim Teks + Konteks ke Provider LLM Aktif
        │
        ├──► [Jika LLM Memanggil Tool Bash Red-Tier]
        │     1. Ubah state ➔ CONFIRM
        │     2. Tampilkan kotak dialog perintah di atas Avatar
        │     3. Tunggu sinyal `confirm_yes` (Alt+Y) atau `confirm_no` (Alt+N)
        │
        └──► [Jika LLM Membalas Teks / Tool Green-Tier]
              1. Jalankan Green-Tier tool (jika ada)
              2. Kirim teks balasan ke engine TTS lokal
              3. Ubah state ➔ SPEAKING (Mulut karakter bergerak mengikuti amplitudo)
              4. Selesai bicara ➔ Kembali ke state IDLE
```

---

## 4. Design Brief (UI/UX & Sistem Karakter 2D)

### 4.1 Filosofi Visual Overlay
* **Dimensi Jendela Utama:** `340px x 420px` (transparan penuh, tanpa bayangan jendela kompositor).
* **Gaya Visual:** *Modern Dark RPG Glassmorphism* (`bg-slate-900/90`, `backdrop-blur-md`, aksen *border* tipis `border-slate-700/80`).
* **Ergonomi Layar:**
  * Dapat digeser dari bagian tubuh avatar maupun *header* balon dialog.
  * **Ghost Mode (👻):** Menurunkan *opacity* menjadi `35%` saat pengguna fokus mengetik, dan kembali `100%` saat kursor diarahkan ke avatar.
  * **Minimized Pill Mode (`_` / `Super + H`):** Menyusutkan avatar menjadi kapsul indikator kecil berisi lampu status dan teks *state* aktif.

### 4.2 Sistem Warna Indikator Status (State Ring & Aura)
| State | Warna Indikator | Kode Tailwind / Hex | Perilaku Visual Karakter |
| :--- | :--- | :--- | :--- |
| **Idle** | Abu-abu Kebiruan | `bg-slate-400` (`#94a3b8`) | Diam tenang, mata berkedip selama `160ms` setiap `4000ms`. |
| **Listening** | Hijau Zamrud | `bg-emerald-500` (`#10b981`) | Sedikit membesar (`scale-105`), aura hijau, ikon mikrofon menyala. |
| **Thinking** | Kuning Amber | `bg-amber-500` (`#f59e0b`) | Mata melirik ke atas, muncul gelembung pikiran di samping kepala. |
| **Speaking** | Biru Langit | `bg-sky-500` (`#0ea5e9`) | Berganti cepat antara *frame* mulut tertutup & terbuka setiap `140ms` (atau via amplitudo). |
| **Confirm** | Ungu | `bg-purple-500` (`#a855f7`) | Ekspresi serius + muncul kotak kode terminal dengan tombol `[Alt+Y]` & `[Alt+N]`. |
| **Error** | Merah Rose | `bg-rose-500` (`#f43f5e`) | Mata berbentuk `X X`, aura merah. |

### 4.3 Standar Aset Avatar Kustom (`manifest.json`)
Pengguna dapat mengganti karakter dengan menaruh file PNG/WebP transparan berukuran `512x512` px:
* `idle_open.webp` & `idle_blink.webp`
* `listening.webp`
* `thinking.webp`
* `speak_closed.webp` & `speak_open.webp`
* `confirm.webp`
* `error.webp`

---

## 5. Backend Scheme (Arsitektur Rust & Kontrak Data)

### 5.1 Struktur State & Payload (`src-tauri/src/main.rs`)
```rust
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatePayload {
    pub state: AssistantState,
    pub message: String,
    pub pending_command: Option<String>,
    pub minimized: bool,
}
```

### 5.2 Daftar Protokol Perintah Unix Socket (`/tmp/ai-assistant.sock`)
| String Perintah Masuk | Sumber Pemicu | Aksi di Backend Rust |
| :--- | :--- | :--- |
| `toggle_mic` | Keybind `Super + V` | Menjalankan fungsi `handle_smart_toggle()` (Start Mic / Stop Mic / Barge-In). |
| `toggle_hide` | Keybind `Super + H` | Membalik nilai boolean `minimized` dan menyiarkan state. |
| `confirm_yes` | Keybind `Alt + Y` | Mengeksekusi `pending_command` yang tertahan di state `Confirm`. |
| `confirm_no` | Keybind `Alt + N` | Membatalkan `pending_command` dan mengembalikan state ke `Idle`. |
| `state:<nama_state>` | CLI Testing | Memaksa perubahan state secara manual untuk keperluan *debugging*. |

### 5.3 Skema Output Status Bar Ryoku (`/tmp/ai_state.json`)
Setiap kali ada perubahan state, backend Rust otomatis menulis file JSON satu baris ini agar dapat dibaca oleh modul *bar* Ryoku:
```json
{"state":"listening","message":"Mendengarkan...","pending_command":null,"minimized":false}
```

---

## 6. Implementation Plan (Peta Jalan Pengembangan 4 Tahap)

* ✅ **Tahap 1: Fondasi Window Transparan, IPC Socket Hyprland & 2D Avatar UI (Selesai)**
  * Setup Tauri v2 + Vue 3 + Tailwind CSS.
  * Implementasi Unix Domain Socket server di `/tmp/ai-assistant.sock`.
  * Pembuatan komponen Avatar 2D State Machine + RPG Dialog Box + Konfigurasi `hyprland.conf`.
* ⏳ **Tahap 2: Integrasi Otak Multi-LLM, Konteks Hyprland & Bash Executor (Langkah Berikutnya)**
  * Pembuatan modul `llm_client.rs` (mendukung Gemini, OpenAI-compatible/Groq, dan Ollama).
  * Pembuatan modul `hyprland.rs` (membaca `hyprctl activewindow -j`, `wl-paste`, dan `grim`).
  * Implementasi `bash_executor.rs` dengan klasifikasi *Green Tier* vs *Red Tier* dan koneksi nyata ke tombol `Alt+Y` / `Alt+N`.
* ⏳ **Tahap 3: Suara Lokal (TTS) & Real-Time Audio Lip-Sync**
  * Integrasi `sherpa-onnx` / Piper / Kokoro TTS lokal di Rust.
  * Pemutaran audio via PipeWire disertai perhitungan RMS amplitudo suara untuk menggerakkan mulut `isMouthOpen` secara presisi.
* ⏳ **Tahap 4: Telinga Lokal (Whisper STT), Settings Window & System Packaging**
  * Perekaman mikrofon via `cpal` + transkripsi lokal `whisper-rs`.
  * Pembuatan jendela kedua untuk pengaturan API Key & pemilihan *folder* karakter avatar.
  * Kompilasi menjadi binary rilis (`cargo tauri build`) dan pembuatan `.desktop` launcher.