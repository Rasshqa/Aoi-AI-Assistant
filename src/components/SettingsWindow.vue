<script setup lang="ts">
import { ref, onMounted, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { AppConfig, AvatarPresetMeta, ClonedVoiceMeta } from '../types';

const activeTab = ref<'persona' | 'ai' | 'voice' | 'presets' | 'avatar' | 'clone' | 'chat'>('persona');
const isSaving = ref(false);
const saveMessage = ref('');

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
  tts_model_path: '~/.local/share/piper/mommy_lessac.onnx',
  whisper_model_path: '',
  avatar_folder_path: '',
  assistant_name: 'Aoi',
  user_name: 'Kak',
  personality_preset: 'caring',
  custom_personality: '',
  avatar_size: 144,
  tts_engine: 'piper',
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

const customAvatarPack = ref<Record<string, string>>({
  idle_open: '',
  idle_blink: '',
  listening: '',
  thinking: '',
  speak_open: '',
  speak_closed: '',
  confirm: '',
  error: '',
});

watch(() => configData.value.avatar_size, (newSize) => {
  if (newSize && newSize >= 100) {
    localStorage.setItem('aoi_avatar_size', String(newSize));
    invoke('update_window_mode', { expanded: false, avatarSize: newSize }).catch(() => {});
  }
});

// Personality Templates
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
    id: 'playful',
    title: 'Jahil & Menggemaskan',
    badge: 'Mesugaki',
    text: 'Sedikit nakal, suka menggoda dengan nada imut menggemaskan tapi tetap sangat membantu pekerjaan pengguna.',
    sample: (name: string, user: string) => `Hehe~ Lemah banget ${user}, masa ngetik gitu aja butuh bantuan ${name}? Canda deng!`,
  },
  {
    id: 'maid',
    title: 'Pelayan Setia',
    badge: 'Maid',
    text: 'Sangat sopan, setia, patuh, dan mengutamakan efisiensi. Memanggil pengguna dengan sebutan hormat seperti Tuan/Nyonya.',
    sample: (name: string, user: string) => `Selamat datang, ${user}. ${name} siap melayani seluruh kebutuhan terminal Anda hari ini.`,
  },
];

const applyPersonalityTemplate = (tmpl: any) => {
  configData.value.personality_preset = tmpl.id;
  configData.value.custom_personality = tmpl.text;
};

// Presets State
const avatarPresets = ref<AvatarPresetMeta[]>([]);
const showNewPresetModal = ref(false);
const newPresetName = ref('');
const zipFileInput = ref<HTMLInputElement | null>(null);

const fetchAvatarPresets = async () => {
  try {
    const list = await invoke<AvatarPresetMeta[]>('list_avatar_presets');
    avatarPresets.value = list;
  } catch (e) {
    console.error('Fetch avatar presets failed:', e);
  }
};

const createNewPreset = async () => {
  if (!newPresetName.value.trim()) return;
  try {
    const res = await invoke<AvatarPresetMeta>('create_avatar_preset', { name: newPresetName.value.trim() });
    await fetchAvatarPresets();
    await selectAvatarPreset(res.id);
    showNewPresetModal.value = false;
    newPresetName.value = '';
  } catch (e) {
    alert('Gagal membuat preset: ' + e);
  }
};

const selectAvatarPreset = async (presetId: string) => {
  try {
    configData.value.active_avatar_preset = presetId;
    const pack = await invoke<Record<string, string>>('switch_avatar_preset', { presetId });
    customAvatarPack.value = { ...customAvatarPack.value, ...pack };
    await fetchAvatarPresets();
  } catch (e) {
    alert('Gagal berganti preset: ' + e);
  }
};

const deletePreset = async (presetId: string) => {
  if (!confirm('Yakin ingin menghapus preset avatar ini?')) return;
  try {
    await invoke('delete_avatar_preset', { presetId });
    await fetchAvatarPresets();
  } catch (e) {
    alert('Gagal menghapus preset: ' + e);
  }
};

const triggerZipUpload = () => {
  zipFileInput.value?.click();
};

const handleZipUpload = async (event: Event) => {
  const target = event.target as HTMLInputElement;
  if (!target.files || target.files.length === 0) return;
  const file = target.files[0];
  const reader = new FileReader();
  reader.onload = async () => {
    try {
      const base64 = (reader.result as string).split(',')[1];
      const preset = await invoke<AvatarPresetMeta>('import_avatar_preset_zip', {
        name: file.name.replace(/\.zip$/i, ''),
        zipBase64: base64,
      });
      await fetchAvatarPresets();
      await selectAvatarPreset(preset.id);
      alert('Preset ZIP berhasil diimpor!');
    } catch (e) {
      alert('Gagal impor ZIP: ' + e);
    }
  };
  reader.readAsDataURL(file);
  target.value = '';
};

