# Graph Report - ai-hypr-assistant  (2026-10-22)

## Corpus Check
- 33 files · ~41,390 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 6 file(s) not represented in the graph (top: (none) 2, .desktop 1, .icns 1)

## Summary
- 538 nodes · 1085 edges · 35 communities (23 shown, 12 thin omitted)
- Extraction: 98% EXTRACTED · 2% INFERRED · 0% AMBIGUOUS · INFERRED: 19 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `1e3fd808`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- App.vue
- package.json
- commands.rs
- tts.rs
- ConversationMemory
- bash_executor.rs
- stt.rs
- 📘 Dokumen Spesifikasi Proyek: AI Hypr-Assistant (Aoi)
- backend_rust_unix_socket_listener.rs
- tauri.conf.json
- compilerOptions
- config.rs
- avatar.rs
- state.rs
- socket.rs
- saveAvatarToLocalStorage
- compilerOptions
- ============================================================================
- default.json
- vue
- handleGlobalKeydown
- Tauri + Vue + TypeScript
- konfigurasi_dependensi_rust.md
- install.sh
- handleAvatarMouseDown
- handleAvatarWheel
- handleChatKeydown
- ai-hypr-assistant
- voice_clone.rs
- fetchClonedVoices
- resetAudioSample

## God Nodes (most connected - your core abstractions)
1. `broadcast_state()` - 18 edges
2. `compilerOptions` - 16 edges
3. `speak()` - 14 edges
4. `run_ai_pipeline()` - 13 edges
5. `ConversationMemory` - 13 edges
6. `TtsPlayerState` - 13 edges
7. `list_cloned_voices()` - 12 edges
8. `import_avatar_preset_zip()` - 11 edges
9. `LlmConfig` - 11 edges
10. `synthesize_speech()` - 11 edges

## Surprising Connections (you probably didn't know these)
- `handle_command()` --calls--> `broadcast_state()`  [INFERRED]
  src-tauri/src/socket.rs → src-tauri/src/lib.rs
- `handle_smart_toggle()` --calls--> `broadcast_state()`  [INFERRED]
  src-tauri/src/socket.rs → src-tauri/src/lib.rs
- `finish_speech()` --calls--> `broadcast_state()`  [INFERRED]
  src-tauri/src/tts.rs → src-tauri/src/lib.rs
- `run()` --calls--> `AppState`  [INFERRED]
  src-tauri/src/lib.rs → src-tauri/src/state.rs
- `run()` --calls--> `AudioRecorderState`  [INFERRED]
  src-tauri/src/lib.rs → src-tauri/src/stt.rs

## Import Cycles
- None detected.

## Communities (35 total, 12 thin omitted)

### Community 0 - "App.vue"
Cohesion: 0.02
Nodes (61): activeCustomFrame, activeSettingsTab, AppConfig, AssistantState, audioSampleBase64, audioSampleFileName, avatarFrameList, avatarMouseDownPos (+53 more)

### Community 1 - "package.json"
Cohesion: 0.05
Nodes (40): dependencies, @tauri-apps/api, @tauri-apps/plugin-opener, vue, devDependencies, autoprefixer, postcss, tailwindcss (+32 more)

### Community 2 - "commands.rs"
Cohesion: 0.11
Nodes (60): AppState, AssistantState, AvatarPresetMeta, ClonedVoiceMeta, ask_llm(), build_llm_config(), build_system_prompt(), cancel_pending_command() (+52 more)

### Community 3 - "tts.rs"
Cohesion: 0.19
Nodes (31): asyncwriteext, json, clean_text_for_tts(), compute_lipsync_schedule(), convert_mp3_to_wav(), expand_user_path(), find_piper_model(), finish_speech() (+23 more)

### Community 4 - "ConversationMemory"
Cohesion: 0.20
Nodes (17): Client, env, chat(), ChatMessage, ConversationMemory, gemini_chat(), http_client(), LlmConfig (+9 more)

### Community 5 - "bash_executor.rs"
Cohesion: 0.15
Nodes (24): command, classify(), CommandClassification, CommandTier, execute_green(), extract_bash_command(), is_red_tier(), regex_lite() (+16 more)

### Community 6 - "stt.rs"
Cohesion: 0.23
Nodes (21): process, AudioRecorderState, expand_user_path(), find_whisper_binary(), find_whisper_model(), first_bin_in(), MIC_WAV_PATH, AppConfig (+13 more)

### Community 7 - "📘 Dokumen Spesifikasi Proyek: AI Hypr-Assistant (Aoi)"
Cohesion: 0.10
Nodes (20): 1.1 Visi & Tujuan Produk, 1.2 Target Pengguna & Kasus Penggunaan, 1.3 Fitur Utama (Key Features), 1.4 Non-Functional Requirements (Target Performa), 1. Product Requirements Document (PRD), 2.1 Spesifikasi Tech Stack, 2.2 Kebutuhan Keamanan & Sandboxing, 2. Technical Requirements Document (TRD) (+12 more)

