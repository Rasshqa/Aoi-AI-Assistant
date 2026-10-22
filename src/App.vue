<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed, watch } from 'vue';
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
  assistant_name?: string;
  user_name?: string;
  personality_preset?: string;
  custom_personality?: string;
  avatar_size?: number;
  tts_engine?: string;
  edge_tts_voice?: string;
  edge_tts_rate?: string;
  elevenlabs_api_key?: string;
  elevenlabs_voice_id?: string;
  openai_speech_endpoint?: string;
  openai_speech_key?: string;
  openai_speech_voice?: string;
  active_avatar_preset?: string;
  active_cloned_voice_id?: string;
  fish_audio_api_key?: string;
  colab_clone_endpoint?: string;
}

interface AvatarPresetMeta {
  id: string;
  name: string;
  thumbnail: string;
  frame_count: number;
  is_active: boolean;
}

interface ClonedVoiceMeta {
  id: string;
  name: string;
  engine: string;
  remote_voice_id: string;
  sample_filename?: string;
  created_at: number;
  is_active: boolean;
}

interface LipSyncPayload {
  is_mouth_open: boolean;
}

// ─────────────────────────────────────────────────────────────────────────────
// State Reaktif
// ─────────────────────────────────────────────────────────────────────────────
const currentState = ref<AssistantState>('idle');
const message = ref<string>('Halo! Tekan Super+V atau ketik pesan untuk mulai ngobrol.');
const pendingCommand = ref<string | null>(null);
const isMinimized = ref<boolean>(false);
const ghostMode = ref<boolean>(false);
const showDevToolbar = ref<boolean>(false);
const showSettings = ref<boolean>(false);
// Menu bar / balon chat default di-hide (hanya muncul saat klik kiri avatar atau saat AI merespons)
const showMenuBar = ref<boolean>(false);
// Ukuran dinamis avatar desktop (default 144px, rentang 80px - 260px)
const avatarSize = ref<number>(
  Number(localStorage.getItem('aoi_avatar_size')) || 144
);

// Sinkronisasi ukuran window dinamis: compact saat avatar standby, expanded saat menu dibuka
const syncWindowBounds = async () => {
  try {
    const isExpanded = showSettings.value || showMenuBar.value;
    await invoke('update_window_mode', {
      expanded: isExpanded,
      avatarSize: avatarSize.value,
    });
  } catch (e) {
    // Mode browser / non-tauri
  }
};

watch([showMenuBar, showSettings, avatarSize], () => {
  syncWindowBounds();
});

// Tabs pengaturan: 'persona' | 'ai' | 'voice' | 'presets' | 'avatar'
const activeSettingsTab = ref<'persona' | 'ai' | 'voice' | 'presets' | 'avatar'>('persona');

// Template Kepribadian & Sifat Asisten (Bisa diisi bebas berupa teks)
const personalityTemplates = [
  {
    id: 'cheerful',
    title: 'Ceria & Manis',
    badge: 'Genki',
    text: 'Ceria, manis, hangat, dan bersemangat. Selalu ramah, suportif, dan menyenangkan didengar. Gunakan bahasa santai kasual anak muda.',
    sample: (name: string, user: string) => `Halo ${user}! Aku ${name}, senang banget bisa nemenin kamu hari ini, hehe~`,
  },
  {
    id: 'caring',
    title: 'Lembut & Perhatian',
    badge: 'Onee-san',
    text: 'Dewasa, tenang, dan penuh perhatian (Onee-san). Nada menenangkan, hangat, dan selalu peduli kenyamanan pengguna saat lelah.',
    sample: (name: string, user: string) => `Halo ${user}... ${name} di sini. Kalau lelah jangan dipaksa ya, istirahat sebentar~`,
  },
  {
    id: 'tsundere',
    title: 'Tsundere Gemas',
    badge: 'Tsundere',
    text: 'Tsundere imut. Gengsi dan agak ketus di awal tapi sebenarnya sangat peduli, loyal, dan perhatian. Sering pakai ekspresi seperti Hmph!, tapi selalu sigap membantu.',
    sample: (name: string, user: string) => `H-hmph! Tumben panggil ${name}, ${user}? B-bukan berarti aku nungguin kamu ya!`,
  },
  {
    id: 'cool',
    title: 'Keren & Kalem',
    badge: 'Kuudere',
    text: 'Tenang, kalem, ringkas, dan to-the-point. Sangat cerdas, tidak banyak basa-basi, efisien, dan selalu dapat diandalkan.',
    sample: (name: string, user: string) => `Sistem ${name} siap, ${user}. Ada yang perlu dieksekusi sekarang?`,
  },
  {
    id: 'gamer',
    title: 'Gamer Kasual',
    badge: 'Gamer',
    text: 'Teman gamer santai. Gunakan istilah kasual gamer, suka humor game, santai dan ceplas-ceplos.',
    sample: (name: string, user: string) => `Yo ${user}! ${name} di sini, udah push rank atau masih nugas nih? Sini aku temenin!`,
  },
  {
    id: 'productive',
    title: 'Asisten Efisien',
    badge: 'Produktif',
    text: 'Asisten kerja cerdas, ringkas, sopan, dan terstruktur. Fokus membantu administrasi sistem Linux, coding, dan tugas harian.',
    sample: (name: string, user: string) => `Selamat datang kembali, ${user}. Sistem ${name} aktif dan siap membantu pekerjaanmu.`,
  },
];

const applyPersonalityTemplate = (t: typeof personalityTemplates[0]) => {
  configData.value.custom_personality = t.text;
  configData.value.personality_preset = t.id;
};

// Input chat
const chatInput = ref<string>('');
const isSendingChat = ref<boolean>(false);

// Animasi frame
const isBlinking = ref<boolean>(false);
const isMouthOpen = ref<boolean>(false);

// Pengaturan Konfigurasi
const configData = ref<AppConfig>({
  llm_provider: 'custom',
  gemini_api_key: '',
  groq_api_key: '',
  openai_api_key: '',
  openrouter_api_key: '',
  ollama_base_url: 'http://localhost:11434',
  custom_api_key: '',
  custom_api_endpoint: '',
  model_name: 'Eris-Max',
  tts_model_path: '',
  whisper_model_path: '',
  avatar_folder_path: '',
  assistant_name: 'Aoi',
  user_name: 'Kak',
  personality_preset: 'cheerful',
  custom_personality: 'Ceria, manis, hangat, dan bersemangat. Selalu ramah, suportif, dan menyenangkan didengar. Gunakan bahasa santai kasual anak muda.',
  avatar_size: 144,
  tts_engine: 'edge_tts',
  edge_tts_voice: 'id-ID-GadisNeural',
  edge_tts_rate: '+10%',
  elevenlabs_api_key: '',
  elevenlabs_voice_id: '',
  openai_speech_endpoint: '',
  openai_speech_key: '',
  openai_speech_voice: 'nova',
  active_avatar_preset: 'default',
  active_cloned_voice_id: '',
  fish_audio_api_key: '',
  colab_clone_endpoint: '',
});
const isSavingConfig = ref<boolean>(false);
const isTestingVoice = ref<boolean>(false);

// Pilihan Provider TTS Engine
const ttsEngineOptions = [
  { id: 'edge_tts', name: 'Edge-TTS', desc: 'Gratis & Cepat (id-ID-Gadis)' },
  { id: 'fish_audio', name: 'Fish Audio', desc: 'Zero-Shot Kloning Suara' },
  { id: 'elevenlabs', name: 'ElevenLabs', desc: 'Kloning Suara Eksak Waifu' },
  { id: 'openai_speech', name: 'OpenAI / Custom', desc: 'OpenAI Speech / Server Mandiri' },
  { id: 'piper', name: 'Piper Offline', desc: '100% Offline Lokal ONNX' },
];

// Pilihan Vokal Rekomendasi Edge-TTS
const edgeVoicePresets = [
  { id: 'id-ID-GadisNeural', name: 'Gadis (Indonesia Ceria & Manis)', lang: 'ID' },
  { id: 'id-ID-ArdiNeural', name: 'Ardi (Indonesia Pria Santai)', lang: 'ID' },
  { id: 'ja-JP-NanamiNeural', name: 'Nanami (Jepang Anime Waifu)', lang: 'JA' },
  { id: 'ja-JP-AoiNeural', name: 'Aoi (Jepang Natural Lembut)', lang: 'JA' },
  { id: 'ja-JP-KeitaNeural', name: 'Keita (Jepang Pria)', lang: 'JA' },
  { id: 'en-US-AnaNeural', name: 'Ana (Inggris Kartun Ramah)', lang: 'EN' },
];

// State Avatar Presets
const avatarPresets = ref<AvatarPresetMeta[]>([]);
const isLoadingPresets = ref<boolean>(false);
const showNewPresetModal = ref<boolean>(false);
const newPresetName = ref<string>('');
const showQuickAvatarMenu = ref<boolean>(false);
const zipFileInput = ref<HTMLInputElement | null>(null);

// State Voice Cloning Studio
const clonedVoices = ref<ClonedVoiceMeta[]>([]);
const isLoadingClonedVoices = ref<boolean>(false);
const isCloningVoice = ref<boolean>(false);
const cloneEngine = ref<'fish_audio' | 'elevenlabs' | 'custom_server'>('fish_audio');
const newCloneName = ref<string>('');
const manualVoiceId = ref<string>('');
const showManualVoiceModal = ref<boolean>(false);
const audioSampleBase64 = ref<string>('');
const audioSampleFileName = ref<string>('');
const isRecordingMic = ref<boolean>(false);
const recordingSeconds = ref<number>(0);
const playingVoiceId = ref<string | null>(null);
const voiceFileInput = ref<HTMLInputElement | null>(null);

let mediaRecorder: MediaRecorder | null = null;
let recordingChunks: Blob[] = [];
let recordingTimer: number | null = null;
let sampleAudioPlayer: HTMLAudioElement | null = null;

// Custom avatar pack (kosong = pakai SVG built-in)
const customAvatarPack = ref<Record<string, string>>({
  idle_open: '',
  idle_blink: '',
  listening: '',
  thinking: '',
  speak_closed: '',
  speak_open: '',
  confirm: '',
  error: '',
});

// Pilihan provider LLM
const providerOptions = [
  { id: 'custom', name: 'Custom API', desc: 'Port 20128 / Endpoint Mandiri' },
  { id: 'gemini', name: 'Gemini', desc: 'Google Flash 2.0' },
  { id: 'groq', name: 'Groq', desc: 'Llama 3.3 Fast' },
  { id: 'openai', name: 'OpenAI', desc: 'GPT-4o Mini' },
  { id: 'openrouter', name: 'OpenRouter', desc: 'Multi-Provider' },
  { id: 'ollama', name: 'Ollama', desc: 'Lokal Offline' },
];

// Daftar 8 frame avatar
const avatarFrameList = [
  { key: 'idle_open', label: 'Idle Normal', desc: 'Mata terbuka santai' },
  { key: 'idle_blink', label: 'Idle Kedip', desc: 'Mata terpejam sejenak' },
  { key: 'listening', label: 'Mendengarkan', desc: 'Saat mikrofon merekam' },
  { key: 'thinking', label: 'Berpikir', desc: 'Saat LLM memproses respon' },
  { key: 'speak_closed', label: 'Bicara (Tutup)', desc: 'Frame 1 vokal tertutup' },
  { key: 'speak_open', label: 'Bicara (Buka)', desc: 'Frame 2 vokal terbuka' },
  { key: 'confirm', label: 'Konfirmasi', desc: 'Perintah terminal bahaya' },
  { key: 'error', label: 'Error', desc: 'Koneksi gagal atau salah' },
];

// Frame dan emosi yang sedang dipilih di Studio
const selectedPreviewFrame = ref<string>('idle_open');
const previewEmotion = ref<AssistantState>('idle');

let blinkInterval: number | null = null;
let lipSyncInterval: number | null = null;
let unlistenEvent: (() => void) | null = null;
let unlistenLipSync: (() => void) | null = null;
let lastLipSyncEventTime = 0;

// ─────────────────────────────────────────────────────────────────────────────
// Computed
// ─────────────────────────────────────────────────────────────────────────────
const stateBadgeStyle = computed(() => {
  switch (currentState.value) {
    case 'listening':
      return {
        dot: 'bg-rose-500 animate-pulse',
        pill: 'bg-zinc-900/90 border-zinc-700/80 text-rose-300',
      };
    case 'thinking':
      return {
        dot: 'bg-amber-400 animate-pulse',
        pill: 'bg-zinc-900/90 border-zinc-700/80 text-amber-300',
      };
    case 'speaking':
      return {
        dot: 'bg-sky-400 animate-pulse',
        pill: 'bg-zinc-900/90 border-zinc-700/80 text-sky-300',
      };
    case 'confirm':
      return {
        dot: 'bg-purple-400 animate-pulse',
        pill: 'bg-zinc-900/90 border-zinc-700/80 text-purple-300',
      };
    case 'error':
      return {
        dot: 'bg-rose-400',
        pill: 'bg-zinc-900/90 border-zinc-700/80 text-rose-300',
      };
    default:
      return {
        dot: 'bg-emerald-400',
        pill: 'bg-zinc-900/90 border-zinc-800 text-zinc-300',
      };
  }
});

const stateLabel = computed(() => {
  const labels: Record<AssistantState, string> = {
    idle: 'Standby',
    listening: 'Mendengarkan...',
    thinking: 'Memproses...',
    speaking: 'Berbicara',
    confirm: 'Konfirmasi',
    error: 'Kendala',
  };
  return labels[currentState.value];
});