// Sprites State
const avatarFrameList = [
  { key: 'idle_open', label: 'Idle Terbuka', desc: 'Mata terbuka saat standby' },
  { key: 'idle_blink', label: 'Idle Kedip', desc: 'Mata terpejam saat kedip' },
  { key: 'listening', label: 'Mendengarkan', desc: 'Saat mikrofon aktif (Super+V)' },
  { key: 'thinking', label: 'Berpikir', desc: 'Saat memproses LLM' },
  { key: 'speak_open', label: 'Bicara Buka', desc: 'Mulut terbuka saat TTS suara' },
  { key: 'speak_closed', label: 'Bicara Tutup', desc: 'Mulut tertutup saat jeda kata' },
  { key: 'confirm', label: 'Konfirmasi', desc: 'Perintah berbahaya Red Tier' },
  { key: 'error', label: 'Kendala', desc: 'Saat terjadi error sistem' },
];

const loadAvatarPack = async () => {
  try {
    const pack = await invoke<Record<string, string>>('load_avatar_pack');
    if (pack) {
      customAvatarPack.value = { ...customAvatarPack.value, ...pack };
    }
  } catch (e) {
    console.warn('Gagal load avatar pack:', e);
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
      console.error('Gagal simpan sprite:', e);
    }
  };
  reader.readAsDataURL(file);
  target.value = '';
};

const resetAvatarPack = async () => {
  if (!confirm('Kembalikan ke karakter anime bawaan?')) return;
  try {
    await invoke('reset_avatar_pack');
    for (const k of Object.keys(customAvatarPack.value)) {
      customAvatarPack.value[k] = '';
    }
    alert('Avatar berhasil dikembalikan ke default SVG!');
  } catch (e) {
    alert('Gagal reset avatar: ' + e);
  }
};

// Cloned Voices
const clonedVoices = ref<ClonedVoiceMeta[]>([]);
const voiceUploadInput = ref<HTMLInputElement | null>(null);
const isCloningVoice = ref(false);

const fetchClonedVoices = async () => {
  try {
    clonedVoices.value = await invoke<ClonedVoiceMeta[]>('list_cloned_voices');
  } catch (e) {
    console.error('Fetch cloned voices failed:', e);
  }
};

const triggerVoiceUpload = () => {
  voiceUploadInput.value?.click();
};

const handleVoiceUpload = async (event: Event) => {
  const target = event.target as HTMLInputElement;
  if (!target.files || target.files.length === 0) return;
  const file = target.files[0];
  const reader = new FileReader();
  reader.onload = async () => {
    try {
      isCloningVoice.value = true;
      const base64 = (reader.result as string).split(',')[1];
      await invoke('clone_voice_from_file', {
        name: file.name.replace(/\.[^/.]+$/, ''),
        engine: 'elevenlabs',
        fileBase64: base64,
        filename: file.name,
        apiKey: configData.value.elevenlabs_api_key || null,
      });
      await fetchClonedVoices();
      alert('Suara berhasil diunggah!');
    } catch (e) {
      alert('Gagal kloning suara: ' + e);
    } finally {
      isCloningVoice.value = false;
    }
  };
  reader.readAsDataURL(file);
  target.value = '';
};

const deleteClonedVoice = async (id: string) => {
  if (!confirm('Hapus profil suara ini?')) return;
  try {
    await invoke('delete_cloned_voice', { voiceId: id });
    await fetchClonedVoices();
  } catch (e) {
    alert('Gagal hapus suara: ' + e);
  }
};

// Test Suara TTS
const testVoiceLoading = ref(false);
const testVoice = async () => {
  testVoiceLoading.value = true;
  try {
    await invoke('test_tts_voice');
  } catch (e) {
    alert('Uji suara gagal: ' + e);
  } finally {
    testVoiceLoading.value = false;
  }
};

// Chat Tester
const testChatInput = ref('');
const testChatHistory = ref<{ role: 'user' | 'assistant'; text: string }[]>([]);
const isTestingChat = ref(false);

const sendTestChat = async () => {
  const msg = testChatInput.value.trim();
  if (!msg || isTestingChat.value) return;
  testChatInput.value = '';
  testChatHistory.value.push({ role: 'user', text: msg });
  isTestingChat.value = true;
  try {
    const res = await invoke<string>('ask_llm', { userMessage: msg });
    testChatHistory.value.push({ role: 'assistant', text: res });
  } catch (e) {
    testChatHistory.value.push({ role: 'assistant', text: 'Error: ' + e });
  } finally {
    isTestingChat.value = false;
  }
};

// Save Settings
const saveConfig = async () => {
  isSaving.value = true;
  saveMessage.value = '';
  try {
    await invoke('save_config', { cfg: configData.value });
    saveMessage.value = '✅ Pengaturan berhasil disimpan!';
    setTimeout(() => {
      saveMessage.value = '';
    }, 3500);
  } catch (e) {
    saveMessage.value = '❌ Gagal menyimpan: ' + e;
  } finally {
    isSaving.value = false;
  }
};

onMounted(async () => {
  try {
    const cfg = await invoke<AppConfig>('get_config');
    configData.value = { ...configData.value, ...cfg };
  } catch (e) {
    console.error('Gagal mengambil config:', e);
  }
  await fetchAvatarPresets();
  await loadAvatarPack();
  await fetchClonedVoices();
});
</script>

