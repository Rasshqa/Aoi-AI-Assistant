<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

// ─────────────────────────────────────────────────────────────────────────────
// Tipe Data
// ─────────────────────────────────────────────────────────────────────────────
type AssistantState = 'idle' | 'listening' | 'thinking' | 'speaking' | 'confirm' | 'error';

interface StatePayload {
  state: AssistantState;
  message: string;
  pending_command: string | null;
  minimized: boolean;
}

interface AppConfig {
  llm_provider: string;
  gemini_api_key: string;
  groq_api_key: string;
  openai_api_key: string;
  openrouter_api_key: string;
  ollama_base_url: string;
  custom_api_key: string;
  custom_api_endpoint: string;
  model_name: string;
  tts_model_path: string;
  whisper_model_path: string;
  avatar_folder_path: string;
}

interface LipSyncPayload {
  is_mouth_open: boolean;
}

// ─────────────────────────────────────────────────────────────────────────────
// State reaktif
// ─────────────────────────────────────────────────────────────────────────────
const currentState = ref<AssistantState>('idle');
const message = ref<string>('Halo! Tekan Super+V untuk mulai ngobrol.');
const pendingCommand = ref<string | null>(null);
const isMinimized = ref<boolean>(false);
const ghostMode = ref<boolean>(false);
const showDevToolbar = ref<boolean>(false);
const showSettings = ref<boolean>(false);

// Input chat untuk mengetik langsung di UI
const chatInput = ref<string>('');
const isSendingChat = ref<boolean>(false);

// Animasi 2D frame
const isBlinking = ref<boolean>(false);
const isMouthOpen = ref<boolean>(false);

// Pengaturan Konfigurasi
const configData = ref<AppConfig>({
  llm_provider: 'gemini',
  gemini_api_key: '',
  groq_api_key: '',
  openai_api_key: '',
  openrouter_api_key: '',
  ollama_base_url: 'http://localhost:11434',
  custom_api_key: '',
  custom_api_endpoint: '',
  model_name: 'gemini-2.0-flash',
  tts_model_path: '',
  whisper_model_path: '',
  avatar_folder_path: '',
});
const isSavingConfig = ref<boolean>(false);

// Custom avatar pack (kosong = pakai SVG built-in)
const customAvatarPack = ref<Record<string, string>>({
  idle_open: '', idle_blink: '', listening: '',
  thinking: '', speak_closed: '', speak_open: '',
  confirm: '', error: ''
});

let blinkInterval: number | null = null;
let lipSyncInterval: number | null = null;
let unlistenEvent: (() => void) | null = null;
let unlistenLipSync: (() => void) | null = null;
let lastLipSyncEventTime = 0;

// ─────────────────────────────────────────────────────────────────────────────
// Computed
// ─────────────────────────────────────────────────────────────────────────────
const stateBadgeColor = computed(() => {
  switch (currentState.value) {
    case 'listening': return 'bg-emerald-500 shadow-emerald-500/50';
    case 'thinking':  return 'bg-amber-500  shadow-amber-500/50';
    case 'speaking':  return 'bg-sky-500    shadow-sky-500/50';
    case 'confirm':   return 'bg-purple-500 shadow-purple-500/50';
    case 'error':     return 'bg-rose-500   shadow-rose-500/50';
    default:          return 'bg-slate-400';
  }
});

const stateLabel = computed(() => {
  const labels: Record<AssistantState, string> = {
    idle: 'Idle', listening: 'Listening ●', thinking: 'Thinking...',
    speaking: 'Speaking', confirm: 'Confirm?', error: 'Error'
  };
  return labels[currentState.value];
});

const activeCustomFrame = computed(() => {
  const pack = customAvatarPack.value;
  // Jika seluruh pack kosong, return empty string agar SVG bawaan yang aktif
  const hasAnyCustom = Object.values(pack).some(v => Boolean(v && v.trim()));
  if (!hasAnyCustom) return '';

  if (currentState.value === 'idle') {
    if (isBlinking.value && pack.idle_blink) return pack.idle_blink;
    return pack.idle_open || pack.idle_blink || '';
  }
  if (currentState.value === 'speaking') {
    if (isMouthOpen.value && pack.speak_open) return pack.speak_open;
    return pack.speak_closed || pack.speak_open || pack.idle_open || '';
  }
  return pack[currentState.value] || pack.idle_open || '';
});