// Frame avatar kustom yang sedang aktif di desktop
const activeCustomFrame = computed(() => {
  const pack = customAvatarPack.value;
  const hasAnyCustom = Object.values(pack).some((v) => Boolean(v && v.trim()));
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

// Frame live preview untuk Sprite Studio
const studioPreviewFrame = computed(() => {
  const pack = customAvatarPack.value;
  // Jika user memilih frame tertentu di grid, tampilkan itu
  if (selectedPreviewFrame.value && pack[selectedPreviewFrame.value]) {
    return pack[selectedPreviewFrame.value];
  }
  // Fallback berdasarkan emosi yang dipilih di preview tester
  const em = previewEmotion.value;
  if (em === 'idle') return pack.idle_open || pack.idle_blink || '';
  if (em === 'speaking') return pack.speak_open || pack.speak_closed || pack.idle_open || '';
  return pack[em] || pack.idle_open || '';
});

const uploadedFrameCount = computed(() => {
  return Object.values(customAvatarPack.value).filter((v) => Boolean(v && v.trim())).length;
});

// Quote preview kepribadian
const currentPersonalityQuote = computed(() => {
  const assistant = configData.value.assistant_name?.trim() || 'Aoi';
  const user = configData.value.user_name?.trim() || 'Kak';
  const text = configData.value.custom_personality?.trim();
  if (text) {
    const matched = personalityTemplates.find((t) => t.text === text || t.id === configData.value.personality_preset);
    if (matched) {
      return matched.sample(assistant, user);
    }
    return text.length > 120 ? `"${text.slice(0, 117)}..."` : `"${text}"`;
  }
  return `Halo ${user}! Aku ${assistant}, siap membantu aktivitasmu di Hyprland.`;
});

// Fitur Hide/Show Preview Sprite & Segment Filter
const showSpritePreview = ref<boolean>(false);
const spriteFilter = ref<'all' | 'idle' | 'emotions'>('all');

const filteredAvatarFrames = computed(() => {
  if (spriteFilter.value === 'idle') {
    return avatarFrameList.filter((f) =>
      ['idle_open', 'idle_blink', 'speak_closed', 'speak_open'].includes(f.key)
    );
  }
  if (spriteFilter.value === 'emotions') {
    return avatarFrameList.filter((f) =>
      ['listening', 'thinking', 'confirm', 'error'].includes(f.key)
    );
  }
  return avatarFrameList;
});

// Sinkronisasi Avatar antara Web Preview & Tauri Desktop App
const AVATAR_STORAGE_KEY = 'aoi_custom_avatar_pack';

const saveAvatarToLocalStorage = (pack: Record<string, string>) => {
  try {
    localStorage.setItem(AVATAR_STORAGE_KEY, JSON.stringify(pack));
  } catch (e) {
    console.warn('Gagal menyimpan avatar ke localStorage:', e);
  }
};

const loadAvatarFromLocalStorage = (): Record<string, string> | null => {
  try {
    const raw = localStorage.getItem(AVATAR_STORAGE_KEY);
    if (raw) return JSON.parse(raw);
  } catch (e) {
    console.warn('Gagal membaca avatar dari localStorage:', e);
  }
  return null;
};

// ─────────────────────────────────────────────────────────────────────────────
// Animasi Timer
// ─────────────────────────────────────────────────────────────────────────────
const startBlinkTimer = () => {
  blinkInterval = window.setInterval(() => {
    if (currentState.value === 'idle' || currentState.value === 'listening') {
      isBlinking.value = true;
      setTimeout(() => {
        isBlinking.value = false;
      }, 160);
    }
  }, 3800);
};

const startLipSyncTimer = () => {
  lipSyncInterval = window.setInterval(() => {
    if (Date.now() - lastLipSyncEventTime > 300) {
      if (currentState.value === 'speaking') {
        isMouthOpen.value = !isMouthOpen.value;
      } else {
        isMouthOpen.value = false;
      }
    }
  }, 130);
};

// ─────────────────────────────────────────────────────────────────────────────
// Scroll Handler Helper (Solusi Pasti untuk Wayland / WebKitGTK)
// ─────────────────────────────────────────────────────────────────────────────
const handleWheelScroll = (e: WheelEvent) => {
  const el = e.currentTarget as HTMLElement;
  if (!el) return;
  let delta = e.deltaY;
  // Di Linux WebKitGTK / Wayland, deltaMode 1 adalah DOM_DELTA_LINE
  if (e.deltaMode === 1) {
    delta *= 35;
  } else if (e.deltaMode === 2) {
    delta *= el.clientHeight;
  }
  if (Math.abs(delta) < 25) {
    delta = Math.sign(delta) * 35;
  }
  el.scrollBy({ top: delta, behavior: 'smooth' });
};

const scrollAreaUp = (id: string) => {
  const el = document.getElementById(id);
  if (el) el.scrollBy({ top: -160, behavior: 'smooth' });
};

const scrollAreaDown = (id: string) => {
  const el = document.getElementById(id);
  if (el) el.scrollBy({ top: 160, behavior: 'smooth' });
};

// ─────────────────────────────────────────────────────────────────────────────
// Drag Window
// ─────────────────────────────────────────────────────────────────────────────
const startDrag = async (e?: MouseEvent) => {
  if (e && (e.target as HTMLElement)?.closest('button, input, textarea, a, svg, path, label')) {
    return;
  }
  try {
    await getCurrentWindow().startDragging();
  } catch (e) {
    console.warn('Drag hanya berjalan dalam Tauri window:', e);
  }
};

// ─────────────────────────────────────────────────────────────────────────────
// Interaksi Ukuran Avatar & Toggle Menu Bar
// ─────────────────────────────────────────────────────────────────────────────
const setAvatarSize = (size: number) => {
  const clamped = Math.min(260, Math.max(80, Math.round(size)));
  avatarSize.value = clamped;
  configData.value.avatar_size = clamped;
  localStorage.setItem('aoi_avatar_size', String(clamped));
  invoke('save_config', { cfg: configData.value }).catch(() => {});
};

// Scroll mouse di atas avatar untuk zoom/resize secara mulus
const handleAvatarWheel = (e: WheelEvent) => {
  e.preventDefault();
  const delta = e.deltaY < 0 ? 8 : -8;
  setAvatarSize(avatarSize.value + delta);
};

// Deteksi klik kiri (buka/tutup menu bar) vs geser posisi (drag window)
let isMouseDownOnAvatar = false;
let avatarMouseDownPos = { x: 0, y: 0 };
let avatarDragStarted = false;

const handleAvatarMouseDown = (e: MouseEvent) => {
  if (e.button !== 0) return; // Hanya respon klik kiri
  if ((e.target as HTMLElement)?.closest('button, input, textarea, a, label')) {
    return;
  }

  isMouseDownOnAvatar = true;
  avatarDragStarted = false;
  avatarMouseDownPos = { x: e.screenX, y: e.screenY };

  const onMouseMove = (moveEvent: MouseEvent) => {
    if (!isMouseDownOnAvatar) return;
    const dist = Math.hypot(moveEvent.screenX - avatarMouseDownPos.x, moveEvent.screenY - avatarMouseDownPos.y);
    if (dist > 5 && !avatarDragStarted) {
      avatarDragStarted = true;
      cleanup();
      startDrag(moveEvent);
    }
  };

  const onMouseUp = (upEvent: MouseEvent) => {
    cleanup();
    if (upEvent.button !== 0) return;
    if (!avatarDragStarted) {
      // Ini adalah klik kiri murni! Buka/Tutup Menu Bar
      showMenuBar.value = !showMenuBar.value;
    }
    isMouseDownOnAvatar = false;
  };

  const cleanup = () => {
    window.removeEventListener('mousemove', onMouseMove);
    window.removeEventListener('mouseup', onMouseUp);
  };

  window.addEventListener('mousemove', onMouseMove);
  window.addEventListener('mouseup', onMouseUp);
};

// ─────────────────────────────────────────────────────────────────────────────
// Tauri IPC Invokes
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
  try {
    await invoke('toggle_minimize');
  } catch {
    isMinimized.value = !isMinimized.value;
  }
};

const toggleMic = async () => {
  try {
    await invoke('trigger_mic');
  } catch (e) {
    console.error('Trigger mic error:', e);
  }
};

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

const confirmBash = async () => {
  try {
    await invoke('confirm_pending_command');
  } catch (e) {
    console.error('Confirm error:', e);
  }
};

const cancelBash = async () => {
  try {
    await invoke('cancel_pending_command');
  } catch (e) {
    console.error('Cancel error:', e);
  }
};

const clearMemory = async () => {
  try {
    await invoke('clear_chat_memory');
  } catch (e) {
    console.error('Clear memory error:', e);
  }
};

const handleGlobalKeydown = (e: KeyboardEvent) => {
  if (currentState.value === 'confirm') {
    if (e.altKey && (e.key === 'y' || e.key === 'Y')) {
      e.preventDefault();
      confirmBash();
      return;
    } else if (e.altKey && (e.key === 'n' || e.key === 'N')) {
      e.preventDefault();
      cancelBash();
      return;
    }
  }

  // Escape untuk tutup modal pengaturan atau sembunyikan menu bar
  if (e.key === 'Escape') {
    if (showSettings.value) {
      e.preventDefault();
      showSettings.value = false;
      return;
    }
    if (showMenuBar.value) {
      e.preventDefault();
      showMenuBar.value = false;
      return;
    }
  }

  // Pintasan Super/Ctrl+, atau Alt+S untuk toggle pengaturan
  if ((e.ctrlKey && e.key === ',') || (e.altKey && (e.key === 's' || e.key === 'S'))) {
    e.preventDefault();
    if (showSettings.value) {
      showSettings.value = false;
    } else {
      openSettings();
    }
  }
};

// ─────────────────────────────────────────────────────────────────────────────
// Sprite & Config Management
// ─────────────────────────────────────────────────────────────────────────────
const loadCustomAvatar = async (folder?: string) => {
  // 1. Muat dari localStorage terlebih dahulu (cache lokal cepat)
  const localData = loadAvatarFromLocalStorage();
  if (localData && Object.keys(localData).length > 0) {
    customAvatarPack.value = { ...customAvatarPack.value, ...localData };
  }

  // 2. Jika dalam runtime Tauri, muat dari disk ~/.config/aoi/avatar/ via IPC
  let loadedFromTauri = false;
  try {
    const pack = await invoke<Record<string, string>>('load_avatar_pack', {
      folderOverride: folder || null,
    });
    if (pack && Object.keys(pack).length > 0) {
      customAvatarPack.value = { ...customAvatarPack.value, ...pack };
      saveAvatarToLocalStorage(customAvatarPack.value);
      loadedFromTauri = true;
    } else if (localData && Object.keys(localData).length > 0) {
      await invoke('save_avatar_pack', { pack: customAvatarPack.value });
      loadedFromTauri = true;
    }
  } catch {
    // Bukan runtime Tauri
  }

  // 3. Jika di Web Preview (di luar Tauri), muat langsung dari dev server API ~/.config/aoi/avatar/
  if (!loadedFromTauri) {
    try {
      const res = await fetch('/api/avatar-pack');
      if (res.ok) {
        const serverPack = await res.json();
        if (serverPack && Object.keys(serverPack).length > 0) {
          customAvatarPack.value = { ...customAvatarPack.value, ...serverPack };
          saveAvatarToLocalStorage(customAvatarPack.value);
        }
      }
    } catch (e) {
      console.info('Fetch avatar dari dev server dilewati:', e);
    }
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
    selectedPreviewFrame.value = frameKey;
    saveAvatarToLocalStorage(customAvatarPack.value);

    // Simpan ke disk ~/.config/aoi/avatar/ via Tauri IPC
    try {
      await invoke('save_avatar_sprite', { frameName: frameKey, dataUrl });
    } catch {
      // Jika di web browser, simpan ke dev server API agar langsung tersinkron ke disk!
      try {
        await fetch('/api/avatar-sprite', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ frameName: frameKey, dataUrl }),
        });
      } catch (err) {
        console.warn('Simpan sprite ke server dev dilewati:', err);
      }
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
    selectedPreviewFrame.value = 'idle_open';
    saveAvatarToLocalStorage(customAvatarPack.value);

    try {
      await invoke('save_avatar_pack', { pack: customAvatarPack.value });
    } catch {
      try {
        await fetch('/api/avatar-pack', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(customAvatarPack.value),
        });
      } catch (err) {
        console.warn('Simpan batch pack ke server dev dilewati:', err);
      }
    }
  };
  reader.readAsDataURL(file);
};

const removeSingleSprite = async (frameKey: string) => {
  customAvatarPack.value[frameKey] = '';
  saveAvatarToLocalStorage(customAvatarPack.value);
  try {
    await invoke('save_avatar_pack', { pack: customAvatarPack.value });
  } catch {
    try {
      await fetch('/api/avatar-pack', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(customAvatarPack.value),
      });
    } catch (err) {
      console.warn('Update pack ke server dev dilewati:', err);
    }
  }
};

const resetToBuiltInAvatar = async () => {
  for (const item of avatarFrameList) {
    customAvatarPack.value[item.key] = '';
  }
  selectedPreviewFrame.value = 'idle_open';
  saveAvatarToLocalStorage(customAvatarPack.value);
  try {
    await invoke('reset_avatar_pack');
  } catch {
    try {
      await fetch('/api/avatar-reset', { method: 'POST' });
    } catch (err) {
      console.warn('Reset avatar ke server dev dilewati:', err);
    }
  }
};