### Community 8 - "backend_rust_unix_socket_listener.rs"
Cohesion: 0.23
Nodes (21): AppState, AssistantState, broadcast_state(), get_current_state(), handle_smart_toggle(), main(), AppHandle, Arc (+13 more)

### Community 9 - "tauri.conf.json"
Cohesion: 0.11
Nodes (18): app, security, windows, withGlobalTauri, build, beforeBuildCommand, beforeDevCommand, devUrl (+10 more)

### Community 10 - "compilerOptions"
Cohesion: 0.11
Nodes (18): compilerOptions, allowImportingTsExtensions, isolatedModules, jsx, lib, module, moduleResolution, noEmit (+10 more)

### Community 11 - "config.rs"
Cohesion: 0.17
Nodes (18): fs, AppConfig, default_active_avatar_preset(), default_assistant_name(), default_edge_tts_rate(), default_edge_tts_voice(), default_openai_speech_voice(), default_personality_preset() (+10 more)

### Community 12 - "avatar.rs"
Cohesion: 0.21
Nodes (32): Path, AvatarPresetMeta, create_avatar_preset(), delete_avatar_preset(), ensure_presets_initialized(), export_avatar_preset_zip(), flatten_dir_if_nested(), FRAME_NAMES (+24 more)

### Community 13 - "state.rs"
Cohesion: 0.33
Nodes (8): serde, AppState, AssistantState, Arc, Mutex, Option, String, StatePayload

### Community 14 - "socket.rs"
Cohesion: 0.44
Nodes (11): broadcast_state, handle_command(), handle_smart_toggle(), AppHandle, Arc, Child, Mutex, Option (+3 more)

### Community 15 - "saveAvatarToLocalStorage"
Cohesion: 0.19
Nodes (13): createNewPreset(), deletePreset(), fetchAvatarPresets(), handleBatchUpload(), handleSpriteUpload(), handleWindowFocus(), handleZipUpload(), loadAvatarFromLocalStorage() (+5 more)

### Community 16 - "compilerOptions"
Cohesion: 0.25
Nodes (7): compilerOptions, allowSyntheticDefaultImports, composite, module, moduleResolution, skipLibCheck, include

### Community 17 - "============================================================================"
Cohesion: 0.29
Nodes (7): ============================================================================, 1. Window Rules agar Overlay Transparan, Floating, dan Sticky di semua Workspace, 2. Smart Keybind (Super + V) untuk Mulai Bicara / Stop / Menyela Pembicaraan (Barge-In), ============================================================================, 3. Keybind (Super + H) untuk Minimize / Sembunyikan Avatar, 4. Keybind Konfirmasi Cepat Perintah Bash [Alt + Y / Alt + N], Tambahkan baris berikut ke dalam ~/.config/hypr/hyprland.conf milikmu

### Community 18 - "default.json"
Cohesion: 0.33
Nodes (5): description, identifier, permissions, $schema, windows

### Community 19 - "vue"
Cohesion: 0.40
Nodes (3): vue, src_style, *.vue

### Community 20 - "handleGlobalKeydown"
Cohesion: 0.50
Nodes (4): cancelBash(), confirmBash(), handleGlobalKeydown(), openSettings()

### Community 32 - "voice_clone.rs"
Cohesion: 0.36
Nodes (17): multipart, prelude, clone_voice_from_file(), ClonedVoiceMeta, delete_cloned_voice(), ensure_cloned_voices_initialized(), get_cloned_voice_sample_base64(), get_cloned_voices_dir() (+9 more)

### Community 33 - "fetchClonedVoices"
Cohesion: 0.60
Nodes (5): deleteClonedVoice(), fetchClonedVoices(), saveManualVoice(), selectClonedVoice(), submitVoiceClone()

## Knowledge Gaps
- **162 isolated node(s):** `SOCKET_PATH`, `RYOKU_STATUS_PATH`, `install.sh script`, `name`, `private` (+157 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 223 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **12 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `vue` connect `vue` to `App.vue`, `package.json`?**
  _High betweenness centrality (0.019) - this node is a cross-community bridge._
- **Why does `@tauri-apps/api` connect `package.json` to `App.vue`?**
  _High betweenness centrality (0.016) - this node is a cross-community bridge._
- **Why does `ConversationMemory` connect `ConversationMemory` to `commands.rs`?**
  _High betweenness centrality (0.016) - this node is a cross-community bridge._
- **Are the 12 inferred relationships involving `broadcast_state()` (e.g. with `cancel_pending_command()` and `clear_chat_memory()`) actually correct?**
  _`broadcast_state()` has 12 INFERRED edges - model-reasoned connections that need verification._
- **What connects `SOCKET_PATH`, `RYOKU_STATUS_PATH`, `install.sh script` to the rest of the system?**
  _162 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `App.vue` be split into smaller, more focused modules?**
  _Cohesion score 0.024096385542168676 - nodes in this community are weakly interconnected._
- **Should `package.json` be split into smaller, more focused modules?**
  _Cohesion score 0.048726467331118496 - nodes in this community are weakly interconnected._