// ─────────────────────────────────────────────────────────────────────────────
// Animasi
// ─────────────────────────────────────────────────────────────────────────────
const startBlinkTimer = () => {
  blinkInterval = window.setInterval(() => {
    if (currentState.value === 'idle' || currentState.value === 'listening') {
      isBlinking.value = true;
      setTimeout(() => { isBlinking.value = false; }, 160);
    }
  }, 4000);
};

const startLipSyncTimer = () => {
  lipSyncInterval = window.setInterval(() => {
    // Jika tidak ada sinyal RMS audio live dalam 300ms, gunakan natural fallback toggle
    if (Date.now() - lastLipSyncEventTime > 300) {
      if (currentState.value === 'speaking') {
        isMouthOpen.value = !isMouthOpen.value;
      } else {
        isMouthOpen.value = false;
      }
    }
  }, 140);
};

// ─────────────────────────────────────────────────────────────────────────────
// Drag window
// ─────────────────────────────────────────────────────────────────────────────
const startDrag = async () => {
  try { await getCurrentWindow().startDragging(); }
  catch (e) { console.warn('Drag hanya berjalan dalam Tauri window:', e); }
};

// ─────────────────────────────────────────────────────────────────────────────
// Tauri Commands
// ─────────────────────────────────────────────────────────────────────────────
const triggerState = async (
  state: AssistantState,
  msg: string,
  cmd: string | null = null
) => {
  try {
    await invoke('set_manual_state', { newState: state, message: msg, command: cmd });
  } catch {
    currentState.value = state;
    message.value = msg;
    pendingCommand.value = cmd;
  }
};

const toggleMinimize = async () => {
  try { await invoke('toggle_minimize'); }
  catch { isMinimized.value = !isMinimized.value; }
};

const toggleMic = async () => {
  try {
    await invoke('trigger_mic');
  } catch (e) {
    console.error('Trigger mic error:', e);
  }
};

// Kirim pesan chat ke LLM
const sendChat = async () => {
  const text = chatInput.value.trim();
  if (!text || isSendingChat.value) return;

  chatInput.value = '';
  isSendingChat.value = true;

  try {
    await invoke('ask_llm', { userMessage: text });
  } catch (e) {
    console.error('LLM error:', e);
  } finally {
    isSendingChat.value = false;
  }
};

const handleChatKeydown = (e: KeyboardEvent) => {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    sendChat();
  }
};

// ─────────────────────────────────────────────────────────────────────────────
// Pengaturan & Kustomisasi Sprite
// ─────────────────────────────────────────────────────────────────────────────
const activeSettingsTab = ref<'ai' | 'avatar'>('ai');

const avatarFrameList = [
  { key: 'idle_open', label: 'Idle (Mata Buka)', desc: 'Frame diam normal' },
  { key: 'idle_blink', label: 'Idle (Kedip)', desc: 'Frame mata tertutup' },
  { key: 'listening', label: 'Mendengarkan', desc: 'Saat mic aktif' },
  { key: 'thinking', label: 'Berpikir', desc: 'Saat LLM memproses' },
  { key: 'speak_open', label: 'Bicara (Mulut Buka)', desc: 'Lip-sync frame 2' },
  { key: 'speak_closed', label: 'Bicara (Mulut Tutup)', desc: 'Lip-sync frame 1' },
  { key: 'confirm', label: 'Konfirmasi', desc: 'Dialog bash Alt+Y' },
  { key: 'error', label: 'Error', desc: 'Koneksi gagal / error' },
];

const loadCustomAvatar = async (folder?: string) => {
  try {
    const pack = await invoke<Record<string, string>>('load_avatar_pack', { folderOverride: folder || null });
    if (pack && Object.keys(pack).length > 0) {
      customAvatarPack.value = { ...customAvatarPack.value, ...pack };
    }
  } catch (e) {
    console.warn('Gagal memuat avatar pack:', e);
  }
};

const handleSpriteUpload = (event: Event, frameKey: string) => {
  const target = event.target as HTMLInputElement;
  if (!target.files || target.files.length === 0) return;
  const file = target.files[0];
  const reader = new FileReader();
  reader.onload = async () => {
    const dataUrl = reader.result as string;
    customAvatarPack.value[frameKey] = dataUrl;
    try {
      await invoke('save_avatar_sprite', { frameName: frameKey, dataUrl });
    } catch (e) {
      console.error('Gagal menyimpan sprite ke disk:', e);
    }
  };
  reader.readAsDataURL(file);
};