const openSettings = async () => {
  showSettings.value = true;
  try {
    const loaded = await invoke<AppConfig>('get_config');
    if (loaded) {
      configData.value = { ...configData.value, ...loaded };
      if (loaded.avatar_size) {
        avatarSize.value = loaded.avatar_size;
      }
    }
  } catch (e) {
    console.warn('Gagal membaca config:', e);
  }
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

const testVoice = async () => {
  if (isTestingVoice.value) return;
  isTestingVoice.value = true;
  try {
    // Simpan konfigurasi suara terlebih dahulu agar backend menggunakan setting terkini
    await invoke('save_config', { cfg: configData.value });
    await invoke('test_tts_voice', { sampleText: null });
  } catch (e) {
    console.error('Test voice error:', e);
  } finally {
    setTimeout(() => {
      isTestingVoice.value = false;
    }, 2000);
  }
};

// ─────────────────────────────────────────────────────────────────────────────
// Manajemen Preset Avatar
// ─────────────────────────────────────────────────────────────────────────────

const fetchAvatarPresets = async () => {
  isLoadingPresets.value = true;
  try {
    const list = await invoke<AvatarPresetMeta[]>('list_avatar_presets');
    avatarPresets.value = list;
  } catch (e) {
    console.warn('Gagal memuat preset avatar:', e);
  } finally {
    isLoadingPresets.value = false;
  }
};

const selectAvatarPreset = async (presetId: string) => {
  try {
    const pack = await invoke<Record<string, string>>('switch_avatar_preset', { presetId });
    configData.value.active_avatar_preset = presetId;
    customAvatarPack.value = {
      idle_open: '',
      idle_blink: '',
      listening: '',
      thinking: '',
      speak_closed: '',
      speak_open: '',
      confirm: '',
      error: '',
      ...pack,
    };
    saveAvatarToLocalStorage(customAvatarPack.value);
    await fetchAvatarPresets();
  } catch (e) {
    console.error('Gagal beralih preset:', e);
  }
};

const createNewPreset = async () => {
  if (!newPresetName.value.trim()) return;
  try {
    const meta = await invoke<AvatarPresetMeta>('create_avatar_preset', { name: newPresetName.value.trim() });
    newPresetName.value = '';
    showNewPresetModal.value = false;
    await fetchAvatarPresets();
    await selectAvatarPreset(meta.id);
    activeSettingsTab.value = 'avatar'; // Arahkan langsung ke editor frame untuk upload gambar
  } catch (e) {
    console.error('Gagal membuat preset:', e);
  }
};

const deletePreset = async (presetId: string) => {
  if (confirm('Yakin ingin menghapus preset avatar ini? File frame di folder akan dihapus.')) {
    try {
      await invoke('delete_avatar_preset', { presetId });
      await fetchAvatarPresets();
      const pack = await invoke<Record<string, string>>('load_avatar_pack');
      customAvatarPack.value = { ...pack };
      saveAvatarToLocalStorage(customAvatarPack.value);
    } catch (e) {
      console.error('Gagal menghapus preset:', e);
    }
  }
};

const triggerZipUpload = () => {
  zipFileInput.value?.click();
};

const handleZipUpload = async (event: Event) => {
  const target = event.target as HTMLInputElement;
  const file = target.files?.[0];
  if (!file) return;

  const reader = new FileReader();
  reader.onload = async () => {
    try {
      const rawResult = reader.result as string;
      const b64 = rawResult.includes(';base64,') ? rawResult.split(';base64,')[1] : rawResult;
      const presetName = file.name.replace(/\.zip$/i, '');
      const meta = await invoke<AvatarPresetMeta>('import_avatar_preset_zip', {
        name: presetName,
        zipBase64: b64,
      });
      await fetchAvatarPresets();
      await selectAvatarPreset(meta.id);
    } catch (e) {
      alert('Gagal mengimpor file ZIP: ' + e);
    } finally {
      target.value = '';
    }
  };
  reader.readAsDataURL(file);
};

const exportPresetZip = async (presetId: string, name: string) => {
  try {
    const b64 = await invoke<string>('export_avatar_preset_zip', { presetId });
    const link = document.createElement('a');
    link.href = `data:application/zip;base64,${b64}`;
    link.download = `${name.toLowerCase().replace(/\s+/g, '_')}_avatar.zip`;
    link.click();
  } catch (e) {
    alert('Gagal mengekspor ZIP: ' + e);
  }
};

// ─────────────────────────────────────────────────────────────────────────────
// Manajemen Studio Kloning Suara Kustom (Voice Cloning Studio)
// ─────────────────────────────────────────────────────────────────────────────

const fetchClonedVoices = async () => {
  isLoadingClonedVoices.value = true;
  try {
    const list = await invoke<ClonedVoiceMeta[]>('list_cloned_voices');
    clonedVoices.value = list;
  } catch (e) {
    console.warn('Gagal memuat profil suara kloning:', e);
  } finally {
    isLoadingClonedVoices.value = false;
  }
};

const triggerVoiceFileUpload = () => {
  voiceFileInput.value?.click();
};

const handleAudioFileUpload = (event: Event) => {
  const target = event.target as HTMLInputElement;
  const file = target.files?.[0];
  if (!file) return;

  audioSampleFileName.value = file.name;
  const reader = new FileReader();
  reader.onload = () => {
    const raw = reader.result as string;
    const b64 = raw.includes(';base64,') ? raw.split(';base64,')[1] : raw;
    audioSampleBase64.value = b64;
  };
  reader.readAsDataURL(file);
};

const startMicRecording = async () => {
  try {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    recordingChunks = [];
    const mimeType = MediaRecorder.isTypeSupported('audio/webm;codecs=opus')
      ? 'audio/webm;codecs=opus'
      : MediaRecorder.isTypeSupported('audio/ogg;codecs=opus')
      ? 'audio/ogg;codecs=opus'
      : '';
    mediaRecorder = mimeType ? new MediaRecorder(stream, { mimeType }) : new MediaRecorder(stream);

    mediaRecorder.ondataavailable = (e) => {
      if (e.data && e.data.size > 0) {
        recordingChunks.push(e.data);
      }
    };

    mediaRecorder.onstop = () => {
      const blob = new Blob(recordingChunks, { type: mediaRecorder?.mimeType || 'audio/wav' });
      audioSampleFileName.value = `rekaman_${Date.now()}.wav`;
      const reader = new FileReader();
      reader.onload = () => {
        const raw = reader.result as string;
        const b64 = raw.includes(';base64,') ? raw.split(';base64,')[1] : raw;
        audioSampleBase64.value = b64;
      };
      reader.readAsDataURL(blob);
      stream.getTracks().forEach((t) => t.stop());
    };

    mediaRecorder.start();
    isRecordingMic.value = true;
    recordingSeconds.value = 0;
    if (recordingTimer) clearInterval(recordingTimer);
    recordingTimer = window.setInterval(() => {
      recordingSeconds.value += 1;
    }, 1000);
  } catch (e) {
    alert('Tidak dapat mengakses mikrofon: ' + e);
  }
};

const stopMicRecording = () => {
  if (mediaRecorder && isRecordingMic.value) {
    mediaRecorder.stop();
    isRecordingMic.value = false;
    if (recordingTimer) {
      clearInterval(recordingTimer);
      recordingTimer = null;
    }
  }
};

const resetAudioSample = () => {
  if (isRecordingMic.value) {
    stopMicRecording();
  }
  audioSampleBase64.value = '';
  audioSampleFileName.value = '';
  recordingSeconds.value = 0;
};

const submitVoiceClone = async () => {
  if (!newCloneName.value.trim()) {
    alert('Harap masukkan nama untuk profil suara waifu (misal: Megumin, Rem, dll).');
    return;
  }
  if (!audioSampleBase64.value) {
    alert('Harap unggah file audio sampel (WAV/MP3/M4A/OGG) atau rekam suara via mikrofon.');
    return;
  }

  isCloningVoice.value = true;
  try {
    const apiKey = cloneEngine.value === 'elevenlabs'
      ? configData.value.elevenlabs_api_key?.trim()
      : configData.value.fish_audio_api_key?.trim();

    if (!apiKey && cloneEngine.value !== 'custom_server') {
      alert(`API Key untuk ${cloneEngine.value === 'elevenlabs' ? 'ElevenLabs' : 'Fish Audio'} belum diisi di form pengaturan.`);
      isCloningVoice.value = false;
      return;
    }

    const created = await invoke<ClonedVoiceMeta>('clone_voice_from_file', {
      name: newCloneName.value.trim(),
      engine: cloneEngine.value,
      fileBase64: audioSampleBase64.value,
      filename: audioSampleFileName.value || 'sample.wav',
      apiKey: apiKey || null,
    });

    await fetchClonedVoices();
    await selectClonedVoice(created.id);

    // Reset formulir setelah sukses
    newCloneName.value = '';
    audioSampleBase64.value = '';
    audioSampleFileName.value = '';
    recordingSeconds.value = 0;

    alert(`Suara "${created.name}" berhasil dikloning dan siap digunakan!`);
  } catch (e) {
    alert('Gagal mengkloning suara: ' + e);
  } finally {
    isCloningVoice.value = false;
  }
};

const saveManualVoice = async () => {
  if (!newCloneName.value.trim() || !manualVoiceId.value.trim()) {
    alert('Harap isi Nama Suara dan Voice ID.');
    return;
  }
  try {
    const created = await invoke<ClonedVoiceMeta>('save_cloned_voice_manual', {
      name: newCloneName.value.trim(),
      engine: cloneEngine.value,
      remoteVoiceId: manualVoiceId.value.trim(),
    });
    await fetchClonedVoices();
    await selectClonedVoice(created.id);
    newCloneName.value = '';
    manualVoiceId.value = '';
    showManualVoiceModal.value = false;
  } catch (e) {
    alert('Gagal menyimpan suara: ' + e);
  }
};

const selectClonedVoice = async (voiceId: string) => {
  try {
    await invoke('set_active_cloned_voice', { voiceId });
    configData.value.active_cloned_voice_id = voiceId;
    const found = clonedVoices.value.find((v) => v.id === voiceId);
    if (found) {
      configData.value.tts_engine = found.engine;
      if (found.engine === 'elevenlabs') {
        configData.value.elevenlabs_voice_id = found.remote_voice_id;
      }
    }
    await fetchClonedVoices();
  } catch (e) {
    console.error('Gagal memilih suara kloning:', e);
  }
};

const deleteClonedVoice = async (voiceId: string) => {
  if (confirm('Yakin ingin menghapus profil suara kloning ini?')) {
    try {
      await invoke('delete_cloned_voice', { voiceId });
      if (configData.value.active_cloned_voice_id === voiceId) {
        configData.value.active_cloned_voice_id = '';
      }
      await fetchClonedVoices();
    } catch (e) {
      alert('Gagal menghapus suara kloning: ' + e);
    }
  }
};

const playSampleAudio = async (voice: ClonedVoiceMeta) => {
  if (playingVoiceId.value === voice.id) {
    if (sampleAudioPlayer) {
      sampleAudioPlayer.pause();
      sampleAudioPlayer = null;
    }
    playingVoiceId.value = null;
    return;
  }

  try {
    const b64 = await invoke<string>('get_cloned_voice_sample', { voiceId: voice.id });
    if (sampleAudioPlayer) {
      sampleAudioPlayer.pause();
      sampleAudioPlayer = null;
    }
    const audio = new Audio(`data:audio/wav;base64,${b64}`);
    playingVoiceId.value = voice.id;
    sampleAudioPlayer = audio;
    audio.onended = () => {
      playingVoiceId.value = null;
      sampleAudioPlayer = null;
    };
    audio.onerror = () => {
      playingVoiceId.value = null;
      sampleAudioPlayer = null;
    };
    await audio.play();
  } catch (e) {
    alert('Gagal memutar sampel: ' + e);
    playingVoiceId.value = null;
  }
};

// ─────────────────────────────────────────────────────────────────────────────
// Lifecycle
// ─────────────────────────────────────────────────────────────────────────────
const handleStorageEvent = (event: StorageEvent) => {
  if (event.key === AVATAR_STORAGE_KEY && event.newValue) {
    try {
      const updated = JSON.parse(event.newValue);
      customAvatarPack.value = { ...customAvatarPack.value, ...updated };
    } catch {}
  }
};

const handleWindowFocus = () => {
  loadCustomAvatar();
};

const handleWindowBlur = () => {
  if (showMenuBar.value && !showSettings.value && !isSendingChat.value && currentState.value === 'idle') {
    showMenuBar.value = false;
  }
};

onMounted(async () => {
  startBlinkTimer();
  startLipSyncTimer();
  window.addEventListener('keydown', handleGlobalKeydown);
  window.addEventListener('storage', handleStorageEvent);
  window.addEventListener('focus', handleWindowFocus);
  window.addEventListener('blur', handleWindowBlur);

  // Selalu muat avatar, preset, dan suara kloning terlebih dahulu
  await loadCustomAvatar();
  await fetchAvatarPresets();
  await fetchClonedVoices();

  try {
    const loadedConfig = await invoke<AppConfig>('get_config');
    if (loadedConfig) {
      configData.value = { ...configData.value, ...loadedConfig };
      if (loadedConfig.avatar_size) {
        avatarSize.value = loadedConfig.avatar_size;
      }
    }
  } catch (e) {
    console.info('Config awal memakai default atau offline:', e);
  }

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
      if (event.payload.state !== 'idle') {
        showMenuBar.value = true;
      }
    });

    unlistenLipSync = await listen<LipSyncPayload>('lip-sync-update', (event) => {
      lastLipSyncEventTime = Date.now();
      isMouthOpen.value = event.payload.is_mouth_open;
    });
  } catch {
    console.info('Mode preview web — tanpa koneksi Tauri backend.');
  }
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleGlobalKeydown);
  window.removeEventListener('storage', handleStorageEvent);
  window.removeEventListener('focus', handleWindowFocus);
  window.removeEventListener('blur', handleWindowBlur);
  if (blinkInterval) clearInterval(blinkInterval);
  if (lipSyncInterval) clearInterval(lipSyncInterval);
  if (recordingTimer) clearInterval(recordingTimer);
  if (sampleAudioPlayer) sampleAudioPlayer.pause();
  if (unlistenEvent) unlistenEvent();
  if (unlistenLipSync) unlistenLipSync();
});
</script>