<template>
  <div class="w-full h-full bg-zinc-950 text-zinc-100 flex flex-col font-sans select-none overflow-hidden">
    <!-- Header Window -->
    <header class="flex-shrink-0 px-6 py-4 border-b border-zinc-800/80 bg-zinc-900/60 flex items-center justify-between">
      <div class="flex items-center gap-3">
        <div class="w-9 h-9 rounded-xl overflow-hidden border border-zinc-700/80 shadow-md flex-shrink-0 bg-zinc-900">
          <img src="/Aoi.png" alt="Aoi Icon" class="w-full h-full object-cover" />
        </div>
        <div>
          <h1 class="text-base font-bold text-zinc-100 flex items-center gap-2">
            Pengaturan Aoi
            <span class="text-[10px] font-mono px-2 py-0.5 rounded-full bg-indigo-500/10 text-indigo-400 border border-indigo-500/20">
              Desktop Companion
            </span>
          </h1>
          <p class="text-xs text-zinc-400">Atur model AI, kepribadian, suara, dan tampilan avatar</p>
        </div>
      </div>
      <div class="flex items-center gap-3">
        <button
          @click="saveConfig"
          :disabled="isSaving"
          class="bg-indigo-600 hover:bg-indigo-500 active:scale-95 disabled:opacity-50 text-white font-medium text-xs px-4 py-2 rounded-xl transition shadow-lg shadow-indigo-600/20 flex items-center gap-1.5"
        >
          <svg v-if="!isSaving" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
          </svg>
          <span v-else class="animate-spin text-sm">⟳</span>
          <span>{{ isSaving ? 'Menyimpan...' : 'Simpan Perubahan' }}</span>
        </button>
      </div>
    </header>

    <!-- Main Container: Sidebar Navigation + Content Panel -->
    <div class="flex-1 flex min-h-0 overflow-hidden">
      <!-- Sidebar Tabs -->
      <aside class="w-56 border-r border-zinc-800/80 bg-zinc-900/30 p-3 flex flex-col gap-1 overflow-y-auto custom-scrollbar">
        <button
          @click="activeTab = 'persona'"
          class="w-full text-left px-3 py-2.5 rounded-xl text-xs font-medium transition flex items-center gap-2.5"
          :class="activeTab === 'persona' ? 'bg-indigo-600/15 text-indigo-400 border border-indigo-500/30 font-semibold' : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-850'"
        >
          <span>👤</span>
          <span>Karakter & Persona</span>
        </button>

        <button
          @click="activeTab = 'ai'"
          class="w-full text-left px-3 py-2.5 rounded-xl text-xs font-medium transition flex items-center gap-2.5"
          :class="activeTab === 'ai' ? 'bg-indigo-600/15 text-indigo-400 border border-indigo-500/30 font-semibold' : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-850'"
        >
          <span>🧠</span>
          <span>Model AI & API</span>
        </button>

        <button
          @click="activeTab = 'voice'"
          class="w-full text-left px-3 py-2.5 rounded-xl text-xs font-medium transition flex items-center gap-2.5"
          :class="activeTab === 'voice' ? 'bg-indigo-600/15 text-indigo-400 border border-indigo-500/30 font-semibold' : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-850'"
        >
          <span>🔊</span>
          <span>Audio & Suara (TTS)</span>
        </button>

        <button
          @click="activeTab = 'presets'"
          class="w-full text-left px-3 py-2.5 rounded-xl text-xs font-medium transition flex items-center gap-2.5"
          :class="activeTab === 'presets' ? 'bg-indigo-600/15 text-indigo-400 border border-indigo-500/30 font-semibold' : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-850'"
        >
          <span>🎭</span>
          <span>Preset Karakter</span>
        </button>

        <button
          @click="activeTab = 'avatar'"
          class="w-full text-left px-3 py-2.5 rounded-xl text-xs font-medium transition flex items-center gap-2.5"
          :class="activeTab === 'avatar' ? 'bg-indigo-600/15 text-indigo-400 border border-indigo-500/30 font-semibold' : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-850'"
        >
          <span>🖼</span>
          <span>Sprite Studio (8 Emosi)</span>
        </button>

        <button
          @click="activeTab = 'clone'"
          class="w-full text-left px-3 py-2.5 rounded-xl text-xs font-medium transition flex items-center gap-2.5"
          :class="activeTab === 'clone' ? 'bg-indigo-600/15 text-indigo-400 border border-indigo-500/30 font-semibold' : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-850'"
        >
          <span>🎙</span>
          <span>Kloning Suara</span>
        </button>

        <div class="my-2 border-t border-zinc-800/80"></div>

        <button
          @click="activeTab = 'chat'"
          class="w-full text-left px-3 py-2.5 rounded-xl text-xs font-medium transition flex items-center gap-2.5"
          :class="activeTab === 'chat' ? 'bg-indigo-600/15 text-indigo-400 border border-indigo-500/30 font-semibold' : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-850'"
        >
          <span>💬</span>
          <span>Uji Coba Obrolan</span>
        </button>

        <!-- Toast Simpan Status -->
        <div v-if="saveMessage" class="mt-auto p-2.5 rounded-xl bg-zinc-900 border border-zinc-700 text-center text-xs font-medium">
          {{ saveMessage }}
        </div>
      </aside>

      <!-- Content Area -->
      <main class="flex-1 p-6 overflow-y-auto custom-scrollbar bg-zinc-950">
        <!-- ── TAB PERSONA ── -->
        <section v-if="activeTab === 'persona'" class="space-y-6 max-w-2xl">
          <div>
            <h2 class="text-sm font-bold text-zinc-100 mb-1">Karakter & Identitas Asisten</h2>
            <p class="text-xs text-zinc-400">Tentukan nama, cara asisten memanggilmu, dan sifat kepribadiannya.</p>
          </div>

          <div class="grid grid-cols-2 gap-4">
            <div>
              <label class="block text-xs font-medium text-zinc-300 mb-1.5">Nama Asisten</label>
              <input
                v-model="configData.assistant_name"
                placeholder="Contoh: Aoi, Lumi, Kurisu"
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-indigo-500 rounded-xl px-3 py-2 text-xs text-zinc-100 outline-none transition"
              />
            </div>
            <div>
              <label class="block text-xs font-medium text-zinc-300 mb-1.5">Panggilan Kamu</label>
              <input
                v-model="configData.user_name"
                placeholder="Contoh: Kak, Master, Sensei"
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-indigo-500 rounded-xl px-3 py-2 text-xs text-zinc-100 outline-none transition"
              />
            </div>
          </div>

          <div>
            <label class="block text-xs font-medium text-zinc-300 mb-1.5">Ukuran Avatar Desktop (Pixel)</label>
            <div class="flex items-center gap-3">
              <input
                type="range"
                min="120"
                max="800"
                step="10"
                v-model.number="configData.avatar_size"
                class="flex-1 accent-indigo-500"
              />
              <span class="font-mono text-xs text-zinc-300 bg-zinc-900 px-3 py-1.5 rounded-lg border border-zinc-800">
                {{ configData.avatar_size || 280 }}px
              </span>
            </div>
          </div>

          <div>
            <label class="block text-xs font-medium text-zinc-300 mb-1.5">Deskripsi Kepribadian Bebas</label>
            <textarea
              v-model="configData.custom_personality"
              rows="4"
              placeholder="Tuliskan kepribadian sesukamu di sini... Contoh: Ceria, ramah, suka bercanda ringan, dan selalu siap membantu tugas Linux di Hyprland."
              class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-indigo-500 rounded-xl p-3 text-xs text-zinc-100 outline-none transition custom-scrollbar resize-none leading-relaxed"
            />
          </div>

          <div>
            <div class="text-xs font-medium text-zinc-400 mb-2">Pilih Cepat dari Template:</div>
            <div class="grid grid-cols-3 gap-2">
              <button
                v-for="tmpl in personalityTemplates"
                :key="tmpl.id"
                type="button"
                @click="applyPersonalityTemplate(tmpl)"
                class="p-3 rounded-xl border border-zinc-800 bg-zinc-900/60 hover:bg-zinc-850 hover:border-indigo-500/50 text-left transition flex flex-col justify-between"
              >
                <div class="text-xs font-semibold text-zinc-200">{{ tmpl.title }}</div>
                <div class="text-[10px] text-indigo-400 mt-1">{{ tmpl.badge }}</div>
              </button>
            </div>
          </div>
        </section>

        <!-- ── TAB MODEL AI ── -->
        <section v-else-if="activeTab === 'ai'" class="space-y-6 max-w-2xl">
          <div>
            <h2 class="text-sm font-bold text-zinc-100 mb-1">Konfigurasi Model AI & Provider</h2>
            <p class="text-xs text-zinc-400">Pilih provider LLM yang ingin kamu gunakan untuk kecerdasan Aoi.</p>
          </div>

          <div>
            <label class="block text-xs font-medium text-zinc-300 mb-2">Pilih Provider AI</label>
            <div class="grid grid-cols-3 gap-2">
              <button
                v-for="p in [
                  { id: 'gemini', name: 'Google Gemini', desc: 'Rekomendasi (Cepat & Gratis)' },
                  { id: 'groq', name: 'Groq Cloud', desc: 'Ultra-cepat (Llama 3)' },
                  { id: 'openai', name: 'OpenAI (GPT)', desc: 'GPT-4o / GPT-4o-mini' },
                  { id: 'openrouter', name: 'OpenRouter', desc: 'Akses ratusan model' },
                  { id: 'ollama', name: 'Ollama Lokal', desc: 'Offline tanpa kuota' },
                  { id: 'custom', name: 'Custom OpenAI-API', desc: 'DeepSeek / Local vLLM' },
                ]"
                :key="p.id"
                type="button"
                @click="configData.llm_provider = p.id"
                class="p-3 rounded-xl border text-left transition flex flex-col justify-between"
                :class="configData.llm_provider === p.id ? 'border-indigo-500 bg-indigo-500/10 text-zinc-100' : 'border-zinc-800 bg-zinc-900/60 text-zinc-400 hover:bg-zinc-850'"
              >
                <div class="text-xs font-semibold">{{ p.name }}</div>
                <div class="text-[10px] text-zinc-500 mt-0.5">{{ p.desc }}</div>
              </button>
            </div>
          </div>

          <!-- Dynamic Key Inputs -->
          <div v-if="configData.llm_provider === 'gemini'" class="space-y-3">
            <div>
              <label class="block text-xs font-medium text-zinc-300 mb-1">Gemini API Key</label>
              <input
                type="password"
                v-model="configData.gemini_api_key"
                placeholder="AIzaSy..."
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-indigo-500 rounded-xl px-3 py-2 text-xs text-zinc-100 outline-none transition font-mono"
              />
              <span class="text-[10px] text-zinc-500 mt-1 block">Dapatkan gratis di aistudio.google.com</span>
            </div>
            <div>
              <label class="block text-xs font-medium text-zinc-300 mb-1">Nama Model</label>
              <input
                v-model="configData.model_name"
                placeholder="gemini-2.0-flash"
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-indigo-500 rounded-xl px-3 py-2 text-xs text-zinc-100 outline-none transition"
              />
            </div>
          </div>

          <div v-else-if="configData.llm_provider === 'groq'" class="space-y-3">
            <div>
              <label class="block text-xs font-medium text-zinc-300 mb-1">Groq API Key</label>
              <input
                type="password"
                v-model="configData.groq_api_key"
                placeholder="gsk_..."
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-indigo-500 rounded-xl px-3 py-2 text-xs text-zinc-100 outline-none transition font-mono"
              />
            </div>
            <div>
              <label class="block text-xs font-medium text-zinc-300 mb-1">Nama Model</label>
              <input
                v-model="configData.model_name"
                placeholder="llama-3.3-70b-versatile"
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-indigo-500 rounded-xl px-3 py-2 text-xs text-zinc-100 outline-none transition"
              />
            </div>
          </div>

          <div v-else-if="configData.llm_provider === 'openai'" class="space-y-3">
            <div>
              <label class="block text-xs font-medium text-zinc-300 mb-1">OpenAI API Key</label>
              <input
                type="password"
                v-model="configData.openai_api_key"
                placeholder="sk-proj-..."
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-indigo-500 rounded-xl px-3 py-2 text-xs text-zinc-100 outline-none transition font-mono"
              />
            </div>
            <div>
              <label class="block text-xs font-medium text-zinc-300 mb-1">Nama Model</label>
              <input
                v-model="configData.model_name"
                placeholder="gpt-4o-mini"
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-indigo-500 rounded-xl px-3 py-2 text-xs text-zinc-100 outline-none transition"
              />
            </div>
          </div>

          <div v-else-if="configData.llm_provider === 'ollama'" class="space-y-3">
            <div>
              <label class="block text-xs font-medium text-zinc-300 mb-1">Ollama Base URL</label>
              <input
                v-model="configData.ollama_base_url"
                placeholder="http://localhost:11434"
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-indigo-500 rounded-xl px-3 py-2 text-xs text-zinc-100 outline-none transition font-mono"
              />
            </div>
            <div>
              <label class="block text-xs font-medium text-zinc-300 mb-1">Nama Model Ollama</label>
              <input
                v-model="configData.model_name"
                placeholder="qwen2.5:7b / llama3.2"
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-indigo-500 rounded-xl px-3 py-2 text-xs text-zinc-100 outline-none transition"
              />
            </div>
          </div>

          <div v-else-if="configData.llm_provider === 'custom'" class="space-y-3">
            <div>
              <label class="block text-xs font-medium text-zinc-300 mb-1">Endpoint API (OpenAI Compatible)</label>
              <input
                v-model="configData.custom_api_endpoint"
                placeholder="https://api.deepseek.com/v1"
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-indigo-500 rounded-xl px-3 py-2 text-xs text-zinc-100 outline-none transition font-mono"
              />
            </div>
            <div>
              <label class="block text-xs font-medium text-zinc-300 mb-1">API Key</label>
              <input
                type="password"
                v-model="configData.custom_api_key"
                placeholder="sk-..."
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-indigo-500 rounded-xl px-3 py-2 text-xs text-zinc-100 outline-none transition font-mono"
              />
            </div>
            <div>
              <label class="block text-xs font-medium text-zinc-300 mb-1">Nama Model</label>
              <input
                v-model="configData.model_name"
                placeholder="deepseek-chat"
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-indigo-500 rounded-xl px-3 py-2 text-xs text-zinc-100 outline-none transition"
              />
            </div>
          </div>
        </section>

        <!-- ── TAB AUDIO & TTS ── -->
        <section v-else-if="activeTab === 'voice'" class="space-y-6 max-w-2xl">
          <div>
            <h2 class="text-sm font-bold text-zinc-100 mb-1">Sistem Suara & Text-to-Speech</h2>
            <p class="text-xs text-zinc-400">Atur suara asisten yang berbicara saat merespons obrolan.</p>
          </div>

          <div>
            <label class="block text-xs font-medium text-zinc-300 mb-2">Engine Suara (TTS)</label>
            <div class="grid grid-cols-3 gap-2">
              <button
                v-for="eng in [
                  { id: 'edge_tts', name: 'Microsoft Edge-TTS', desc: 'Natural & Gratis (Online)' },
                  { id: 'elevenlabs', name: 'ElevenLabs', desc: 'Kualitas AI Ultra-Realistis' },
                  { id: 'piper', name: 'Piper / Kokoro ONNX', desc: 'Suara Lokal (Offline)' },
                ]"
                :key="eng.id"
                type="button"
                @click="configData.tts_engine = eng.id"
                class="p-3 rounded-xl border text-left transition flex flex-col justify-between"
                :class="configData.tts_engine === eng.id ? 'border-indigo-500 bg-indigo-500/10 text-zinc-100' : 'border-zinc-800 bg-zinc-900/60 text-zinc-400 hover:bg-zinc-850'"
              >
                <div class="text-xs font-semibold">{{ eng.name }}</div>
                <div class="text-[10px] text-zinc-500 mt-0.5">{{ eng.desc }}</div>
              </button>
            </div>
          </div>

          <!-- Edge TTS Config -->
          <div v-if="configData.tts_engine === 'edge_tts'" class="space-y-3">
            <div>
              <label class="block text-xs font-medium text-zinc-300 mb-1">Pilihan Suara Edge-TTS</label>
              <select
                v-model="configData.edge_tts_voice"
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-indigo-500 rounded-xl px-3 py-2 text-xs text-zinc-100 outline-none transition"
              >
                <option value="id-ID-GadisNeural">Indonesia - Gadis (Ramah & Hangat)</option>
                <option value="id-ID-ArdiNeural">Indonesia - Ardi (Pria Natural)</option>
                <option value="ja-JP-NanamiNeural">Jepang - Nanami (Anime Chibi)</option>
                <option value="ja-JP-AoiNeural">Jepang - Aoi (Manis Ceria)</option>
                <option value="en-US-AnaNeural">English - Ana (Ceria Kasual)</option>
                <option value="en-US-JennyNeural">English - Jenny (Natural)</option>
              </select>
            </div>
            <div>
              <label class="block text-xs font-medium text-zinc-300 mb-1">Kecepatan Bicara (Rate)</label>
              <input
                v-model="configData.edge_tts_rate"
                placeholder="+10% atau +0%"
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-indigo-500 rounded-xl px-3 py-2 text-xs text-zinc-100 outline-none transition font-mono"
              />
            </div>
          </div>

          <!-- ElevenLabs Config -->
          <div v-else-if="configData.tts_engine === 'elevenlabs'" class="space-y-3">
            <div>
              <label class="block text-xs font-medium text-zinc-300 mb-1">ElevenLabs API Key</label>
              <input
                type="password"
                v-model="configData.elevenlabs_api_key"
                placeholder="xi-api-key..."
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-indigo-500 rounded-xl px-3 py-2 text-xs text-zinc-100 outline-none transition font-mono"
              />
            </div>
            <div>
              <label class="block text-xs font-medium text-zinc-300 mb-1">Voice ID</label>
              <input
                v-model="configData.elevenlabs_voice_id"
                placeholder="21m00Tcm4TlvDq8ikWAM"
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-indigo-500 rounded-xl px-3 py-2 text-xs text-zinc-100 outline-none transition font-mono"
              />
            </div>
          </div>

          <!-- Piper TTS Config -->
          <div v-else-if="configData.tts_engine === 'piper'" class="space-y-3">
            <div>
              <label class="block text-xs font-medium text-zinc-300 mb-1">Path Model Piper (.onnx)</label>
              <input
                v-model="configData.tts_model_path"
                placeholder="~/.local/share/piper/mommy_lessac.onnx"
                class="w-full bg-zinc-900 border border-zinc-700/80 focus:border-indigo-500 rounded-xl px-3 py-2 text-xs text-zinc-100 outline-none transition font-mono"
              />
              <p class="text-[10px] text-zinc-500 mt-1">
                Model suara wanita dewasa lokal (Mommy vibe: en_US-lessac-high). 100% offline di mesin lokal.
              </p>
            </div>
          </div>

          <!-- Test Voice Button -->
          <div class="pt-2">
            <button
              @click="testVoice"
              :disabled="testVoiceLoading"
              class="bg-zinc-800 hover:bg-zinc-700 text-zinc-200 text-xs font-medium px-4 py-2 rounded-xl border border-zinc-700 transition flex items-center gap-2"
            >
              <span>{{ testVoiceLoading ? 'Memutar...' : '▶ Uji Coba Suara Sekarang' }}</span>
            </button>
          </div>
        </section>

        <!-- ── TAB PRESETS KARAKTER ── -->
        <section v-else-if="activeTab === 'presets'" class="space-y-6 max-w-3xl">
          <div class="flex items-center justify-between">
            <div>
              <h2 class="text-sm font-bold text-zinc-100 mb-1">Galeri Preset Avatar</h2>
              <p class="text-xs text-zinc-400">Pilih, buat baru, atau impor preset karakter dalam format ZIP.</p>
            </div>
            <div class="flex items-center gap-2">
              <button
                @click="showNewPresetModal = true"
                class="bg-zinc-800 hover:bg-zinc-700 text-zinc-200 text-xs font-medium px-3 py-1.5 rounded-xl border border-zinc-700 transition"
              >
                + Buat Preset Baru
              </button>
              <button
                @click="triggerZipUpload"
                class="bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-medium px-3 py-1.5 rounded-xl transition shadow"
              >
                Unggah ZIP
              </button>
              <input ref="zipFileInput" type="file" accept=".zip" class="hidden" @change="handleZipUpload" />
            </div>
          </div>

          <!-- Modal Baru -->
          <div v-if="showNewPresetModal" class="p-4 rounded-xl bg-zinc-900 border border-zinc-700 space-y-3">
            <h3 class="text-xs font-semibold text-zinc-200">Nama Karakter Baru</h3>
            <div class="flex gap-2">
              <input
                v-model="newPresetName"
                placeholder="Contoh: Megumin / Rem / Kurisu"
                class="flex-1 bg-zinc-950 border border-zinc-700 rounded-xl px-3 py-2 text-xs outline-none"
                @keydown.enter="createNewPreset"
              />
              <button @click="createNewPreset" class="bg-indigo-600 text-white text-xs font-medium px-4 py-2 rounded-xl">
                Buat
              </button>
              <button @click="showNewPresetModal = false" class="bg-zinc-800 text-zinc-400 text-xs px-3 py-2 rounded-xl">
                Batal
              </button>
            </div>
          </div>

          <!-- Grid Presets -->
          <div class="grid grid-cols-3 gap-4">
            <div
              v-for="preset in avatarPresets"
              :key="preset.id"
              class="bg-zinc-900/60 border rounded-2xl p-4 flex flex-col justify-between transition hover:border-zinc-700 relative"
              :class="configData.active_avatar_preset === preset.id ? 'border-indigo-500 ring-1 ring-indigo-500/30' : 'border-zinc-800'"
            >
              <div class="w-full h-36 bg-zinc-950 rounded-xl flex items-center justify-center overflow-hidden mb-3 border border-zinc-800/80">
                <img v-if="preset.thumbnail" :src="preset.thumbnail" class="w-full h-full object-contain p-2" />
                <span v-else class="text-3xl">🌸</span>
              </div>
              <div class="mb-3">
                <div class="font-bold text-xs text-zinc-100 truncate">{{ preset.name }}</div>
                <div class="text-[10px] text-zinc-500 mt-0.5">
                  {{ preset.id === 'default' ? 'Built-in SVG Anime' : `${preset.frame_count}/8 frame terisi` }}
                </div>
              </div>
              <div class="flex items-center gap-2 pt-2 border-t border-zinc-800">
                <button
                  @click="selectAvatarPreset(preset.id)"
                  class="flex-1 py-1.5 rounded-lg text-xs font-semibold transition"
                  :class="configData.active_avatar_preset === preset.id ? 'bg-emerald-500 text-zinc-950' : 'bg-zinc-800 hover:bg-zinc-700 text-zinc-200'"
                >
                  {{ configData.active_avatar_preset === preset.id ? 'Aktif' : 'Pilih' }}
                </button>
                <button
                  v-if="preset.id !== 'default'"
                  @click="deletePreset(preset.id)"
                  class="px-2 py-1.5 rounded-lg hover:bg-rose-500/20 text-zinc-500 hover:text-rose-400 transition text-xs"
                >
                  🗑
                </button>
              </div>
            </div>
          </div>
        </section>

        <!-- ── TAB SPRITE STUDIO ── -->
        <section v-else-if="activeTab === 'avatar'" class="space-y-6 max-w-3xl">
          <div class="flex items-center justify-between">
            <div>
              <h2 class="text-sm font-bold text-zinc-100 mb-1">Sprite Studio (8 Emosi Karakter)</h2>
              <p class="text-xs text-zinc-400">Unggah sprite gambar transparan (PNG/WebP) untuk masing-masing ekspresi.</p>
            </div>
            <button
              @click="resetAvatarPack"
              class="text-xs text-rose-400 hover:text-rose-300 bg-rose-500/10 border border-rose-500/20 px-3 py-1.5 rounded-xl transition"
            >
              Reset ke Default
            </button>
          </div>

          <div class="grid grid-cols-4 gap-3">
            <div
              v-for="frame in avatarFrameList"
              :key="frame.key"
              class="bg-zinc-900/60 border border-zinc-800 hover:border-zinc-700 rounded-2xl p-3 flex flex-col items-center gap-2 relative group"
            >
              <div class="w-full h-28 bg-zinc-950 rounded-xl flex items-center justify-center overflow-hidden border border-zinc-800/80">
                <img v-if="customAvatarPack[frame.key]" :src="customAvatarPack[frame.key]" class="w-full h-full object-contain p-1" />
                <span v-else class="text-zinc-600 text-[10px] font-mono">Kosong</span>
              </div>
              <div class="text-center w-full">
                <div class="text-xs font-semibold text-zinc-200 truncate">{{ frame.label }}</div>
                <div class="text-[9px] text-zinc-500 truncate">{{ frame.desc }}</div>
              </div>
              <label class="w-full text-center bg-zinc-800 hover:bg-zinc-700 text-zinc-200 text-[10px] py-1.5 rounded-lg cursor-pointer transition font-medium">
                Pilih Berkas
                <input type="file" accept="image/*" class="hidden" @change="(e) => handleSpriteUpload(e, frame.key)" />
              </label>
            </div>
          </div>
        </section>

        <!-- ── TAB KLONING SUARA ── -->
        <section v-else-if="activeTab === 'clone'" class="space-y-6 max-w-2xl">
          <div class="flex items-center justify-between">
            <div>
              <h2 class="text-sm font-bold text-zinc-100 mb-1">Studio Kloning Suara</h2>
              <p class="text-xs text-zinc-400">Unggah sampel audio suara (.mp3/.wav) untuk membuat profil suara asisten kustom.</p>
            </div>
            <button
              @click="triggerVoiceUpload"
              :disabled="isCloningVoice"
              class="bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-medium px-4 py-2 rounded-xl transition shadow"
            >
              {{ isCloningVoice ? 'Memproses...' : '+ Unggah Sampel Suara' }}
            </button>
            <input ref="voiceUploadInput" type="file" accept="audio/*" class="hidden" @change="handleVoiceUpload" />
          </div>

          <div v-if="clonedVoices.length === 0" class="p-8 text-center bg-zinc-900/40 rounded-2xl border border-zinc-800 text-zinc-500 text-xs">
            Belum ada profil suara yang dikloning. Klik "+ Unggah Sampel Suara" untuk mulai.
          </div>

          <div v-else class="space-y-2">
            <div
              v-for="voice in clonedVoices"
              :key="voice.id"
              class="p-4 rounded-xl bg-zinc-900/60 border border-zinc-800 flex items-center justify-between"
            >
              <div>
                <div class="text-xs font-semibold text-zinc-200">{{ voice.name }}</div>
                <div class="text-[10px] text-zinc-500">Engine: {{ voice.engine }} • ID: {{ voice.remote_voice_id }}</div>
              </div>
              <button
                @click="deleteClonedVoice(voice.id)"
                class="text-xs text-rose-400 hover:text-rose-300 p-2"
              >
                Hapus
              </button>
            </div>
          </div>
        </section>

        <!-- ── TAB TEST CHAT ── -->
        <section v-else-if="activeTab === 'chat'" class="flex flex-col h-full max-w-2xl space-y-4">
          <div>
            <h2 class="text-sm font-bold text-zinc-100 mb-1">Uji Coba Obrolan Langsung</h2>
            <p class="text-xs text-zinc-400">Tes respons model AI dan suara TTS secara langsung tanpa membuka overlay.</p>
          </div>

          <!-- Chat History -->
          <div class="flex-1 bg-zinc-900/50 border border-zinc-800 rounded-2xl p-4 overflow-y-auto space-y-3 custom-scrollbar min-h-[300px]">
            <div v-if="testChatHistory.length === 0" class="text-center text-xs text-zinc-500 py-12">
              Ketik pesan di bawah untuk menguji respons {{ configData.assistant_name || 'Aoi' }}.
            </div>
            <div
              v-for="(chat, i) in testChatHistory"
              :key="i"
              class="flex flex-col text-xs"
              :class="chat.role === 'user' ? 'items-end' : 'items-start'"
            >
              <span class="text-[9px] text-zinc-500 mb-0.5">{{ chat.role === 'user' ? 'Kamu' : (configData.assistant_name || 'Aoi') }}</span>
              <div
                class="px-3.5 py-2 rounded-xl max-w-[80%] leading-relaxed select-text"
                :class="chat.role === 'user' ? 'bg-indigo-600 text-white rounded-br-none' : 'bg-zinc-800 text-zinc-100 rounded-bl-none'"
              >
                {{ chat.text }}
              </div>
            </div>
          </div>

          <!-- Input Bar -->
          <div class="flex gap-2">
            <input
              v-model="testChatInput"
              @keydown.enter="sendTestChat"
              :disabled="isTestingChat"
              placeholder="Ketik pesan untuk dites..."
              class="flex-1 bg-zinc-900 border border-zinc-700/80 focus:border-indigo-500 rounded-xl px-4 py-2.5 text-xs text-zinc-100 outline-none transition"
            />
            <button
              @click="sendTestChat"
              :disabled="isTestingChat || !testChatInput.trim()"
              class="bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 text-white font-semibold text-xs px-5 py-2.5 rounded-xl transition shadow"
            >
              {{ isTestingChat ? '...' : 'Kirim' }}
            </button>
          </div>
        </section>
      </main>
    </div>
  </div>
</template>