const handleBatchUpload = (event: Event) => {
  const target = event.target as HTMLInputElement;
  if (!target.files || target.files.length === 0) return;
  const file = target.files[0];
  const reader = new FileReader();
  reader.onload = async () => {
    const dataUrl = reader.result as string;
    for (const item of avatarFrameList) {
      customAvatarPack.value[item.key] = dataUrl;
    }
    try {
      await invoke('save_avatar_pack', { pack: customAvatarPack.value });
    } catch (e) {
      console.error('Gagal menyimpan avatar pack:', e);
    }
  };
  reader.readAsDataURL(file);
};

const removeSingleSprite = async (frameKey: string) => {
  customAvatarPack.value[frameKey] = '';
  try {
    await invoke('save_avatar_pack', { pack: customAvatarPack.value });
  } catch (e) {
    console.error('Gagal mengupdate sprite:', e);
  }
};

const resetToBuiltInAvatar = async () => {
  for (const item of avatarFrameList) {
    customAvatarPack.value[item.key] = '';
  }
  try {
    await invoke('reset_avatar_pack');
  } catch (e) {
    console.error('Gagal reset avatar pack:', e);
  }
};

const openSettings = async () => {
  try {
    const loaded = await invoke<AppConfig>('get_config');
    configData.value = loaded;
  } catch (e) {
    console.warn('Gagal membaca config:', e);
  }
  showSettings.value = true;
};

const saveSettings = async () => {
  isSavingConfig.value = true;
  try {
    await invoke('save_config', { cfg: configData.value });
    showSettings.value = false;
  } catch (e) {
    console.error('Gagal menyimpan config:', e);
  } finally {
    isSavingConfig.value = false;
  }
};

// ─────────────────────────────────────────────────────────────────────────────
// Lifecycle
// ─────────────────────────────────────────────────────────────────────────────
onMounted(async () => {
  startBlinkTimer();
  startLipSyncTimer();

  try {
    const initial = await invoke<StatePayload>('get_current_state');
    currentState.value = initial.state;
    message.value = initial.message;
    pendingCommand.value = initial.pending_command;
    isMinimized.value = initial.minimized;

    unlistenEvent = await listen<StatePayload>('assistant-state-changed', (event) => {
      currentState.value = event.payload.state;
      message.value = event.payload.message;
      pendingCommand.value = event.payload.pending_command;
      isMinimized.value = event.payload.minimized;
    });

    unlistenLipSync = await listen<LipSyncPayload>('lip-sync-update', (event) => {
      lastLipSyncEventTime = Date.now();
      isMouthOpen.value = event.payload.is_mouth_open;
    });

    await loadCustomAvatar();
  } catch {
    console.info('Mode preview web — tanpa koneksi Tauri backend.');
  }
});

onUnmounted(() => {
  if (blinkInterval)  clearInterval(blinkInterval);
  if (lipSyncInterval) clearInterval(lipSyncInterval);
  if (unlistenEvent)  unlistenEvent();
  if (unlistenLipSync) unlistenLipSync();
});
</script>