<template>
  <div
    class="w-full h-full overflow-hidden flex flex-col justify-end items-center transition-opacity duration-200 font-sans select-none text-zinc-100"
    :class="{
      'opacity-35 hover:opacity-100': ghostMode,
      'max-w-[360px] max-h-[500px] p-2': showMenuBar || showSettings,
      'p-1': !showMenuBar && !showSettings
    }"
    @contextmenu.prevent="showMenuBar = true; showQuickAvatarMenu = !showQuickAvatarMenu"
  >
    <!-- ══════════════════════════════════════════════ -->
    <!-- MODE MINIMIZE: Pill Mengambang                 -->
    <!-- ══════════════════════════════════════════════ -->
    <div
      v-if="isMinimized"
      @mousedown="startDrag"
      class="flex items-center gap-2 bg-zinc-950/95 backdrop-blur-md border border-zinc-800 px-3.5 py-1.5 rounded-full shadow-xl cursor-move hover:border-zinc-700 transition"
    >
      <span class="w-2 h-2 rounded-full animate-pulse shadow-sm" :class="stateBadgeStyle.dot" />
      <span class="text-[11px] font-medium text-zinc-300 tracking-wide">{{ stateLabel }}</span>
      <button
        @click.stop="toggleMinimize"
        @mousedown.stop
        class="ml-1 text-[10px] text-zinc-400 hover:text-zinc-100 font-semibold px-2 py-0.5 rounded bg-zinc-850 hover:bg-zinc-800 transition"
      >
        Buka
      </button>
    </div>

    <!-- ══════════════════════════════════════════════ -->
    <!-- MODE UTAMA                                     -->
    <!-- ══════════════════════════════════════════════ -->
    <div v-else class="w-full h-full flex flex-col items-center justify-end gap-1.5 overflow-hidden">

      <!-- ════════════════════════════════════════════ -->
      <!-- MODAL PENGATURAN STUDIO                       -->
      <!-- ════════════════════════════════════════════ -->
      <div
        v-if="showSettings"
        class="w-full h-[460px] max-h-[460px] bg-zinc-950/95 backdrop-blur-xl border border-zinc-800 rounded-xl shadow-2xl flex flex-col text-zinc-100 overflow-hidden animate-fade-in"
      >
        <!-- Header Studio -->
        <div class="flex-shrink-0 px-3 py-2 border-b border-zinc-800/80 flex items-center justify-between bg-zinc-900/40">
          <div class="flex items-center gap-2">
            <svg width="16" height="16" class="w-4 h-4 text-zinc-400 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
              <path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
            </svg>
            <div>
              <div class="text-[11px] font-semibold text-zinc-200">Pengaturan Asisten</div>
              <div class="text-[8.5px] text-zinc-500">Karakter, model LLM, audio & sprite</div>
            </div>
          </div>
          <button
            @click.stop="showSettings = false"
            @mousedown.stop
            title="Tutup Pengaturan (Esc)"
            class="text-zinc-400 hover:text-zinc-100 p-1.5 rounded-md hover:bg-zinc-800/80 transition shrink-0"
          >
            <svg width="14" height="14" class="w-3.5 h-3.5 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        <!-- Segmented Tab Bar -->
        <div class="flex-shrink-0 px-2 py-1.5 flex items-center gap-1 border-b border-zinc-850 bg-zinc-900/20">
          <button
            @click="activeSettingsTab = 'persona'"
            class="flex-1 text-[9.5px] font-medium py-1 rounded-md transition flex items-center justify-center gap-1 border"
            :class="
              activeSettingsTab === 'persona'
                ? 'bg-zinc-800 text-zinc-100 border-zinc-700 shadow-sm'
                : 'text-zinc-400 hover:text-zinc-200 bg-transparent border-transparent hover:bg-zinc-850'
            "
          >
            <svg width="12" height="12" class="w-3 h-3 text-zinc-400 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
            </svg>
            Karakter
          </button>
          <button
            @click="activeSettingsTab = 'ai'"
            class="flex-1 text-[9.5px] font-medium py-1 rounded-md transition flex items-center justify-center gap-1 border"
            :class="
              activeSettingsTab === 'ai'
                ? 'bg-zinc-800 text-zinc-100 border-zinc-700 shadow-sm'
                : 'text-zinc-400 hover:text-zinc-200 bg-transparent border-transparent hover:bg-zinc-850'
            "
          >
            <svg width="12" height="12" class="w-3 h-3 text-zinc-400 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <rect x="4" y="4" width="16" height="16" rx="2" />
              <rect x="9" y="9" width="6" height="6" />
              <path d="M9 1v3M15 1v3M9 20v3M15 20v3M20 9h3M20 14h3M1 9h3M1 14h3" />
            </svg>
            Model AI
          </button>
          <button
            @click="activeSettingsTab = 'voice'"
            class="flex-1 text-[9.5px] font-medium py-1 rounded-md transition flex items-center justify-center gap-1 border"
            :class="
              activeSettingsTab === 'voice'
                ? 'bg-zinc-800 text-zinc-100 border-zinc-700 shadow-sm'
                : 'text-zinc-400 hover:text-zinc-200 bg-transparent border-transparent hover:bg-zinc-850'
            "
          >
            <svg width="12" height="12" class="w-3 h-3 text-zinc-400 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M19 11a7 7 0 01-7 7m0 0a7 7 0 01-7-7m7 7v4m0 0H8m4 0h4m-4-8a3 3 0 01-3-3V5a3 3 0 116 0v6a3 3 0 01-3 3z" />
            </svg>
            Audio
          </button>
          <button
            @click="activeSettingsTab = 'presets'"
            class="flex-1 text-[9.5px] font-medium py-1 rounded-md transition flex items-center justify-center gap-1 border"
            :class="
              activeSettingsTab === 'presets'
                ? 'bg-zinc-800 text-zinc-100 border-zinc-700 shadow-sm'
                : 'text-zinc-400 hover:text-zinc-200 bg-transparent border-transparent hover:bg-zinc-850'
            "
          >
            <svg width="12" height="12" class="w-3 h-3 text-zinc-400 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <rect x="2" y="3" width="20" height="14" rx="2" stroke-linecap="round" stroke-linejoin="round" />
              <line x1="8" y1="21" x2="16" y2="21" />
              <line x1="12" y1="17" x2="12" y2="21" />
            </svg>
            Preset
          </button>
          <button
            @click="activeSettingsTab = 'avatar'"
            class="flex-1 text-[9.5px] font-medium py-1 rounded-md transition flex items-center justify-center gap-1 border"
            :class="
              activeSettingsTab === 'avatar'
                ? 'bg-zinc-800 text-zinc-100 border-zinc-700 shadow-sm'
                : 'text-zinc-400 hover:text-zinc-200 bg-transparent border-transparent hover:bg-zinc-850'
            "
          >
            <svg width="12" height="12" class="w-3 h-3 text-zinc-400 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <rect x="3" y="3" width="18" height="18" rx="2" stroke-linecap="round" stroke-linejoin="round"/>
              <circle cx="8.5" cy="8.5" r="1.5" />
              <polyline points="21 15 16 10 5 21" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
            Sprite
          </button>
        </div>

        <!-- ── TAB 0: Karakter & Kepribadian Bebas ── -->
        <div
          v-if="activeSettingsTab === 'persona'"
          id="tab-persona-scroll"
          @wheel.passive="handleWheelScroll"
          class="flex-1 min-h-0 overflow-y-auto p-2.5 space-y-2.5 text-[10px] custom-scrollbar"
        >
          <!-- Nama Asisten & Panggilan User -->
          <div class="grid grid-cols-2 gap-2">
            <div>
              <label class="block text-zinc-400 font-medium mb-1 text-[9.5px]">Nama Asisten</label>
              <input
                v-model="configData.assistant_name"
                placeholder="Contoh: Aoi, Lumi, Kurisu"
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-zinc-400 rounded-lg px-2.5 py-1.5 text-zinc-100 outline-none transition text-[10px]"
              />
            </div>
            <div>
              <label class="block text-zinc-400 font-medium mb-1 text-[9.5px]">Panggilan Kamu</label>
              <input
                v-model="configData.user_name"
                placeholder="Contoh: Kak, Master, Sensei"
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-zinc-400 rounded-lg px-2.5 py-1.5 text-zinc-100 outline-none transition text-[10px]"
              />
            </div>
          </div>

          <!-- Input Teks Kepribadian Bebas Sesuai Keinginan User -->
          <div class="space-y-1">
            <div class="flex items-center justify-between">
              <label class="block text-zinc-300 font-medium text-[9.5px]">
                Deskripsi Kepribadian & Watak (Teks Bebas)
              </label>
              <span class="text-[8px] text-zinc-500">Atur sesuka hati</span>
            </div>
            <textarea
              v-model="configData.custom_personality"
              rows="3"
              placeholder="Tuliskan sifat dan kepribadian karakter sesuka hatimu di sini... Contoh: Ceria, ramah, suka bercanda ringan, santai, dan selalu sigap membantu di Hyprland."
              class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-zinc-400 rounded-lg p-2 text-zinc-100 outline-none transition text-[9.5px] leading-relaxed custom-scrollbar resize-none"
            />
          </div>

          <!-- Template Cepat (Klik untuk otomatis salin ke teks kepribadian) -->
          <div>
            <div class="text-[9px] font-medium text-zinc-400 mb-1">
              Inspirasi Template Cepat (Klik untuk isi otomatis):
            </div>
            <div class="grid grid-cols-3 gap-1">
              <button
                v-for="tmpl in personalityTemplates"
                :key="tmpl.id"
                type="button"
                @click="applyPersonalityTemplate(tmpl)"
                class="p-1.5 rounded-lg border border-zinc-800 bg-zinc-900/60 hover:bg-zinc-850 hover:border-zinc-700 text-left transition flex flex-col justify-between"
              >
                <div class="text-[9px] font-semibold text-zinc-200 truncate">{{ tmpl.title }}</div>
                <div class="text-[7.5px] text-zinc-400 mt-0.5">{{ tmpl.badge }}</div>
              </button>
            </div>
          </div>

          <!-- Live Sample Quote Card -->
          <div class="bg-zinc-900/40 border border-zinc-800 p-2.5 rounded-xl space-y-1">
            <div class="flex items-center justify-between text-[9px] font-medium text-zinc-400">
              <span>Contoh Gaya Bicara:</span>
              <span class="text-[8px] text-zinc-300 bg-zinc-800/80 px-1.5 py-0.5 rounded">
                {{ configData.assistant_name || 'Aoi' }}
              </span>
            </div>
            <div class="italic text-[9.5px] text-zinc-200 bg-zinc-950/80 p-2 rounded-lg border border-zinc-800/80 leading-relaxed">
              "{{ currentPersonalityQuote }}"
            </div>
            <div class="text-[8px] text-zinc-500 pt-0.5">
              Respon dibatasi 1–3 kalimat santai agar alami dan cepat disuarakan.
            </div>
          </div>
        </div>

        <!-- ── TAB 1: AI Model & Provider ── -->
        <div
          v-else-if="activeSettingsTab === 'ai'"
          id="tab-ai-scroll"
          @wheel.passive="handleWheelScroll"
          class="flex-1 min-h-0 overflow-y-auto p-2.5 space-y-2 text-[10px] custom-scrollbar"
        >
          <div>
            <label class="block text-zinc-400 font-medium mb-1 text-[9.5px]">Pilih Provider LLM</label>
            <div class="grid grid-cols-3 gap-1">
              <button
                v-for="prov in providerOptions"
                :key="prov.id"
                type="button"
                @click="configData.llm_provider = prov.id"
                class="py-1.5 px-1 rounded-lg border text-center transition flex flex-col items-center justify-center gap-0.5"
                :class="
                  configData.llm_provider === prov.id
                    ? 'bg-zinc-800 border-zinc-500 text-zinc-100 shadow-sm'
                    : 'bg-zinc-900/80 border-zinc-800 text-zinc-400 hover:text-zinc-200 hover:border-zinc-700'
                "
              >
                <span class="text-[9.5px] font-semibold">{{ prov.name }}</span>
                <span class="text-[7.5px] text-zinc-500 truncate max-w-full">{{ prov.desc }}</span>
              </button>
            </div>
          </div>

          <div>
            <label class="block text-zinc-400 font-medium mb-0.5 text-[9.5px]">Nama Model</label>
            <input
              v-model="configData.model_name"
              placeholder="e.g. gemini-2.0-flash / llama-3.3-70b / gpt-4o-mini"
              class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-zinc-400 rounded-lg px-2.5 py-1.5 text-zinc-100 outline-none transition text-[10px]"
            />
          </div>

          <!-- Input spesifik provider -->
          <div v-if="configData.llm_provider === 'gemini'">
            <label class="block text-zinc-400 font-medium mb-0.5 text-[9.5px]">Google Gemini API Key</label>
            <input
              v-model="configData.gemini_api_key"
              type="password"
              placeholder="AIzaSy..."
              class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-zinc-400 rounded-lg px-2.5 py-1.5 text-zinc-100 outline-none transition text-[10px]"
            />
          </div>

          <div v-if="configData.llm_provider === 'groq'">
            <label class="block text-zinc-400 font-medium mb-0.5 text-[9.5px]">Groq API Key</label>
            <input
              v-model="configData.groq_api_key"
              type="password"
              placeholder="gsk_..."
              class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-zinc-400 rounded-lg px-2.5 py-1.5 text-zinc-100 outline-none transition text-[10px]"
            />
            <span class="text-[8px] text-zinc-500 mt-0.5 block">Juga digunakan untuk transkripsi Whisper Cloud super-cepat.</span>
          </div>

          <div v-if="configData.llm_provider === 'openai'">
            <label class="block text-zinc-400 font-medium mb-0.5 text-[9.5px]">OpenAI API Key</label>
            <input
              v-model="configData.openai_api_key"
              type="password"
              placeholder="sk-..."
              class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-zinc-400 rounded-lg px-2.5 py-1.5 text-zinc-100 outline-none transition text-[10px]"
            />
          </div>

          <div v-if="configData.llm_provider === 'openrouter'">
            <label class="block text-zinc-400 font-medium mb-0.5 text-[9.5px]">OpenRouter API Key</label>
            <input
              v-model="configData.openrouter_api_key"
              type="password"
              placeholder="sk-or-v1-..."
              class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-zinc-400 rounded-lg px-2.5 py-1.5 text-zinc-100 outline-none transition text-[10px]"
            />
          </div>

          <div v-if="configData.llm_provider === 'ollama'">
            <label class="block text-zinc-400 font-medium mb-0.5 text-[9.5px]">Ollama Endpoint URL</label>
            <input
              v-model="configData.ollama_base_url"
              placeholder="http://localhost:11434"
              class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-zinc-400 rounded-lg px-2.5 py-1.5 text-zinc-100 outline-none transition text-[10px]"
            />
          </div>

          <div v-if="configData.llm_provider === 'custom'" class="space-y-1.5">
            <div>
              <label class="block text-zinc-400 font-medium mb-0.5 text-[9.5px]">Custom API Endpoint URL</label>
              <input
                v-model="configData.custom_api_endpoint"
                placeholder="http://127.0.0.1:20128/v1"
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-zinc-400 rounded-lg px-2.5 py-1.5 text-zinc-100 outline-none transition text-[10px]"
              />
            </div>
            <div>
              <label class="block text-zinc-400 font-medium mb-0.5 text-[9.5px]">Custom API Key (Opsional)</label>
              <input
                v-model="configData.custom_api_key"
                type="password"
                placeholder="sk-... (kosongkan jika lokal)"
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-zinc-400 rounded-lg px-2.5 py-1.5 text-zinc-100 outline-none transition text-[10px]"
              />
            </div>
          </div>
        </div>

        <!-- ── TAB 2: Audio, Engine Suara & Kloning ── -->
        <div
          v-else-if="activeSettingsTab === 'voice'"
          id="tab-voice-scroll"
          @wheel.passive="handleWheelScroll"
          class="flex-1 min-h-0 overflow-y-auto p-2.5 space-y-2.5 text-[10px] custom-scrollbar"
        >
          <!-- Tes Suara Header -->
          <div class="bg-zinc-900/90 border border-zinc-800 p-2.5 rounded-xl flex items-center justify-between shadow-sm">
            <div>
              <div class="font-medium text-zinc-100 text-[10px]">Uji Suara Asisten</div>
              <div class="text-[8px] text-zinc-400">
                Engine: <span class="text-zinc-200 font-semibold">{{ configData.tts_engine?.toUpperCase() || 'EDGE_TTS' }}</span>
              </div>
            </div>
            <button
              @click="testVoice"
              :disabled="isTestingVoice"
              class="bg-zinc-100 hover:bg-white text-zinc-950 disabled:opacity-50 font-semibold text-[9px] px-3 py-1.5 rounded-lg transition shadow flex items-center gap-1.5 active:scale-95"
            >
              <svg width="12" height="12" class="w-3 h-3 text-zinc-950 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" />
                <path d="M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07" />
              </svg>
              <span>{{ isTestingVoice ? 'Memutar...' : 'Tes Suara Sekarang' }}</span>
            </button>
          </div>

          <!-- Pilihan Provider Engine TTS -->
          <div>
            <label class="block text-zinc-400 font-medium mb-1 text-[9.5px]">Pilih Engine Sintesis Suara</label>
            <div class="grid grid-cols-2 gap-1.5">
              <button
                v-for="eng in ttsEngineOptions"
                :key="eng.id"
                type="button"
                @click="configData.tts_engine = eng.id"
                class="p-2 rounded-lg border text-left transition flex flex-col justify-center gap-0.5"
                :class="
                  configData.tts_engine === eng.id
                    ? 'bg-zinc-800 border-zinc-500 text-zinc-100 shadow-sm'
                    : 'bg-zinc-900/80 border-zinc-800 text-zinc-400 hover:text-zinc-200 hover:border-zinc-700'
                "
              >
                <div class="flex items-center justify-between w-full">
                  <span class="text-[9.5px] font-semibold">{{ eng.name }}</span>
                  <span v-if="eng.id === 'edge_tts'" class="text-[7px] text-emerald-400 bg-emerald-950/60 border border-emerald-800/80 px-1 py-0.2 rounded font-mono">Bebas Delay</span>
                  <span v-if="eng.id === 'elevenlabs'" class="text-[7px] text-violet-400 bg-violet-950/60 border border-violet-800/80 px-1 py-0.2 rounded font-mono">Clone Waifu</span>
                </div>
                <span class="text-[8px] text-zinc-500 truncate max-w-full">{{ eng.desc }}</span>
              </button>
            </div>
          </div>

          <!-- PENGATURAN SPESIFIK ENGINE -->

          <!-- 1. OPSI EDGE-TTS -->
          <div v-if="configData.tts_engine === 'edge_tts'" class="space-y-2 bg-zinc-900/40 border border-zinc-800/80 p-2.5 rounded-xl">
            <div class="flex items-center justify-between border-b border-zinc-800/60 pb-1.5">
              <span class="text-[9.5px] font-semibold text-zinc-200">Katalog Vokal Edge-TTS</span>
              <span class="text-[8px] text-zinc-400">Gratis & Alami</span>
            </div>

            <div>
              <label class="block text-zinc-400 font-medium mb-1 text-[9px]">Pilih Karakter Suara</label>
              <div class="grid grid-cols-2 gap-1">
                <button
                  v-for="v in edgeVoicePresets"
                  :key="v.id"
                  type="button"
                  @click="configData.edge_tts_voice = v.id"
                  class="p-1.5 rounded-md border text-left transition flex items-center justify-between"
                  :class="
                    configData.edge_tts_voice === v.id
                      ? 'bg-zinc-800 border-zinc-500 text-zinc-100 font-medium'
                      : 'bg-zinc-900/60 border-zinc-800/80 text-zinc-400 hover:text-zinc-200 hover:border-zinc-700'
                  "
                >
                  <span class="text-[8.5px] truncate">{{ v.name }}</span>
                  <span class="text-[7.5px] text-zinc-500 font-mono ml-1 shrink-0">[{{ v.lang }}]</span>
                </button>
              </div>
            </div>

            <div class="grid grid-cols-2 gap-2 pt-1">
              <div>
                <label class="block text-zinc-400 font-medium mb-0.5 text-[9px]">Tempo Suara</label>
                <select
                  v-model="configData.edge_tts_rate"
                  class="w-full bg-zinc-900 border border-zinc-700/80 rounded-lg px-2 py-1 text-zinc-100 text-[9.5px] outline-none"
                >
                  <option value="+0%">Normal (+0%)</option>
                  <option value="+10%">Ceria / Agak Cepat (+10% • Rekomendasi)</option>
                  <option value="+20%">Cepat (+20%)</option>
                  <option value="-10%">Tenang / Lambat (-10%)</option>
                </select>
              </div>
              <div>
                <label class="block text-zinc-400 font-medium mb-0.5 text-[9px]">Kode Vokal Kustom</label>
                <input
                  v-model="configData.edge_tts_voice"
                  placeholder="e.g. id-ID-GadisNeural"
                  class="w-full bg-zinc-900 border border-zinc-700/80 rounded-lg px-2 py-1 text-zinc-100 text-[9.5px] outline-none"
                />
              </div>
            </div>
          </div>

          <!-- 2. OPSI FISH AUDIO ZERO-SHOT CLONING -->
          <div v-else-if="configData.tts_engine === 'fish_audio'" class="space-y-2 bg-zinc-900/40 border border-zinc-800/80 p-2.5 rounded-xl">
            <div class="flex items-center justify-between border-b border-zinc-800/60 pb-1.5">
              <span class="text-[9.5px] font-semibold text-zinc-200">Fish Audio (Zero-Shot Waifu Voice)</span>
              <span class="text-[8px] text-amber-400 font-mono">Model SOTA Zero-Shot</span>
            </div>
            <div>
              <label class="block text-zinc-400 font-medium mb-0.5 text-[9px]">Fish Audio API Key</label>
              <input
                v-model="configData.fish_audio_api_key"
                type="password"
                placeholder="fsh_..."
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-zinc-400 rounded-lg px-2.5 py-1.5 text-zinc-100 outline-none transition text-[10px]"
              />
            </div>
            <div class="text-[8px] text-zinc-400 bg-zinc-950/80 p-2 rounded-lg border border-zinc-800/80 leading-relaxed">
              💡 <b>Kelebihan Fish Audio:</b> Kloning vokal waifu anime instan dengan sampel audio 10-30 detik. Akun gratis &amp; API Key bisa didapatkan langsung di <span class="text-zinc-200 underline">fish.audio</span>.
            </div>
          </div>

          <!-- 3. OPSI ELEVENLABS VOICE CLONING -->
          <div v-else-if="configData.tts_engine === 'elevenlabs'" class="space-y-2 bg-zinc-900/40 border border-zinc-800/80 p-2.5 rounded-xl">
            <div class="flex items-center justify-between border-b border-zinc-800/60 pb-1.5">
              <span class="text-[9.5px] font-semibold text-zinc-200">Kloning Suara ElevenLabs</span>
              <span class="text-[8px] text-violet-400">Model Multilingual v2</span>
            </div>
            <div>
              <label class="block text-zinc-400 font-medium mb-0.5 text-[9px]">ElevenLabs API Key</label>
              <input
                v-model="configData.elevenlabs_api_key"
                type="password"
                placeholder="xi-..."
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-zinc-400 rounded-lg px-2.5 py-1.5 text-zinc-100 outline-none transition text-[10px]"
              />
            </div>
            <div>
              <label class="block text-zinc-400 font-medium mb-0.5 text-[9px]">Voice ID Hasil Kloning</label>
              <input
                v-model="configData.elevenlabs_voice_id"
                placeholder="e.g. 21m00Tcm4TlvDq8ikWAM"
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-zinc-400 rounded-lg px-2.5 py-1.5 text-zinc-100 outline-none transition text-[10px]"
              />
            </div>
            <div class="text-[8px] text-zinc-400 bg-zinc-950/80 p-2 rounded-lg border border-zinc-800/80 leading-relaxed">
              💡 <b>Cara Kloning Suara:</b> Buka <span class="text-zinc-200 underline">elevenlabs.io</span> &gt; <b>VoiceLab</b> &gt; <b>Add Generative or Cloned Voice</b> &gt; Unggah sampel audio 1 menit dari waifu/karakter yang diinginkan &gt; Copy <b>Voice ID</b> ke input di atas.
            </div>
          </div>

          <!-- 4. OPSI OPENAI / CUSTOM SPEECH API -->
          <div v-else-if="configData.tts_engine === 'openai_speech'" class="space-y-2 bg-zinc-900/40 border border-zinc-800/80 p-2.5 rounded-xl">
            <div class="flex items-center justify-between border-b border-zinc-800/60 pb-1.5">
              <span class="text-[9.5px] font-semibold text-zinc-200">OpenAI / Custom Speech API</span>
              <span class="text-[8px] text-zinc-400">/v1/audio/speech</span>
            </div>
            <div>
              <label class="block text-zinc-400 font-medium mb-0.5 text-[9px]">Endpoint URL</label>
              <input
                v-model="configData.openai_speech_endpoint"
                placeholder="https://api.openai.com/v1/audio/speech atau http://localhost:8000/v1/audio/speech"
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-zinc-400 rounded-lg px-2.5 py-1.5 text-zinc-100 outline-none transition text-[10px]"
              />
            </div>
            <div class="grid grid-cols-2 gap-2">
              <div>
                <label class="block text-zinc-400 font-medium mb-0.5 text-[9px]">API Key (Opsional)</label>
                <input
                  v-model="configData.openai_speech_key"
                  type="password"
                  placeholder="sk-... (kosongkan jika server lokal)"
                  class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-zinc-400 rounded-lg px-2.5 py-1.5 text-zinc-100 outline-none transition text-[10px]"
                />
              </div>
              <div>
                <label class="block text-zinc-400 font-medium mb-0.5 text-[9px]">Voice Name</label>
                <input
                  v-model="configData.openai_speech_voice"
                  placeholder="nova / shimmer / alloy"
                  class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-zinc-400 rounded-lg px-2.5 py-1.5 text-zinc-100 outline-none transition text-[10px]"
                />
              </div>
            </div>
          </div>

          <!-- 5. OPSI PIPER OFFLINE -->
          <div v-else-if="configData.tts_engine === 'piper'" class="space-y-2 bg-zinc-900/40 border border-zinc-800/80 p-2.5 rounded-xl">
            <div>
              <label class="block text-zinc-400 font-medium mb-0.5 text-[9.5px]">Piper TTS Model Path (.onnx)</label>
              <input
                v-model="configData.tts_model_path"
                placeholder="/home/user/.local/share/piper/model.onnx"
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-zinc-400 rounded-lg px-2.5 py-1.5 text-zinc-100 outline-none transition text-[10px]"
              />
              <span class="text-[7.5px] text-zinc-500 mt-0.5 block">Default tersimpan di ~/.local/share/piper/</span>
            </div>
          </div>

          <!-- ── STUDIO KLONING SUARA KUSTOM ── -->
          <div class="space-y-2.5 bg-gradient-to-b from-zinc-900/80 to-zinc-950/80 border border-zinc-700/60 p-2.5 rounded-xl shadow-lg">
            <div class="flex items-center justify-between border-b border-zinc-800 pb-1.5">
              <div class="flex items-center gap-1.5">
                <span class="w-2 h-2 rounded-full bg-amber-400 animate-pulse"></span>
                <span class="text-[10px] font-semibold text-zinc-100">Studio Kloning Suara</span>
              </div>
              <button
                type="button"
                @click="showManualVoiceModal = !showManualVoiceModal"
                class="text-[8px] text-zinc-400 hover:text-zinc-200 underline"
              >
                {{ showManualVoiceModal ? 'Tutup Input ID' : '+ Input Voice ID Manual' }}
              </button>
            </div>

            <!-- Form Manual Voice ID jika dibuka -->
            <div v-if="showManualVoiceModal" class="p-2 bg-zinc-900/90 border border-zinc-700 rounded-lg space-y-2">
              <div class="text-[8.5px] font-medium text-zinc-300">Daftarkan Voice ID yang sudah ada:</div>
              <div class="grid grid-cols-2 gap-1.5">
                <div>
                  <label class="block text-zinc-400 text-[8px] mb-0.5">Nama Karakter</label>
                  <input
                    v-model="newCloneName"
                    placeholder="e.g. Megumin"
                    class="w-full bg-zinc-950 border border-zinc-700 rounded px-2 py-1 text-zinc-100 text-[9px] outline-none"
                  />
                </div>
                <div>
                  <label class="block text-zinc-400 text-[8px] mb-0.5">Engine</label>
                  <select
                    v-model="cloneEngine"
                    class="w-full bg-zinc-950 border border-zinc-700 rounded px-2 py-1 text-zinc-100 text-[9px] outline-none"
                  >
                    <option value="fish_audio">Fish Audio</option>
                    <option value="elevenlabs">ElevenLabs</option>
                  </select>
                </div>
              </div>
              <div>
                <label class="block text-zinc-400 text-[8px] mb-0.5">Voice ID / Reference Model ID</label>
                <input
                  v-model="manualVoiceId"
                  placeholder="e.g. 21m00Tcm4TlvDq8ikWAM atau model_id fish.audio"
                  class="w-full bg-zinc-950 border border-zinc-700 rounded px-2 py-1 text-zinc-100 text-[9px] outline-none"
                />
              </div>
              <button
                type="button"
                @click="saveManualVoice"
                class="w-full bg-zinc-100 hover:bg-white text-zinc-950 font-semibold py-1 rounded text-[9px] transition"
              >
                Simpan Profil Suara
              </button>
            </div>

            <!-- Form Kloning Baru dari File / Mic -->
            <div v-else class="space-y-2">
              <div class="grid grid-cols-2 gap-1.5">
                <div>
                  <label class="block text-zinc-400 text-[8.5px] mb-0.5 font-medium">Nama Karakter / Waifu</label>
                  <input
                    v-model="newCloneName"
                    placeholder="e.g. Megumin / Rem"
                    class="w-full bg-zinc-950 border border-zinc-700/80 focus:border-zinc-400 rounded-lg px-2 py-1 text-zinc-100 text-[9px] outline-none"
                  />
                </div>
                <div>
                  <label class="block text-zinc-400 text-[8.5px] mb-0.5 font-medium">Engine Kloning</label>
                  <select
                    v-model="cloneEngine"
                    class="w-full bg-zinc-950 border border-zinc-700/80 focus:border-zinc-400 rounded-lg px-2 py-1 text-zinc-100 text-[9px] outline-none"
                  >
                    <option value="fish_audio">Fish Audio (Zero-Shot Rekomendasi)</option>
                    <option value="elevenlabs">ElevenLabs VoiceLab</option>
                  </select>
                </div>
              </div>

              <!-- Input Sampel Audio: Unggah atau Rekam -->
              <div>
                <label class="block text-zinc-400 text-[8.5px] mb-1 font-medium">Sampel Suara Referensi (10 - 60 detik)</label>
                
                <input
                  ref="voiceFileInput"
                  type="file"
                  accept=".wav,.mp3,.ogg,.m4a,.flac"
                  @change="handleAudioFileUpload"
                  class="hidden"
                />

                <div class="grid grid-cols-2 gap-1.5">
                  <!-- Tombol Unggah File Audio -->
                  <button
                    type="button"
                    @click="triggerVoiceFileUpload"
                    class="p-2 border border-dashed rounded-lg flex items-center justify-center gap-1.5 transition text-[8.5px]"
                    :class="audioSampleBase64 && !isRecordingMic ? 'bg-zinc-800 border-zinc-500 text-zinc-100' : 'bg-zinc-950/60 border-zinc-800 text-zinc-400 hover:text-zinc-200 hover:border-zinc-700'"
                  >
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                      <polyline points="17 8 12 3 7 8" />
                      <line x1="12" y1="3" x2="12" y2="15" />
                    </svg>
                    <span class="truncate">{{ audioSampleFileName ? audioSampleFileName : 'Unggah File Audio' }}</span>
                  </button>

                  <!-- Tombol Rekam Mikrofon -->
                  <button
                    type="button"
                    @click="isRecordingMic ? stopMicRecording() : startMicRecording()"
                    class="p-2 border rounded-lg flex items-center justify-center gap-1.5 transition text-[8.5px]"
                    :class="isRecordingMic ? 'bg-rose-950/80 border-rose-500 text-rose-200 animate-pulse' : 'bg-zinc-950/60 border-zinc-800 text-zinc-400 hover:text-zinc-200 hover:border-zinc-700'"
                  >
                    <span v-if="isRecordingMic" class="w-2 h-2 rounded-full bg-rose-500"></span>
                    <svg v-else width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z" />
                      <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
                      <line x1="12" y1="19" x2="12" y2="22" />
                    </svg>
                    <span>{{ isRecordingMic ? `Merekam... (${recordingSeconds}s)` : 'Rekam Mikrofon' }}</span>
                  </button>
                </div>

                <!-- Status File Sampel -->
                <div v-if="audioSampleBase64" class="mt-1 flex items-center justify-between bg-zinc-950/80 px-2 py-1 rounded text-[8px] text-zinc-300 border border-zinc-800">
                  <span class="truncate">📁 {{ audioSampleFileName }}</span>
                  <button type="button" @click="resetAudioSample" class="text-rose-400 hover:text-rose-300 ml-1">Batal</button>
                </div>
              </div>

              <!-- Tombol Eksekusi Kloning -->
              <button
                type="button"
                @click="submitVoiceClone"
                :disabled="isCloningVoice || (!audioSampleBase64 && !newCloneName)"
                class="w-full bg-gradient-to-r from-amber-400 to-amber-500 hover:from-amber-300 hover:to-amber-400 text-zinc-950 font-bold py-1.5 rounded-lg text-[9.5px] transition shadow disabled:opacity-50 flex items-center justify-center gap-1.5 active:scale-98"
              >
                <svg v-if="isCloningVoice" class="animate-spin w-3 h-3 text-zinc-950" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                  <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                  <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
                </svg>
                <svg v-else width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="m12 3-1.9 5.8a2 2 0 0 1-1.3 1.3L3 12l5.8 1.9a2 2 0 0 1 1.3 1.3L12 21l1.9-5.8a2 2 0 0 1 1.3-1.3L21 12l-5.8-1.9a2 2 0 0 1-1.3-1.3L12 3z"/>
                </svg>
                <span>{{ isCloningVoice ? 'Sedang Memproses Kloning Suara...' : 'Mulai Kloning Suara' }}</span>
              </button>
            </div>
          </div>

          <!-- ── DAFTAR SUARA KLONING SAYA ── -->
          <div class="space-y-1.5">
            <div class="flex items-center justify-between">
              <label class="block text-zinc-400 font-medium text-[9.5px]">Koleksi Suara Kloning Saya ({{ clonedVoices.length }})</label>
              <button type="button" @click="fetchClonedVoices" class="text-[8px] text-zinc-400 hover:text-zinc-200">Segarkan</button>
            </div>

            <div v-if="clonedVoices.length === 0" class="text-[8.5px] text-zinc-500 bg-zinc-900/40 p-2.5 rounded-xl border border-zinc-800/80 text-center leading-relaxed">
              Belum ada suara kloning yang tersimpan. Gunakan Studio Kloning di atas untuk membuat suara waifu kustom kamu!
            </div>

            <div v-else class="space-y-1">
              <div
                v-for="voice in clonedVoices"
                :key="voice.id"
                class="p-2 rounded-xl border transition flex items-center justify-between"
                :class="
                  configData.active_cloned_voice_id === voice.id
                    ? 'bg-zinc-800/90 border-amber-500/80 shadow-sm'
                    : 'bg-zinc-900/60 border-zinc-800 hover:border-zinc-700'
                "
              >
                <div class="flex items-center gap-2 min-w-0">
                  <div
                    class="w-7 h-7 rounded-lg flex items-center justify-center font-bold text-[10px] shrink-0"
                    :class="
                      voice.engine === 'fish_audio'
                        ? 'bg-amber-950/80 border border-amber-700 text-amber-300'
                        : 'bg-violet-950/80 border border-violet-700 text-violet-300'
                    "
                  >
                    {{ voice.name.slice(0, 2).toUpperCase() }}
                  </div>
                  <div class="min-w-0">
                    <div class="flex items-center gap-1.5">
                      <span class="text-[9.5px] font-semibold text-zinc-100 truncate">{{ voice.name }}</span>
                      <span
                        v-if="configData.active_cloned_voice_id === voice.id"
                        class="text-[7px] text-amber-300 bg-amber-950/80 border border-amber-800 px-1 py-0.2 rounded font-mono"
                      >
                        ✓ Aktif
                      </span>
                    </div>
                    <div class="text-[7.5px] text-zinc-500 truncate">
                      Engine: {{ voice.engine === 'fish_audio' ? 'Fish Audio' : 'ElevenLabs' }} • ID: {{ voice.remote_voice_id.slice(0, 10) }}...
                    </div>
                  </div>
                </div>

                <div class="flex items-center gap-1 shrink-0">
                  <!-- Putar Sampel Audio -->
                  <button
                    v-if="voice.sample_filename"
                    type="button"
                    @click="playSampleAudio(voice)"
                    class="p-1 rounded-md bg-zinc-800 hover:bg-zinc-700 text-zinc-300 transition text-[8px] flex items-center gap-0.5"
                    title="Dengar sampel rekaman asli"
                  >
                    <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" />
                    </svg>
                    <span>{{ playingVoiceId === voice.id ? 'Pause' : 'Sampel' }}</span>
                  </button>

                  <!-- Tombol Pilih / Aktifkan -->
                  <button
                    v-if="configData.active_cloned_voice_id !== voice.id"
                    type="button"
                    @click="selectClonedVoice(voice.id)"
                    class="px-2 py-1 rounded-md bg-zinc-700 hover:bg-zinc-600 text-zinc-100 transition text-[8.5px] font-medium"
                  >
                    Gunakan
                  </button>

                  <!-- Tombol Hapus -->
                  <button
                    type="button"
                    @click="deleteClonedVoice(voice.id)"
                    class="p-1 rounded-md hover:bg-rose-950/60 text-zinc-500 hover:text-rose-400 transition"
                    title="Hapus profil suara"
                  >
                    <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                    </svg>
                  </button>
                </div>
              </div>
            </div>
          </div>

          <!-- Whisper STT Offline -->
          <div class="border-t border-zinc-800 pt-2">
            <label class="block text-zinc-400 font-medium mb-0.5 text-[9.5px]">Whisper STT Model Path (.bin)</label>
            <input
              v-model="configData.whisper_model_path"
              placeholder="/home/user/.local/share/whisper/ggml-base.bin"
              class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-zinc-400 rounded-lg px-2.5 py-1.5 text-zinc-100 outline-none transition text-[10px]"
            />
            <span class="text-[7.5px] text-zinc-500 mt-0.5 block">Digunakan saat input suara offline (Super+V).</span>
          </div>
        </div>

        <!-- ── TAB BARU: Galeri & Manajemen Preset Avatar ── -->
        <div
          v-else-if="activeSettingsTab === 'presets'"
          id="tab-presets-scroll"
          @wheel.passive="handleWheelScroll"
          class="flex-1 min-h-0 overflow-y-auto p-2.5 space-y-2.5 text-[10px] custom-scrollbar"
        >
          <!-- Toolbar Atas Preset -->
          <div class="flex items-center justify-between gap-1.5 bg-zinc-900/90 p-2 rounded-xl border border-zinc-800 shadow-sm flex-shrink-0">
            <div>
              <div class="font-medium text-zinc-100 text-[10.5px]">Preset Avatar</div>
              <div class="text-[8px] text-zinc-400">Pilih, tambah, atau unggah karakter avatar</div>
            </div>
            <div class="flex items-center gap-1.5">
              <button
                @click="showNewPresetModal = true"
                type="button"
                class="bg-zinc-800 hover:bg-zinc-700 text-zinc-200 border border-zinc-700 font-medium text-[9px] px-2 py-1 rounded-md transition shadow flex items-center gap-1"
              >
                <svg width="11" height="11" class="w-3 h-3 text-zinc-300" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M12 5v14M5 12h14" />
                </svg>
                <span>+ Baru</span>
              </button>
              <button
                @click="triggerZipUpload"
                type="button"
                class="bg-zinc-100 hover:bg-white text-zinc-950 font-semibold text-[9px] px-2.5 py-1 rounded-md transition shadow flex items-center gap-1 active:scale-95"
              >
                <svg width="11" height="11" class="w-3 h-3 text-zinc-950" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" />
                </svg>
                <span>Unggah ZIP</span>
              </button>
              <input
                ref="zipFileInput"
                type="file"
                accept=".zip"
                class="hidden"
                @change="handleZipUpload"
              />
            </div>
          </div>

          <!-- Modal Mini Buat Preset Baru -->
          <div
            v-if="showNewPresetModal"
            class="bg-zinc-900 border border-zinc-700/80 p-2.5 rounded-xl space-y-2 animate-fade-in shadow-xl"
          >
            <div class="flex items-center justify-between">
              <span class="text-[9.5px] font-semibold text-zinc-200">Nama Karakter / Preset Baru</span>
              <button @click="showNewPresetModal = false" class="text-zinc-500 hover:text-zinc-300 text-xs">✕</button>
            </div>
            <div class="flex items-center gap-1.5">
              <input
                v-model="newPresetName"
                placeholder="Contoh: Megumin / Rem / Chibi Cat"
                class="flex-1 bg-zinc-950 border border-zinc-700 rounded-lg px-2.5 py-1.5 text-zinc-100 text-[10px] outline-none"
                @keydown.enter="createNewPreset"
              />
              <button
                @click="createNewPreset"
                class="bg-zinc-100 hover:bg-white text-zinc-950 font-semibold text-[9.5px] px-3 py-1.5 rounded-lg transition shadow"
              >
                Buat
              </button>
            </div>
          </div>

          <!-- Daftar Kartu Preset (Grid) -->
          <div class="grid grid-cols-2 gap-2">
            <div
              v-for="preset in avatarPresets"
              :key="preset.id"
              class="relative bg-zinc-900/60 border rounded-xl p-2.5 flex flex-col justify-between transition hover:border-zinc-700"
              :class="
                configData.active_avatar_preset === preset.id
                  ? 'border-zinc-400 bg-zinc-900/90 shadow-md ring-1 ring-zinc-500/30'
                  : 'border-zinc-800'
              "
            >
              <!-- Preview Gambar Thumbnail -->
              <div class="w-full h-24 bg-zinc-950/80 rounded-lg flex items-center justify-center overflow-hidden border border-zinc-800/60 mb-2 relative group">
                <img
                  v-if="preset.thumbnail"
                  :src="preset.thumbnail"
                  :alt="preset.name"
                  class="w-full h-full object-contain p-1"
                />
                <!-- Thumbnail bawaan SVG jika default -->
                <div v-else class="flex flex-col items-center justify-center text-zinc-500 gap-1">
                  <svg width="28" height="28" class="w-7 h-7 text-zinc-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                    <circle cx="12" cy="12" r="10" />
                    <circle cx="9" cy="10" r="1.5" />
                    <circle cx="15" cy="10" r="1.5" />
                    <path d="M8 15s1.5 2 4 2 4-2 4-2" />
                  </svg>
                  <span class="text-[7.5px] text-zinc-400 font-mono">Anime Vector</span>
                </div>

                <!-- Badge Aktif -->
                <div
                  v-if="configData.active_avatar_preset === preset.id"
                  class="absolute top-1.5 right-1.5 bg-emerald-500 text-zinc-950 font-bold text-[7px] px-1.5 py-0.5 rounded-full shadow"
                >
                  AKTIF
                </div>
              </div>

              <!-- Info Nama & Jumlah Frame -->
              <div class="mb-2">
                <div class="font-semibold text-zinc-100 text-[10px] truncate" :title="preset.name">
                  {{ preset.name }}
                </div>
                <div class="text-[8px] text-zinc-500">
                  {{ preset.id === 'default' ? '8 frame built-in SVG' : `${preset.frame_count}/8 frame terisi` }}
                </div>
              </div>

              <!-- Tombol Aksi Preset -->
              <div class="flex items-center gap-1 pt-1 border-t border-zinc-800/80">
                <button
                  v-if="configData.active_avatar_preset !== preset.id"
                  @click="selectAvatarPreset(preset.id)"
                  class="flex-1 bg-zinc-800 hover:bg-zinc-700 text-zinc-200 text-[8.5px] font-medium py-1 rounded transition text-center shadow-sm"
                >
                  Gunakan
                </button>
                <div
                  v-else
                  class="flex-1 text-[8.5px] text-zinc-400 font-medium py-1 text-center bg-zinc-950/60 rounded border border-zinc-800"
                >
                  Sedang Dipakai
                </div>

                <!-- Tombol Edit Frame -->
                <button
                  @click="selectAvatarPreset(preset.id); activeSettingsTab = 'avatar'"
                  title="Buka Editor Frame Sprite"
                  class="p-1 rounded bg-zinc-850 hover:bg-zinc-750 text-zinc-400 hover:text-zinc-200 transition border border-zinc-750"
                >
                  <svg width="10" height="10" class="w-2.5 h-2.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M12 20h9M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z" />
                  </svg>
                </button>

                <!-- Tombol Ekspor ZIP (Hanya untuk preset kustom) -->
                <button
                  v-if="preset.id !== 'default'"
                  @click="exportPresetZip(preset.id, preset.name)"
                  title="Unduh Preset (.zip)"
                  class="p-1 rounded bg-zinc-850 hover:bg-zinc-750 text-zinc-400 hover:text-zinc-200 transition border border-zinc-750"
                >
                  <svg width="10" height="10" class="w-2.5 h-2.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3" />
                  </svg>
                </button>

                <!-- Tombol Hapus (Hanya untuk preset kustom) -->
                <button
                  v-if="preset.id !== 'default'"
                  @click="deletePreset(preset.id)"
                  title="Hapus Preset"
                  class="p-1 rounded bg-zinc-850 hover:bg-rose-950/80 text-zinc-400 hover:text-rose-300 transition border border-zinc-750 hover:border-rose-900"
                >
                  <svg width="10" height="10" class="w-2.5 h-2.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                  </svg>
                </button>
              </div>
            </div>
          </div>
        </div>

        <!-- ── TAB 3: Sprite Studio & Kustomisasi ── -->
        <div
          v-else-if="activeSettingsTab === 'avatar'"
          id="tab-avatar-scroll"
          tabindex="0"
          @wheel="handleWheelScroll"
          class="flex-1 min-h-0 overflow-y-auto p-2.5 space-y-2 text-[10px] custom-scrollbar outline-none"
        >
          <!-- Toolbar Atas: Toggle Hide/Show Preview + Quick Actions -->
          <div class="flex items-center justify-between gap-1.5 bg-zinc-900/90 p-1.5 rounded-lg border border-zinc-800 flex-shrink-0">
            <!-- Tombol Sembunyikan / Tampilkan Preview -->
            <button
              @click="showSpritePreview = !showSpritePreview"
              type="button"
              class="text-[9px] font-medium px-2 py-1 rounded-md border transition flex items-center gap-1 shadow-sm"
              :class="
                showSpritePreview
                  ? 'bg-zinc-800 border-zinc-600 text-zinc-200'
                  : 'bg-zinc-900 border-zinc-700 text-zinc-300 hover:text-white'
              "
            >
              <svg width="12" height="12" class="w-3 h-3 text-zinc-400 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                <path stroke-linecap="round" stroke-linejoin="round" d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" />
              </svg>
              <span>{{ showSpritePreview ? 'Sembunyikan Preview' : 'Tampilkan Preview' }}</span>
            </button>

            <!-- Quick Action: Status, Batch Upload & Reset -->
            <div class="flex items-center gap-1">
              <span class="text-[7.5px] text-zinc-400 font-mono px-1.5 py-0.5 rounded bg-zinc-800 border border-zinc-700">
                {{ uploadedFrameCount }}/8
              </span>
              <label class="cursor-pointer bg-zinc-800 hover:bg-zinc-750 text-zinc-200 border border-zinc-700 text-[8px] font-medium px-2 py-1 rounded transition flex items-center gap-1">
                Semua
                <input type="file" accept="image/png,image/webp,image/jpeg,image/svg+xml" class="hidden" @change="handleBatchUpload" />
              </label>
              <button
                @click="resetToBuiltInAvatar"
                title="Reset ke karakter anime SVG bawaan"
                class="bg-zinc-900 hover:bg-zinc-850 border border-zinc-800 text-zinc-400 hover:text-zinc-200 text-[8px] font-medium px-2 py-1 rounded transition"
              >
                Reset
              </button>
            </div>
          </div>

          <!-- ── Pengaturan Ukuran Avatar Desktop ── -->
          <div class="bg-zinc-900/90 border border-zinc-800 rounded-lg p-2.5 space-y-2 flex-shrink-0">
            <div class="flex items-center justify-between">
              <div>
                <span class="text-[9.5px] font-semibold text-zinc-200">Ukuran Avatar Desktop</span>
                <p class="text-[8px] text-zinc-500">Rentang 80px s/d 260px (Default: 144px)</p>
              </div>
              <span class="text-[10px] font-mono font-bold px-2 py-0.5 rounded bg-zinc-850 text-indigo-300 border border-zinc-700">
                {{ avatarSize }}px
              </span>
            </div>

            <input
              type="range"
              min="80"
              max="260"
              step="4"
              :value="avatarSize"
              @input="(e) => setAvatarSize(Number((e.target as HTMLInputElement).value))"
              class="w-full accent-indigo-500 cursor-pointer h-1.5 bg-zinc-800 rounded-lg"
            />

            <div class="flex items-center gap-1.5 pt-0.5">
              <button
                type="button"
                @click="setAvatarSize(96)"
                class="flex-1 text-[8px] py-1 rounded border transition"
                :class="avatarSize === 96 ? 'bg-indigo-600/30 border-indigo-500 text-indigo-200 font-semibold' : 'bg-zinc-850 hover:bg-zinc-800 border-zinc-750 text-zinc-400'"
              >
                96px
              </button>
              <button
                type="button"
                @click="setAvatarSize(144)"
                class="flex-1 text-[8px] py-1 rounded border transition"
                :class="avatarSize === 144 ? 'bg-indigo-600/30 border-indigo-500 text-indigo-200 font-semibold' : 'bg-zinc-850 hover:bg-zinc-800 border-zinc-750 text-zinc-400'"
              >
                144px
              </button>
              <button
                type="button"
                @click="setAvatarSize(192)"
                class="flex-1 text-[8px] py-1 rounded border transition"
                :class="avatarSize === 192 ? 'bg-indigo-600/30 border-indigo-500 text-indigo-200 font-semibold' : 'bg-zinc-850 hover:bg-zinc-800 border-zinc-750 text-zinc-400'"
              >
                192px
              </button>
              <button
                type="button"
                @click="setAvatarSize(240)"
                class="flex-1 text-[8px] py-1 rounded border transition"
                :class="avatarSize === 240 ? 'bg-indigo-600/30 border-indigo-500 text-indigo-200 font-semibold' : 'bg-zinc-850 hover:bg-zinc-800 border-zinc-750 text-zinc-400'"
              >
                240px
              </button>
            </div>
            <div class="text-[7.5px] text-zinc-500 leading-tight">
              💡 Tip: Scroll roda mouse langsung di atas avatar untuk zoom cepat kapan saja!
            </div>
          </div>

          <!-- Keterangan Mode Banner -->
          <div
            class="px-2 py-1 rounded-md border text-[8px] flex items-center justify-between transition flex-shrink-0"
            :class="
              showSpritePreview
                ? 'bg-zinc-900 border-zinc-800 text-zinc-300'
                : 'bg-zinc-900/60 border-zinc-800/80 text-zinc-400'
            "
          >
            <span class="font-medium">
              {{ showSpritePreview ? 'Mode Visual Preview' : 'Mode Ringkas (Semua tombol muat di layar)' }}
            </span>
            <span class="text-[7px] text-zinc-500">
              {{ showSpritePreview ? 'Thumbnail gambar tampil' : 'Bebas scroll' }}
            </span>
          </div>

          <!-- Stage Live Preview Mini (Saat showSpritePreview === true) -->
          <div v-if="showSpritePreview" class="space-y-1.5 flex-shrink-0 animate-fade-in">
            <div class="bg-zinc-900 p-2 rounded-xl border border-zinc-800 flex items-center gap-2.5">
              <div class="w-11 h-11 rounded-lg bg-zinc-950 border border-zinc-800 flex items-center justify-center overflow-hidden flex-shrink-0 relative">
                <img
                  v-if="studioPreviewFrame"
                  :src="studioPreviewFrame"
                  class="w-full h-full object-contain"
                  alt="Stage Preview"
                />
                <span v-else class="text-[7.5px] text-zinc-500 font-medium text-center">Default<br />SVG</span>
                <span class="absolute bottom-0 right-0 text-[6px] bg-zinc-800 text-zinc-300 font-mono px-1 rounded-tl">Live</span>
              </div>

              <div class="flex-1 min-w-0">
                <div class="flex items-center justify-between">
                  <span class="font-medium text-zinc-200 text-[9px]">Live Test Emosi</span>
                  <span class="text-[7.5px] text-zinc-500 font-mono">
                    {{ selectedPreviewFrame || previewEmotion }}
                  </span>
                </div>

                <div class="flex items-center gap-0.5 mt-1">
                  <button
                    v-for="em in (['idle', 'speaking', 'thinking', 'listening', 'confirm', 'error'] as AssistantState[])"
                    :key="em"
                    @click="previewEmotion = em; selectedPreviewFrame = ''"
                    class="text-[7px] font-medium px-1.5 py-0.5 rounded transition uppercase"
                    :class="previewEmotion === em && !selectedPreviewFrame ? 'bg-zinc-100 text-zinc-950 font-bold' : 'text-zinc-400 hover:text-zinc-200 bg-zinc-800'"
                  >
                    {{ em.slice(0, 3) }}
                  </button>
                </div>
              </div>
            </div>
          </div>

          <!-- Filter Segment Tab Frame -->
          <div class="flex items-center justify-between bg-zinc-900/60 p-1 rounded-lg border border-zinc-800 flex-shrink-0">
            <span class="text-[8px] text-zinc-500 font-medium uppercase tracking-wider pl-1">Slot:</span>
            <div class="flex items-center gap-1">
              <button
                @click="spriteFilter = 'all'"
                class="text-[7.5px] font-medium px-2 py-0.5 rounded transition"
                :class="spriteFilter === 'all' ? 'bg-zinc-800 text-zinc-100 shadow-sm' : 'text-zinc-400 hover:text-zinc-200'"
              >
                Semua (8)
              </button>
              <button
                @click="spriteFilter = 'idle'"
                class="text-[7.5px] font-medium px-2 py-0.5 rounded transition"
                :class="spriteFilter === 'idle' ? 'bg-zinc-800 text-zinc-100 shadow-sm' : 'text-zinc-400 hover:text-zinc-200'"
              >
                Utama (4)
              </button>
              <button
                @click="spriteFilter = 'emotions'"
                class="text-[7.5px] font-medium px-2 py-0.5 rounded transition"
                :class="spriteFilter === 'emotions' ? 'bg-zinc-800 text-zinc-100 shadow-sm' : 'text-zinc-400 hover:text-zinc-200'"
              >
                Emosi (4)
              </button>
            </div>
          </div>

          <!-- Grid 8 Frame Sprite (2 Mode: Visual Preview vs Ringkas) -->
          <div class="grid grid-cols-2 gap-1.5">
            <div
              v-for="item in filteredAvatarFrames"
              :key="item.key"
              @click="selectedPreviewFrame = item.key"
              class="bg-zinc-900/70 border rounded-lg cursor-pointer transition select-none"
              :class="[
                selectedPreviewFrame === item.key
                  ? 'border-zinc-500 bg-zinc-850 shadow-sm'
                  : 'border-zinc-800 hover:border-zinc-700',
                showSpritePreview ? 'p-1.5 flex items-center gap-2' : 'px-2 py-1.5 flex items-center justify-between gap-1'
              ]"
            >
              <!-- ── MODE 1: PREVIEW GAMBAR AKTIF ── -->
              <template v-if="showSpritePreview">
                <div class="w-7 h-7 rounded bg-zinc-950 border border-zinc-800 flex items-center justify-center overflow-hidden flex-shrink-0">
                  <img
                    v-if="customAvatarPack[item.key]"
                    :src="customAvatarPack[item.key]"
                    class="w-full h-full object-contain"
                  />
                  <span v-else class="text-[9px] text-zinc-600 font-mono">—</span>
                </div>

                <div class="flex-1 min-w-0">
                  <div class="text-[8.5px] font-medium text-zinc-200 truncate leading-tight">{{ item.label }}</div>
                  <div class="text-[7px] text-zinc-500 truncate leading-tight">{{ item.desc }}</div>
                  <div class="flex items-center gap-1 mt-1">
                    <label
                      class="cursor-pointer bg-zinc-800 hover:bg-zinc-700 text-zinc-200 text-[7.5px] font-medium px-1.5 py-0.5 rounded border border-zinc-700 transition"
                      @click.stop
                    >
                      Upload
                      <input type="file" accept="image/png,image/webp,image/jpeg,image/svg+xml" class="hidden" @change="(e) => handleSpriteUpload(e, item.key)" />
                    </label>
                    <button
                      v-if="customAvatarPack[item.key]"
                      @click.stop="removeSingleSprite(item.key)"
                      title="Hapus sprite frame ini"
                      class="text-zinc-500 hover:text-rose-400 text-[8px] px-0.5 font-bold"
                    >
                      ✕
                    </button>
                  </div>
                </div>
              </template>

              <!-- ── MODE 2: RINGKAS BEBAS GAMBAR ── -->
              <template v-else>
                <div class="flex items-center gap-1.5 min-w-0 flex-1">
                  <span
                    class="text-[7.5px] font-mono px-1 py-0.5 rounded flex-shrink-0 leading-none"
                    :class="
                      customAvatarPack[item.key]
                        ? 'bg-zinc-800 text-emerald-400 border border-zinc-700'
                        : 'bg-zinc-900 text-zinc-600 border border-zinc-800'
                    "
                  >
                    {{ customAvatarPack[item.key] ? 'OK' : '—' }}
                  </span>
                  <div class="min-w-0">
                    <div class="text-[8px] font-medium text-zinc-200 truncate leading-tight">{{ item.label }}</div>
                    <div class="text-[6.5px] text-zinc-500 font-mono truncate leading-none">{{ item.key }}</div>
                  </div>
                </div>
                <div class="flex items-center gap-1 flex-shrink-0">
                  <label
                    class="cursor-pointer bg-zinc-800 hover:bg-zinc-700 text-zinc-200 border border-zinc-700 text-[7px] font-medium px-1.5 py-0.5 rounded transition"
                    @click.stop
                  >
                    Upload
                    <input type="file" accept="image/png,image/webp,image/jpeg,image/svg+xml" class="hidden" @change="(e) => handleSpriteUpload(e, item.key)" />
                  </label>
                  <button
                    v-if="customAvatarPack[item.key]"
                    @click.stop="removeSingleSprite(item.key)"
                    title="Hapus sprite frame ini"
                    class="text-zinc-500 hover:text-rose-400 text-[7.5px] px-0.5 font-bold"
                  >
                    ✕
                  </button>
                </div>
              </template>
            </div>
          </div>

          <!-- Kontrol Navigasi Scroll Manual -->
          <div class="flex items-center justify-center gap-2 pt-1 pb-0.5">
            <button
              @click="scrollAreaUp('tab-avatar-scroll')"
              class="text-[7.5px] text-zinc-400 hover:text-zinc-200 bg-zinc-900 border border-zinc-800 hover:border-zinc-700 px-2.5 py-0.5 rounded transition"
            >
              Scroll Atas
            </button>
            <button
              @click="scrollAreaDown('tab-avatar-scroll')"
              class="text-[7.5px] text-zinc-400 hover:text-zinc-200 bg-zinc-900 border border-zinc-800 hover:border-zinc-700 px-2.5 py-0.5 rounded transition"
            >
              Scroll Bawah
            </button>
          </div>
        </div>

        <!-- Footer Tombol Simpan -->
        <div class="flex-shrink-0 p-2.5 border-t border-zinc-800/80 flex gap-2 bg-zinc-900/40">
          <button
            @click.stop="saveSettings"
            @mousedown.stop
            :disabled="isSavingConfig"
            class="flex-1 bg-zinc-100 hover:bg-white disabled:opacity-50 text-zinc-950 font-semibold py-1.5 rounded-lg text-[10px] transition shadow-sm"
          >
            {{ isSavingConfig ? 'Menyimpan...' : 'Simpan Pengaturan' }}
          </button>
          <button
            @click.stop="showSettings = false"
            @mousedown.stop
            class="px-3 bg-zinc-800 hover:bg-zinc-750 text-zinc-300 rounded-lg text-[10px] font-medium transition"
          >
            Tutup
          </button>
        </div>
      </div>
      <!-- ════════════════════════════════════════════ -->
      <!-- BALON DIALOG & INPUT CHAT                    -->
      <!-- ════════════════════════════════════════════ -->
      <div
        v-if="!showSettings && showMenuBar"
        class="w-full bg-zinc-950/95 backdrop-blur-xl border border-zinc-800 rounded-xl p-2.5 shadow-2xl text-zinc-100 flex flex-col gap-1.5 transition select-text"
      >
        <!-- Header Status & Tombol Kontrol -->
        <div
          @mousedown="startDrag"
          class="flex items-center justify-between border-b border-zinc-850 pb-1.5 cursor-move select-none"
        >
          <div class="flex items-center gap-1.5">
            <span
              class="px-2 py-0.5 rounded-full text-[9px] font-medium border flex items-center gap-1.5 transition"
              :class="stateBadgeStyle.pill"
            >
              <span class="w-1.5 h-1.5 rounded-full animate-pulse shadow-sm" :class="stateBadgeStyle.dot" />
              <span>{{ configData.assistant_name || 'Aoi' }} • {{ stateLabel }}</span>
            </span>
          </div>

          <div class="flex items-center gap-1" @mousedown.stop>
            <!-- Quick Switcher Avatar -->
            <div class="relative" @mousedown.stop>
              <button
                @click.stop="showQuickAvatarMenu = !showQuickAvatarMenu"
                @mousedown.stop
                title="Ganti Avatar Instan"
                class="p-1.5 rounded-md bg-zinc-900 border border-zinc-800 hover:bg-zinc-800 text-zinc-400 hover:text-zinc-200 transition flex items-center gap-1"
                :class="showQuickAvatarMenu ? 'border-zinc-500 text-zinc-100 bg-zinc-800' : ''"
              >
                <svg width="14" height="14" class="w-3.5 h-3.5 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
                </svg>
              </button>

              <!-- Dropdown Quick Switcher Menu -->
              <div
                v-if="showQuickAvatarMenu"
                class="absolute right-0 top-8 w-48 bg-zinc-950/98 backdrop-blur-xl border border-zinc-750 rounded-xl shadow-2xl p-1.5 z-50 animate-fade-in flex flex-col gap-1 text-[9.5px]"
                @mousedown.stop
              >
                <div class="px-2 py-1 text-[8px] font-semibold text-zinc-500 uppercase tracking-wider border-b border-zinc-850">
                  Ganti Karakter Avatar
                </div>
                <div class="max-h-40 overflow-y-auto custom-scrollbar flex flex-col gap-0.5">
                  <button
                    v-for="p in avatarPresets"
                    :key="p.id"
                    @click.stop="selectAvatarPreset(p.id); showQuickAvatarMenu = false"
                    class="w-full text-left px-2 py-1.5 rounded-lg transition flex items-center justify-between"
                    :class="
                      configData.active_avatar_preset === p.id
                        ? 'bg-zinc-800 text-zinc-100 font-medium'
                        : 'text-zinc-400 hover:bg-zinc-900 hover:text-zinc-200'
                    "
                  >
                    <span class="truncate">{{ p.name }}</span>
                    <span v-if="configData.active_avatar_preset === p.id" class="text-emerald-400 text-[10px]">✓</span>
                  </button>
                </div>
                <div class="border-t border-zinc-850 pt-1 flex items-center justify-between px-1">
                  <button
                    @click.stop="activeSettingsTab = 'presets'; openSettings(); showQuickAvatarMenu = false"
                    class="text-[8px] text-zinc-400 hover:text-zinc-200"
                  >
                    Kelola Preset...
                  </button>
                  <button
                    @click.stop="triggerZipUpload(); showQuickAvatarMenu = false"
                    class="text-[8px] text-zinc-300 hover:text-white font-medium"
                  >
                    + Unggah ZIP
                  </button>
                </div>
              </div>
            </div>

            <button
              @click.stop="clearMemory"
              @mousedown.stop
              title="Bersihkan Memori Percakapan"
              class="p-1.5 rounded-md bg-zinc-900 border border-zinc-800 hover:bg-zinc-800 text-zinc-400 hover:text-zinc-200 transition"
            >
              <svg width="14" height="14" class="w-3.5 h-3.5 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
              </svg>
            </button>
            <button
              @click.stop="openSettings"
              @mousedown.stop
              title="Buka Pengaturan (Alt+S)"
              class="p-1.5 rounded-md bg-zinc-900 border border-zinc-800 hover:bg-zinc-800 text-zinc-400 hover:text-zinc-200 transition"
            >
              <svg width="14" height="14" class="w-3.5 h-3.5 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
                <path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
              </svg>
            </button>
            <button
              @click.stop="ghostMode = !ghostMode"
              @mousedown.stop
              title="Ghost Mode (Transparan)"
              class="p-1.5 rounded-md bg-zinc-900 border border-zinc-800 hover:bg-zinc-800 text-zinc-400 hover:text-zinc-200 transition"
            >
              <svg width="14" height="14" class="w-3.5 h-3.5 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M9.88 9.88l-3.29-3.29m7.532 7.532l3.29 3.29M3 3l18 18" />
              </svg>
            </button>
            <button
              @click.stop="toggleMinimize"
              @mousedown.stop
              title="Minimize (Super+H)"
              class="p-1.5 rounded-md bg-zinc-900 border border-zinc-800 hover:bg-zinc-800 text-zinc-400 hover:text-zinc-200 transition"
            >
              <svg width="14" height="14" class="w-3.5 h-3.5 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M18 12H6" />
              </svg>
            </button>
            <button
              @click.stop="showMenuBar = false"
              @mousedown.stop
              title="Sembunyikan Menu Bar (Esc / Klik Avatar)"
              class="p-1.5 rounded-md bg-zinc-900 border border-zinc-800 hover:bg-zinc-800 text-zinc-400 hover:text-rose-300 transition"
            >
              <svg width="14" height="14" class="w-3.5 h-3.5 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>
        </div>

        <!-- Teks Pesan Asisten -->
        <div class="max-h-[110px] overflow-y-auto pr-1 text-[11px] leading-relaxed text-zinc-200 break-words whitespace-pre-wrap custom-scrollbar">
          {{ message }}
        </div>

        <!-- Loading bounce saat thinking -->
        <div v-if="currentState === 'thinking'" class="flex items-center gap-1 mt-0.5">
          <span class="w-1.5 h-1.5 rounded-full bg-zinc-400 animate-bounce" style="animation-delay: 0ms" />
          <span class="w-1.5 h-1.5 rounded-full bg-zinc-400 animate-bounce" style="animation-delay: 150ms" />
          <span class="w-1.5 h-1.5 rounded-full bg-zinc-400 animate-bounce" style="animation-delay: 300ms" />
        </div>

        <!-- ── Konfirmasi Red Tier Perintah Bash ── -->
        <div
          v-if="currentState === 'confirm' && pendingCommand"
          class="mt-1 p-2 rounded-lg bg-zinc-900 border border-zinc-700 shadow-lg"
        >
          <div class="text-[9px] text-zinc-400 font-medium mb-1">
            Konfirmasi Eksekusi Terminal:
          </div>
          <code class="block text-[10px] font-mono text-zinc-200 bg-black/60 px-2 py-1 rounded-md break-all border border-zinc-800">
            $ {{ pendingCommand }}
          </code>
          <div class="flex gap-2 mt-1.5">
            <button
              @click="confirmBash"
              class="flex-1 bg-zinc-100 hover:bg-white text-zinc-950 text-[9.5px] font-semibold py-1 rounded-md transition shadow-sm"
            >
              [Alt+Y] Setuju
            </button>
            <button
              @click="cancelBash"
              class="flex-1 bg-zinc-800 hover:bg-zinc-750 text-zinc-300 text-[9.5px] font-medium py-1 rounded-md transition"
            >
              [Alt+N] Batal
            </button>
          </div>
        </div>

        <!-- ── Chat Input Bar & Mic ── -->
        <div class="flex gap-1.5 mt-0.5">
          <button
            @click="toggleMic"
            :title="currentState === 'listening' ? 'Hentikan & Proses Suara' : 'Mulai Bicara (Super+V)'"
            class="px-2 py-1.5 rounded-lg border text-[11px] font-medium transition flex items-center justify-center shadow-sm"
            :class="
              currentState === 'listening'
                ? 'bg-rose-500/20 border-rose-500/60 text-rose-300 animate-pulse'
                : 'bg-zinc-900 border-zinc-750 text-zinc-300 hover:bg-zinc-850 hover:text-zinc-100'
            "
          >
            <svg width="14" height="14" class="w-3.5 h-3.5 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M19 11a7 7 0 01-7 7m0 0a7 7 0 01-7-7m7 7v4m0 0H8m4 0h4m-4-8a3 3 0 01-3-3V5a3 3 0 116 0v6a3 3 0 01-3 3z" />
            </svg>
          </button>
          <input
            v-model="chatInput"
            @keydown="handleChatKeydown"
            :disabled="isSendingChat || currentState === 'thinking'"
            :placeholder="'Tanya ' + (configData.assistant_name || 'Aoi') + ' atau perintah...'"
            class="flex-1 text-[10.5px] bg-zinc-900 border border-zinc-750 rounded-lg px-2.5 py-1 text-zinc-100 placeholder-zinc-500 outline-none focus:border-zinc-400 transition disabled:opacity-50"
          />
          <button
            @click="sendChat"
            :disabled="isSendingChat || !chatInput.trim() || currentState === 'thinking'"
            class="px-2.5 py-1 rounded-lg bg-zinc-100 hover:bg-white disabled:opacity-40 text-zinc-950 text-[10px] font-semibold transition shadow-sm flex items-center justify-center"
          >
            <svg v-if="!isSendingChat" width="14" height="14" class="w-3.5 h-3.5 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M14 5l7 7m0 0l-7 7m7-7H3" />
            </svg>
            <span v-else class="text-[9px]">...</span>
          </button>
        </div>
      </div>

      <!-- ════════════════════════════════════════════ -->
      <!-- SPRITE AVATAR 2D DESKTOP COMPANION           -->
      <!-- ════════════════════════════════════════════ -->
      <div
        v-if="!showSettings"
        @mousedown="handleAvatarMouseDown"
        @wheel.prevent="handleAvatarWheel"
        :style="{ width: avatarSize + 'px', height: avatarSize + 'px' }"
        class="relative group flex items-center justify-center cursor-move aoi-avatar-container flex-shrink-0 select-none transition-[width,height] duration-150 ease-out"
        :title="'Klik kiri: ' + (showMenuBar ? 'Sembunyikan menu' : 'Tampilkan menu') + ' • Scroll: Zoom (' + avatarSize + 'px) • Tahan & geser: Pindah window'"
      >
        <!-- Overlay Mini Zoom Badge saat Hover Avatar -->
        <div
          class="absolute -top-6 left-1/2 -translate-x-1/2 opacity-0 group-hover:opacity-100 transition-opacity pointer-events-auto flex items-center gap-1 bg-zinc-950/90 border border-zinc-800 backdrop-blur-md px-1.5 py-0.5 rounded-full shadow-lg text-[8px] z-10 select-none whitespace-nowrap"
          @mousedown.stop
        >
          <button
            @click.stop="setAvatarSize(avatarSize - 12)"
            title="Perkecil Avatar"
            class="w-4 h-4 rounded-full bg-zinc-850 hover:bg-zinc-750 text-zinc-300 flex items-center justify-center text-[10px] font-bold transition"
          >
            -
          </button>
          <span class="text-zinc-300 font-mono font-medium px-1">{{ avatarSize }}px</span>
          <button
            @click.stop="setAvatarSize(avatarSize + 12)"
            title="Perbesar Avatar"
            class="w-4 h-4 rounded-full bg-zinc-850 hover:bg-zinc-750 text-zinc-300 flex items-center justify-center text-[10px] font-bold transition"
          >
            +
          </button>
        </div>

        <!-- Custom PNG / WebP Avatar -->
        <img
          v-if="activeCustomFrame"
          :src="activeCustomFrame"
          alt="Aoi Avatar"
          class="w-full h-full object-contain pointer-events-none select-none transition-all duration-150"
        />

        <!-- Built-in Karakter Anime Chibi SVG -->
        <svg
          v-else
          viewBox="0 0 200 200"
          class="w-full h-full transition-transform duration-200 pointer-events-none"
          :class="{
            'scale-105': currentState === 'listening',
            'translate-y-[-2px]': currentState === 'speaking' && isMouthOpen,
          }"
        >
          <!-- Dynamic Aura Glow -->
          <circle
            cx="100"
            cy="105"
            r="74"
            class="transition-colors duration-500"
            :fill="
              currentState === 'listening'
                ? 'rgba(16,185,129,0.25)'
                : currentState === 'thinking'
                ? 'rgba(245,158,11,0.25)'
                : currentState === 'error'
                ? 'rgba(244,63,94,0.28)'
                : currentState === 'confirm'
                ? 'rgba(168,85,247,0.25)'
                : 'rgba(56,189,248,0.18)'
            "
          />

          <!-- Rambut belakang -->
          <path d="M42 85 Q30 155 52 178 L148 178 Q170 155 158 85 Z" fill="#2e1065" />

          <!-- Hoodie & Busana -->
          <path d="M55 145 Q100 135 145 145 L155 192 L45 192 Z" fill="#4338ca" />
          <path d="M82 145 L100 172 L118 145" fill="#e0e7ff" />
          <circle cx="100" cy="155" r="4" fill="#a5b4fc" />

          <!-- Wajah Chibi -->
          <circle cx="100" cy="100" r="47" fill="#fed7aa" />

          <!-- Poni & Rambut Depan Anime -->
          <path
            d="M48 95 Q52 45 100 45 Q148 45 152 95 Q135 68 118 88 Q100 65 82 88 Q65 68 48 95 Z"
            fill="#3730a3"
          />

          <!-- Telinga Kucing / Aksesoris -->
          <polygon points="52,65 38,36 72,52" fill="#4f46e5" />
          <polygon points="50,60 42,42 64,52" fill="#f43f5e" />
          <polygon points="148,65 162,36 128,52" fill="#4f46e5" />
          <polygon points="150,60 158,42 136,52" fill="#f43f5e" />

          <!-- Mata Sesuai Emosi -->
          <g v-if="currentState === 'error'" stroke="#e11d48" stroke-width="3.5" stroke-linecap="round">
            <line x1="72" y1="94" x2="86" y2="108" />
            <line x1="86" y1="94" x2="72" y2="108" />
            <line x1="114" y1="94" x2="128" y2="108" />
            <line x1="128" y1="94" x2="114" y2="108" />
          </g>
          <g v-else-if="isBlinking" stroke="#1e1b4b" stroke-width="3" fill="none" stroke-linecap="round">
            <path d="M70 102 Q79 108 88 102" />
            <path d="M112 102 Q121 108 130 102" />
          </g>
          <g v-else-if="currentState === 'thinking'">
            <ellipse cx="79" cy="98" rx="7.5" ry="8.5" fill="#1e1b4b" />
            <ellipse cx="121" cy="98" rx="7.5" ry="8.5" fill="#1e1b4b" />
            <circle cx="80" cy="94" r="3.5" fill="#fff" />
            <circle cx="122" cy="94" r="3.5" fill="#fff" />
          </g>
          <g v-else>
            <ellipse cx="79" cy="101" rx="8" ry="9.5" fill="#1e1b4b" />
            <ellipse cx="121" cy="101" rx="8" ry="9.5" fill="#1e1b4b" />
            <circle cx="76" cy="97" r="3.5" fill="#fff" />
            <circle cx="118" cy="97" r="3.5" fill="#fff" />
            <circle cx="82" cy="104" r="1.5" fill="#a5b4fc" />
            <circle cx="124" cy="104" r="1.5" fill="#a5b4fc" />
          </g>

          <!-- Pipi Merona / Blush -->
          <ellipse cx="65" cy="113" rx="6" ry="3.5" fill="#fb7185" opacity="0.75" />
          <ellipse cx="135" cy="113" rx="6" ry="3.5" fill="#fb7185" opacity="0.75" />

          <!-- Mulut Animasi -->
          <ellipse
            v-if="currentState === 'speaking' && isMouthOpen"
            cx="100"
            cy="122"
            rx="7"
            ry="6"
            fill="#f43f5e"
          />
          <ellipse
            v-else-if="currentState === 'error' || currentState === 'confirm'"
            cx="100"
            cy="123"
            rx="5"
            ry="4.5"
            fill="none"
            stroke="#1e1b4b"
            stroke-width="2.5"
          />
          <path
            v-else
            d="M93 120 Q100 126 107 120"
            fill="none"
            stroke="#1e1b4b"
            stroke-width="2.5"
            stroke-linecap="round"
          />

          <!-- Ikon status mikrofon saat listening -->
          <g v-if="currentState === 'listening'">
            <circle cx="156" cy="62" r="14" fill="#10b981" />
            <rect x="152" y="53" width="8" height="12" rx="4" fill="white" />
            <path d="M149 61 Q156 70 163 61" fill="none" stroke="white" stroke-width="2" />
          </g>
        </svg>
      </div>

      <!-- ── Dev Toolbar (Klik Kanan Avatar untuk Tampilkan) ── -->
      <div
        v-if="showDevToolbar && !showSettings"
        class="w-full bg-zinc-950/95 border border-zinc-800 rounded-xl p-2 shadow-2xl"
      >
        <div class="text-[8.5px] text-zinc-500 text-center mb-1 font-medium">
          Dev Panel — Klik Kanan Avatar untuk Tutup
        </div>
        <div class="grid grid-cols-3 gap-1">
          <button
            @click="triggerState('idle', 'Standby... Tekan Super+V untuk bicara.')"
            class="text-[9px] py-1 rounded bg-zinc-900 border border-zinc-800 hover:bg-zinc-850 text-zinc-300"
          >
            Idle
          </button>
          <button
            @click="triggerState('listening', 'Mendengarkan suaramu...')"
            class="text-[9px] py-1 rounded bg-zinc-900 border border-zinc-800 hover:bg-zinc-850 text-emerald-400"
          >
            Listening
          </button>
          <button
            @click="triggerState('thinking', 'Sedang memproses instruksi...')"
            class="text-[9px] py-1 rounded bg-zinc-900 border border-zinc-800 hover:bg-zinc-850 text-amber-400"
          >
            Thinking
          </button>
          <button
            @click="triggerState('speaking', 'Halo! Aku sedang berbicara dengan lip-sync aktif.')"
            class="text-[9px] py-1 rounded bg-zinc-900 border border-zinc-800 hover:bg-zinc-850 text-zinc-200"
          >
            Speaking
          </button>
          <button
            @click="triggerState('confirm', 'Izinkan saya membersihkan paket pacman?', 'sudo pacman -Scc')"
            class="text-[9px] py-1 rounded bg-zinc-900 border border-zinc-800 hover:bg-zinc-850 text-purple-400"
          >
            Confirm
          </button>
          <button
            @click="triggerState('error', 'Koneksi ke LLM server terputus.')"
            class="text-[9px] py-1 rounded bg-zinc-900 border border-zinc-800 hover:bg-zinc-850 text-rose-400"
          >
            Error
          </button>
        </div>
        <button
          @click.stop="openSettings"
          @mousedown.stop
          class="w-full mt-1.5 py-1 text-[9px] font-medium rounded bg-zinc-850 hover:bg-zinc-800 border border-zinc-700 text-zinc-200 transition"
        >
          Buka Pengaturan Studio
        </button>
      </div>
    </div>
  </div>