<template>
  <div
    class="w-screen h-screen overflow-hidden select-none flex flex-col items-end justify-end p-3 transition-opacity duration-300"
    :class="{ 'opacity-35 hover:opacity-100 transition-opacity': ghostMode }"
    @contextmenu.prevent="showDevToolbar = !showDevToolbar"
  >

    <!-- ══════════════════════════════════════════════ -->
    <!-- MODE MINIMIZE: Pill mengambang kecil           -->
    <!-- ══════════════════════════════════════════════ -->
    <div
      v-if="isMinimized"
      @mousedown="startDrag"
      class="flex items-center gap-2 bg-slate-900/90 backdrop-blur-md border border-slate-700/80 px-3.5 py-2 rounded-full shadow-lg cursor-move"
    >
      <span class="w-2.5 h-2.5 rounded-full animate-pulse shadow-sm" :class="stateBadgeColor" />
      <span class="text-xs font-medium text-slate-200 uppercase tracking-wider">{{ stateLabel }}</span>
      <button
        @click.stop="toggleMinimize"
        class="ml-1 text-xs text-sky-400 hover:text-sky-300 font-semibold"
      >▲ Buka</button>
    </div>

    <!-- ══════════════════════════════════════════════ -->
    <!-- MODE UTAMA                                     -->
    <!-- ══════════════════════════════════════════════ -->
    <div v-else class="w-full max-w-[320px] flex flex-col items-center gap-2">

      <!-- ── Panel Modal Pengaturan & Kustomisasi Sprite ── -->
      <div 
        v-if="showSettings" 
        class="w-full bg-slate-900/95 backdrop-blur-md border border-slate-700/90 rounded-2xl p-3 shadow-2xl text-slate-100 flex flex-col gap-2 animate-fade-in"
      >
        <div class="flex items-center justify-between border-b border-slate-800 pb-1.5">
          <!-- Tab selector -->
          <div class="flex items-center gap-1">
            <button 
              @click="activeSettingsTab = 'ai'"
              class="text-[10px] font-bold px-2 py-0.5 rounded transition"
              :class="activeSettingsTab === 'ai' ? 'bg-sky-600 text-white shadow-sm' : 'text-slate-400 hover:text-slate-200 bg-slate-800/80'"
            >
              ⚙️ AI & Model
            </button>
            <button 
              @click="activeSettingsTab = 'avatar'"
              class="text-[10px] font-bold px-2 py-0.5 rounded transition"
              :class="activeSettingsTab === 'avatar' ? 'bg-sky-600 text-white shadow-sm' : 'text-slate-400 hover:text-slate-200 bg-slate-800/80'"
            >
              🎭 Sprite Avatar
            </button>
          </div>
          <button @click="showSettings = false" class="text-xs text-slate-400 hover:text-white px-1">✕</button>
        </div>

        <!-- ── TAB 1: AI & Provider Settings ── -->
        <div v-if="activeSettingsTab === 'ai'" class="space-y-2 text-[10px] max-h-[230px] overflow-y-auto pr-1">
          <div>
            <label class="block text-slate-400 font-medium mb-0.5">Provider LLM</label>
            <select 
              v-model="configData.llm_provider"
              class="w-full bg-slate-800 border border-slate-700 rounded px-2 py-1 text-slate-200 outline-none"
            >
              <option value="gemini">Google Gemini</option>
              <option value="groq">Groq (Ultra-Fast)</option>
              <option value="openai">OpenAI</option>
              <option value="openrouter">OpenRouter</option>
              <option value="ollama">Ollama (Lokal)</option>
              <option value="custom">Custom (OpenAI-Compatible)</option>
            </select>
          </div>

          <div>
            <label class="block text-slate-400 font-medium mb-0.5">Nama Model</label>
            <input 
              v-model="configData.model_name" 
              placeholder="e.g. deepseek-chat / llama-3.3-70b / gemini-2.0-flash"
              class="w-full bg-slate-800 border border-slate-700 rounded px-2 py-1 text-slate-200 outline-none"
            />
          </div>

          <!-- API Key & Endpoint Sesuai Provider -->
          <div v-if="configData.llm_provider === 'custom'">
            <label class="block text-slate-400 font-medium mb-0.5">API Endpoint URL</label>
            <input 
              v-model="configData.custom_api_endpoint" 
              placeholder="e.g. https://api.deepseek.com/v1 atau http://localhost:8000/v1"
              class="w-full bg-slate-800 border border-slate-700 rounded px-2 py-1 text-slate-200 outline-none"
            />
            <span class="text-[9px] text-slate-500 mt-0.5 block">Format: https://domain/v1 (otomatis ke /chat/completions)</span>
          </div>

          <div v-if="configData.llm_provider === 'custom'">
            <label class="block text-slate-400 font-medium mb-0.5">Custom API Key</label>
            <input 
              v-model="configData.custom_api_key" 
              type="password" 
              placeholder="sk-... (kosongkan jika local tanpa auth)"
              class="w-full bg-slate-800 border border-slate-700 rounded px-2 py-1 text-slate-200 outline-none"
            />
          </div>

          <div v-if="configData.llm_provider === 'gemini'">
            <label class="block text-slate-400 font-medium mb-0.5">Gemini API Key</label>
            <input 
              v-model="configData.gemini_api_key" 
              type="password" 
              placeholder="AIzaSy..."
              class="w-full bg-slate-800 border border-slate-700 rounded px-2 py-1 text-slate-200 outline-none"
            />
          </div>

          <div v-if="configData.llm_provider === 'groq'">
            <label class="block text-slate-400 font-medium mb-0.5">Groq API Key (juga dipakai untuk Whisper STT)</label>
            <input 
              v-model="configData.groq_api_key" 
              type="password" 
              placeholder="gsk_..."
              class="w-full bg-slate-800 border border-slate-700 rounded px-2 py-1 text-slate-200 outline-none"
            />
          </div>

          <div v-if="configData.llm_provider === 'openai'">
            <label class="block text-slate-400 font-medium mb-0.5">OpenAI API Key</label>
            <input 
              v-model="configData.openai_api_key" 
              type="password" 
              placeholder="sk-..."
              class="w-full bg-slate-800 border border-slate-700 rounded px-2 py-1 text-slate-200 outline-none"
            />
          </div>

          <div v-if="configData.llm_provider === 'openrouter'">
            <label class="block text-slate-400 font-medium mb-0.5">OpenRouter API Key</label>
            <input 
              v-model="configData.openrouter_api_key" 
              type="password" 
              placeholder="sk-or-v1-..."
              class="w-full bg-slate-800 border border-slate-700 rounded px-2 py-1 text-slate-200 outline-none"
            />
          </div>

          <div v-if="configData.llm_provider === 'ollama'">
            <label class="block text-slate-400 font-medium mb-0.5">Ollama Base URL</label>
            <input 
              v-model="configData.ollama_base_url" 
              placeholder="http://localhost:11434"
              class="w-full bg-slate-800 border border-slate-700 rounded px-2 py-1 text-slate-200 outline-none"
            />
          </div>

          <div>
            <label class="block text-slate-400 font-medium mb-0.5">Folder Avatar Kustom (Opsional)</label>
            <input 
              v-model="configData.avatar_folder_path" 
              placeholder="/home/user/.config/aoi/avatar"
              class="w-full bg-slate-800 border border-slate-700 rounded px-2 py-1 text-slate-200 outline-none"
            />
          </div>
        </div>

        <!-- ── TAB 2: Upload Sprite Avatar ── -->
        <div v-else-if="activeSettingsTab === 'avatar'" class="space-y-2 text-[10px] max-h-[230px] overflow-y-auto pr-1">
          <!-- Aksi Cepat -->
          <div class="flex items-center justify-between bg-slate-800/80 p-1.5 rounded-lg border border-slate-700/60">
            <div>
              <div class="font-bold text-slate-200">Aset 2D Sprite</div>
              <div class="text-[8px] text-slate-400">Format PNG / WebP transparan (512x512)</div>
            </div>
            <div class="flex gap-1">
              <label class="cursor-pointer bg-sky-700 hover:bg-sky-600 text-white text-[9px] font-bold px-2 py-1 rounded transition flex items-center gap-1">
                ⚡ 1 Gambar Semua
                <input type="file" accept="image/png,image/webp,image/jpeg,image/svg+xml" class="hidden" @change="handleBatchUpload" />
              </label>
              <button 
                @click="resetToBuiltInAvatar"
                title="Reset ke karakter anime SVG bawaan"
                class="bg-rose-900/60 hover:bg-rose-800 text-rose-200 text-[9px] font-bold px-2 py-1 rounded transition"
              >
                ↺ Reset
              </button>
            </div>
          </div>

          <!-- Grid 8 Frame Sprite -->
          <div class="grid grid-cols-2 gap-1.5">
            <div 
              v-for="item in avatarFrameList" 
              :key="item.key"
              class="bg-slate-800/60 border border-slate-700/60 rounded-lg p-1.5 flex items-center gap-2 hover:border-slate-600 transition"
            >
              <!-- Thumbnail Preview -->
              <div class="w-10 h-10 rounded bg-black/50 border border-slate-700/80 flex items-center justify-center overflow-hidden flex-shrink-0">
                <img 
                  v-if="customAvatarPack[item.key]" 
                  :src="customAvatarPack[item.key]" 
                  class="w-full h-full object-contain"
                />
                <span v-else class="text-[12px] opacity-30 select-none">🖼️</span>
              </div>

              <!-- Metadata & Tombol Upload -->
              <div class="flex-1 min-w-0">
                <div class="text-[9px] font-bold text-slate-200 truncate">{{ item.label }}</div>
                <div class="text-[8px] text-slate-400 truncate">{{ item.desc }}</div>
                <div class="flex items-center gap-1 mt-1">
                  <label class="cursor-pointer bg-slate-700 hover:bg-slate-600 text-slate-200 text-[8px] font-semibold px-1.5 py-0.5 rounded transition">
                    Upload
                    <input type="file" accept="image/png,image/webp,image/jpeg,image/svg+xml" class="hidden" @change="e => handleSpriteUpload(e, item.key)" />
                  </label>
                  <button 
                    v-if="customAvatarPack[item.key]"
                    @click="removeSingleSprite(item.key)"
                    title="Hapus sprite frame ini"
                    class="text-rose-400 hover:text-rose-300 text-[8px] px-1 font-bold"
                  >
                    ✕
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Tombol Simpan & Tutup -->
        <div class="flex gap-2 mt-1">
          <button 
            @click="saveSettings" 
            :disabled="isSavingConfig"
            class="flex-1 bg-sky-600 hover:bg-sky-500 text-white font-bold py-1 rounded text-[10px] transition"
          >
            {{ isSavingConfig ? 'Menyimpan...' : 'Simpan Pengaturan' }}
          </button>
          <button 
            @click="showSettings = false"
            class="px-3 bg-slate-800 hover:bg-slate-700 text-slate-300 rounded text-[10px]"
          >
            Tutup
          </button>
        </div>
      </div>

      <!-- ── Balon Dialog RPG ── -->
      <div v-else class="w-full bg-slate-900/90 backdrop-blur-md border border-slate-700/80 rounded-2xl p-3 shadow-xl text-slate-100">

        <!-- Header: Status + Tombol Kontrol -->
        <div
          @mousedown="startDrag"
          class="flex items-center justify-between border-b border-slate-800 pb-1.5 mb-2 cursor-move"
        >
          <div class="flex items-center gap-1.5">
            <span class="w-2 h-2 rounded-full shadow-sm" :class="stateBadgeColor" />
            <span class="text-[10px] font-bold uppercase tracking-widest text-slate-400">
              Aoi • {{ stateLabel }}
            </span>
          </div>
          <div class="flex items-center gap-1">
            <button
              @click.stop="openSettings"
              title="Pengaturan API & Model"
              class="text-[10px] px-1.5 py-0.5 rounded bg-slate-800 hover:bg-slate-700 text-slate-300"
            >⚙️</button>
            <button
              @click.stop="ghostMode = !ghostMode"
              title="Ghost Mode"
              class="text-[10px] px-1.5 py-0.5 rounded bg-slate-800 hover:bg-slate-700 text-slate-300"
            >👻</button>
            <button
              @click.stop="toggleMinimize"
              title="Minimize (Super+H)"
              class="text-[10px] px-1.5 py-0.5 rounded bg-slate-800 hover:bg-slate-700 text-slate-300"
            >_</button>
          </div>
        </div>

        <!-- Teks pesan AI -->
        <p class="text-xs leading-relaxed text-slate-200 min-h-[2rem] break-words whitespace-pre-wrap">
          {{ message }}
        </p>

        <!-- Thinking spinner -->
        <div v-if="currentState === 'thinking'" class="flex items-center gap-1.5 mt-1.5">
          <span class="w-1.5 h-1.5 rounded-full bg-amber-400 animate-bounce" style="animation-delay:0ms" />
          <span class="w-1.5 h-1.5 rounded-full bg-amber-400 animate-bounce" style="animation-delay:150ms" />
          <span class="w-1.5 h-1.5 rounded-full bg-amber-400 animate-bounce" style="animation-delay:300ms" />
        </div>

        <!-- ── Konfirmasi Red Tier ── -->
        <div
          v-if="currentState === 'confirm' && pendingCommand"
          class="mt-2.5 p-2 rounded-lg bg-slate-950/90 border border-purple-500/40"
        >
          <div class="text-[10px] text-purple-300 font-semibold mb-1">⚠️ Konfirmasi Perintah Terminal:</div>
          <code class="block text-[11px] font-mono text-emerald-400 bg-black/50 px-2 py-1 rounded break-all">
            $ {{ pendingCommand }}
          </code>
          <div class="flex gap-2 mt-2">
            <button
              @click="triggerState('speaking', 'Menjalankan: ' + pendingCommand)"
              class="flex-1 bg-emerald-600 hover:bg-emerald-500 text-white text-[10px] font-bold py-1 rounded transition"
            >[Alt+Y] Setuju</button>
            <button
              @click="triggerState('idle', 'Perintah dibatalkan.')"
              class="flex-1 bg-rose-600/80 hover:bg-rose-600 text-white text-[10px] font-bold py-1 rounded transition"
            >[Alt+N] Batal</button>
          </div>
        </div>

        <!-- ── Chat input bar & Tombol Mic ── -->
        <div class="flex gap-1.5 mt-2.5">
          <button
            @click="toggleMic"
            :title="currentState === 'listening' ? 'Hentikan & Proses Suara' : 'Mulai Bicara (Super+V)'"
            class="px-2.5 py-1.5 rounded-lg border text-[11px] font-bold transition flex items-center justify-center"
            :class="currentState === 'listening' ? 'bg-emerald-600 border-emerald-400 text-white animate-pulse' : 'bg-slate-800 border-slate-700 text-slate-300 hover:bg-slate-700'"
          >
            🎙️
          </button>
          <input
            v-model="chatInput"
            @keydown="handleChatKeydown"
            :disabled="isSendingChat || currentState === 'thinking'"
            placeholder="Ketik pesan... (Enter untuk kirim)"
            class="flex-1 text-[11px] bg-slate-800/80 border border-slate-700 rounded-lg px-2.5 py-1.5 text-slate-200 placeholder-slate-500 outline-none focus:border-sky-500/70 transition disabled:opacity-50"
          />
          <button
            @click="sendChat"
            :disabled="isSendingChat || !chatInput.trim() || currentState === 'thinking'"
            class="px-2.5 py-1.5 rounded-lg bg-sky-600 hover:bg-sky-500 disabled:opacity-40 text-white text-[11px] font-bold transition"
          >
            <span v-if="isSendingChat">⏳</span>
            <span v-else>➤</span>
          </button>
        </div>
      </div>

      <!-- ── Sprite Avatar 2D ── -->
      <div
        @mousedown="startDrag"
        class="relative w-44 h-44 flex items-center justify-center cursor-move"
      >
        <!-- Custom PNG/WebP -->
        <img
          v-if="activeCustomFrame"
          :src="activeCustomFrame"
          alt="Aoi Avatar"
          class="w-full h-full object-contain pointer-events-none select-none drop-shadow-2xl"
        />

        <!-- Built-in SVG chibi -->
        <svg
          v-else
          viewBox="0 0 200 200"
          class="w-full h-full drop-shadow-xl transition-transform duration-200"
          :class="{
            'scale-105': currentState === 'listening',
            'translate-y-[-2px]': currentState === 'speaking' && isMouthOpen
          }"
        >
          <!-- Aura sesuai state -->
          <circle
            cx="100" cy="105" r="72"
            class="transition-colors duration-500"
            :fill="currentState === 'listening' ? 'rgba(16,185,129,0.22)'
                 : currentState === 'thinking'  ? 'rgba(245,158,11,0.22)'
                 : currentState === 'error'      ? 'rgba(244,63,94,0.25)'
                 : currentState === 'confirm'    ? 'rgba(168,85,247,0.20)'
                 : 'rgba(56,189,248,0.12)'"
          />

          <!-- Rambut belakang -->
          <path d="M45 90 Q35 150 55 175 L145 175 Q165 150 155 90 Z" fill="#312e81"/>

          <!-- Tubuh / hoodie -->
          <path d="M60 150 Q100 140 140 150 L150 190 L50 190 Z" fill="#4f46e5"/>
          <path d="M85 150 L100 175 L115 150" fill="#e0e7ff"/>

          <!-- Wajah -->
          <circle cx="100" cy="102" r="46" fill="#ffedd5"/>

          <!-- Rambut depan / poni -->
          <path d="M52 95 Q55 50 100 50 Q145 50 148 95 Q130 70 115 90 Q100 68 85 90 Q70 70 52 95 Z" fill="#4338ca"/>

          <!-- Telinga kucing -->
          <polygon points="55,68 42,40 75,55" fill="#6366f1"/>
          <polygon points="145,68 158,40 125,55" fill="#6366f1"/>

          <!-- MATA -->
          <!-- Error: X X -->
          <g v-if="currentState === 'error'" stroke="#e11d48" stroke-width="3.5" stroke-linecap="round">
            <line x1="72" y1="96" x2="86" y2="110"/>
            <line x1="86" y1="96" x2="72" y2="110"/>
            <line x1="114" y1="96" x2="128" y2="110"/>
            <line x1="128" y1="96" x2="114" y2="110"/>
          </g>
          <!-- Berkedip -->
          <g v-else-if="isBlinking" stroke="#1e1b4b" stroke-width="3" fill="none" stroke-linecap="round">
            <path d="M70 104 Q79 110 88 104"/>
            <path d="M112 104 Q121 110 130 104"/>
          </g>
          <!-- Thinking: mata naik ke atas -->
          <g v-else-if="currentState === 'thinking'">
            <ellipse cx="79" cy="100" rx="7" ry="8" fill="#1e1b4b"/>
            <ellipse cx="121" cy="100" rx="7" ry="8" fill="#1e1b4b"/>
            <circle cx="79" cy="96" r="3" fill="#fff"/>
            <circle cx="121" cy="96" r="3" fill="#fff"/>
          </g>
          <!-- Normal -->
          <g v-else>
            <ellipse cx="79" cy="103" rx="7.5" ry="9" fill="#1e1b4b"/>
            <ellipse cx="121" cy="103" rx="7.5" ry="9" fill="#1e1b4b"/>
            <circle cx="76" cy="99" r="3" fill="#fff"/>
            <circle cx="118" cy="99" r="3" fill="#fff"/>
          </g>

          <!-- Pipi -->
          <ellipse cx="66" cy="115" rx="6" ry="3" fill="#fda4af" opacity="0.7"/>
          <ellipse cx="134" cy="115" rx="6" ry="3" fill="#fda4af" opacity="0.7"/>

          <!-- MULUT -->
          <ellipse
            v-if="currentState === 'speaking' && isMouthOpen"
            cx="100" cy="124" rx="7" ry="6" fill="#f43f5e"
          />
          <ellipse
            v-else-if="currentState === 'error' || currentState === 'confirm'"
            cx="100" cy="125" rx="5" ry="5" fill="none" stroke="#1e1b4b" stroke-width="2.5"
          />
          <path
            v-else
            d="M93 122 Q100 128 107 122"
            fill="none" stroke="#1e1b4b" stroke-width="2.5" stroke-linecap="round"
          />

          <!-- Ikon Mic Listening -->
          <g v-if="currentState === 'listening'">
            <circle cx="155" cy="65" r="14" fill="#10b981"/>
            <rect x="151" y="56" width="8" height="12" rx="4" fill="white"/>
            <path d="M148 64 Q155 73 162 64" fill="none" stroke="white" stroke-width="2"/>
          </g>

          <!-- Balon pikiran Thinking -->
          <g v-if="currentState === 'thinking'">
            <circle cx="152" cy="75" r="4" fill="#fbbf24"/>
            <circle cx="162" cy="63" r="6" fill="#fbbf24"/>
            <circle cx="172" cy="48" r="10" fill="#fbbf24"/>
          </g>

          <!-- Tanda seru Error -->
          <g v-if="currentState === 'error'">
            <circle cx="155" cy="55" r="14" fill="#f43f5e"/>
            <text x="155" y="61" text-anchor="middle" font-size="16" fill="white" font-weight="bold">!</text>
          </g>
        </svg>
      </div>

      <!-- ── Dev Toolbar (klik kanan untuk sembunyikan) ── -->
      <div
        v-if="showDevToolbar"
        class="w-full bg-slate-900/95 border border-slate-700/80 rounded-xl p-2 shadow-lg"
      >
        <div class="text-[9px] text-slate-400 text-center mb-1.5">
          🛠️ Dev Panel — Klik Kanan Avatar untuk Sembunyikan
        </div>
        <div class="grid grid-cols-3 gap-1">
          <button @click="triggerState('idle','Standby... Tekan Super+V untuk bicara.')"
            class="text-[10px] py-1 rounded bg-slate-800 hover:bg-slate-700 text-slate-200">Idle</button>
          <button @click="triggerState('listening','Mendengarkan suaramu...')"
            class="text-[10px] py-1 rounded bg-emerald-900/60 hover:bg-emerald-800 text-emerald-200">Listening</button>
          <button @click="triggerState('thinking','Sedang memikirkan jawaban...')"
            class="text-[10px] py-1 rounded bg-amber-900/60 hover:bg-amber-800 text-amber-200">Thinking</button>
          <button @click="triggerState('speaking','Halo! Aku sedang berbicara dengan lip-sync!')"
            class="text-[10px] py-1 rounded bg-sky-900/60 hover:bg-sky-800 text-sky-200">Speaking</button>
          <button @click="triggerState('confirm','Izinkan saya menghapus cache paket?','sudo pacman -Scc')"
            class="text-[10px] py-1 rounded bg-purple-900/60 hover:bg-purple-800 text-purple-200">Confirm</button>
          <button @click="triggerState('error','Waduh! Koneksi ke server LLM terputus.')"
            class="text-[10px] py-1 rounded bg-rose-900/60 hover:bg-rose-800 text-rose-200">Error</button>
        </div>
      </div>

    </div>
  </div>
</template>

<style>
html, body, #app {
  background: transparent !important;
  margin: 0;
  padding: 0;
  overflow: hidden;
  font-family: 'Inter', 'Noto Sans', system-ui, sans-serif;
}
</style>