</template>

<style>
html,
body,
#app {
  width: 100%;
  height: 100%;
  background: transparent !important;
  margin: 0;
  padding: 0;
  overflow: hidden;
  font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  user-select: none;
}

/* Biarkan teks input, code, dan textarea bisa diseleksi */
input,
textarea,
code,
p {
  user-select: text;
}

/* Custom Sleek Scrollbar (Cross-Browser & WebKitGTK / Wayland) */
.custom-scrollbar {
  scrollbar-width: thin;
  scrollbar-color: rgba(161, 161, 170, 0.3) transparent;
  overscroll-behavior: contain;
  -webkit-overflow-scrolling: touch;
}

.custom-scrollbar::-webkit-scrollbar {
  width: 5px;
  height: 5px;
}

.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}

.custom-scrollbar::-webkit-scrollbar-thumb {
  background: rgba(161, 161, 170, 0.25);
  border-radius: 9999px;
}

.custom-scrollbar::-webkit-scrollbar-thumb:hover {
  background: rgba(161, 161, 170, 0.5);
}

/* Subtle Floating Animation for Avatar */
@keyframes aoi-float {
  0%, 100% {
    transform: translateY(0px);
  }
  50% {
    transform: translateY(-3px);
  }
}

.aoi-avatar-container {
  animation: aoi-float 5s ease-in-out infinite;
}
</style>