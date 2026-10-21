<!-- STREAMING_CHUNK:Setting up Vue 3 script, reactive state, and animation timers... -->
<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

type AssistantState = 'idle' | 'listening' | 'thinking' | 'speaking' | 'confirm' | 'error';

interface StatePayload {
  state: AssistantState;
  message: string;
  pending_command: string | null;
  minimized: boolean;
}

const currentState = ref<AssistantState>('idle');
const message = ref<string>('Halo! Tekan Super+V untuk mulai ngobrol.');
const pendingCommand = ref<string | null>(null);
const isMinimized = ref<boolean>(false);

// State animasi frame 2D ringan (tanpa WebGL/Live2D)
const isBlinking = ref<boolean>(false);
const isMouthOpen = ref<boolean>(false);
const showDevToolbar = ref<boolean>(true);
const ghostMode = ref<boolean>(false);

// Opsi Kustomisasi Gambar (Bisa diisi path gambar PNG/WebP lokal kamu nanti)
// Saat kosong, otomatis menggunakan built-in Anime Chibi SVG Frame yang super ringan
const customAvatarPack = ref<Record<string, string>>({
  idle_open: '',
  idle_blink: '',
  listening: '',
  thinking: '',
  speak_closed: '',
  speak_open: '',
  confirm: '',
  error: ''
});

let blinkInterval: number | null = null;
let lipSyncInterval: number | null = null;
let unlistenEvent: (() => void) | null = null;

/* STREAMING_CHUNK:Configuring 2D frame timers for blinking and lip-sync... */
// Mengatur kedipan mata otomatis setiap ~4 detik (hanya merender ulang 1 boolean!)
const startBlinkTimer = () => {
  blinkInterval = window.setInterval(() => {
    if (currentState.value === 'idle' || currentState.value === 'listening') {
      isBlinking.value = true;
      setTimeout(() => {
        isBlinking.value = false;
      }, 160);
    }
  }, 4000);
};

// Simulasi pergantian frame mulut saat berbicara (Tahap 3 nanti disambung ke amplitudo audio TTS)
const startLipSyncTimer = () => {
  lipSyncInterval = window.setInterval(() => {
    if (currentState.value === 'speaking') {
      isMouthOpen.value = !isMouthOpen.value;
    } else {
      isMouthOpen.value = false;
    }
  }, 140);
};

/* STREAMING_CHUNK:Connecting Tauri events and window drag handlers... */
// Fungsi untuk menggeser posisi window secara bebas di Hyprland
const startDrag = async () => {
  try {
    await getCurrentWindow().startDragging();
  } catch (e) {
    console.warn('Drag dijalankan di luar Tauri window:', e);
  }
};

// Fungsi trigger manual untuk mengetes semua state langsung dari UI
const triggerState = async (state: AssistantState, msg: string, cmd: string | null = null) => {
  try {
    await invoke('set_manual_state', {
      newState: state,
      message: msg,
      command: cmd
    });
  } catch {
    // Fallback jika dibuka di browser biasa
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

// Menentukan frame gambar eksternal mana yang sedang aktif (jika user memakai PNG/WebP sendiri)
const activeCustomFrame = computed(() => {
  const pack = customAvatarPack.value;
  if (currentState.value === 'idle') {
    return isBlinking.value ? pack.idle_blink : pack.idle_open;
  }
  if (currentState.value === 'speaking') {
    return isMouthOpen.value ? pack.speak_open : pack.speak_closed;
  }
  return pack[currentState.value] || '';
});

// Warna indikator cincin status
const stateBadgeColor = computed(() => {
  switch (currentState.value) {
    case 'listening': return 'bg-emerald-500 shadow-emerald-500/50';
    case 'thinking': return 'bg-amber-500 shadow-amber-500/50';
    case 'speaking': return 'bg-sky-500 shadow-sky-500/50';
    case 'confirm': return 'bg-purple-500 shadow-purple-500/50';
    case 'error': return 'bg-rose-500 shadow-rose-500/50';
    default: return 'bg-slate-400';
  }
});

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
  } catch (e) {
    console.info('Berjalan dalam mode preview web.');
  }
});

onUnmounted(() => {
  if (blinkInterval) clearInterval(blinkInterval);
  if (lipSyncInterval) clearInterval(lipSyncInterval);
  if (unlistenEvent) unlistenEvent();
});
</script>

<!-- STREAMING_CHUNK:Rendering transparent overlay UI, RPG dialog box, and 2D sprite frames... -->
<template>
  <div 
    class="w-screen h-screen overflow-hidden select-none flex flex-col items-center justify-end p-3 transition-opacity duration-300"
    :class="{ 'opacity-35 hover:opacity-100': ghostMode }"
    @contextmenu.prevent="showDevToolbar = !showDevToolbar"
  >
    <!-- MODE MINIMIZE: Hanya tombol pil mengambang kecil di pojok -->
    <div 
      v-if="isMinimized" 
      @mousedown="startDrag"
      class="flex items-center gap-2 bg-slate-900/90 backdrop-blur-md border border-slate-700/80 px-3.5 py-2 rounded-full shadow-lg cursor-move"
    >
      <span class="w-2.5 h-2.5 rounded-full animate-pulse" :class="stateBadgeColor"></span>
      <span class="text-xs font-medium text-slate-200 uppercase tracking-wider">{{ currentState }}</span>
      <button 
        @click.stop="toggleMinimize" 
        class="ml-1 text-xs text-sky-400 hover:text-sky-300 font-semibold"
      >
        Buka
      </button>
    </div>

    <!-- MODE UTAMA: Balon Dialog RPG + Karakter 2D + Panel Kontrol -->
    <div v-else class="w-full max-w-[320px] flex flex-col items-center">
      
      <!-- BALON DIALOG & KONFIRMASI BASH (RPG Style) -->
      <div 
        class="w-full bg-slate-900/90 backdrop-blur-md border border-slate-700/80 rounded-2xl p-3 mb-2 shadow-xl text-slate-100 relative transition-all"
      >
        <!-- Header Status Kecil + Tombol Geser & Minimize -->
        <div 
          @mousedown="startDrag"
          class="flex items-center justify-between border-b border-slate-800 pb-1.5 mb-2 cursor-move"
        >
          <div class="flex items-center gap-1.5">
            <span class="w-2 h-2 rounded-full shadow-sm" :class="stateBadgeColor"></span>
            <span class="text-[10px] font-bold uppercase tracking-widest text-slate-400">
              Aoi • {{ currentState }}
            </span>
          </div>
          <div class="flex items-center gap-1.5">
            <button 
              @click.stop="ghostMode = !ghostMode" 
              title="Toggle Ghost Mode (Transparan)"
              class="text-[10px] px-1.5 py-0.5 rounded bg-slate-800 hover:bg-slate-700 text-slate-300"
            >
              👻
            </button>
            <button 
              @click.stop="toggleMinimize" 
              title="Minimize (Super+H)"
              class="text-[10px] px-1.5 py-0.5 rounded bg-slate-800 hover:bg-slate-700 text-slate-300"
            >
              _
            </button>
          </div>
        </div>

        <!-- Teks Ucapan / Status AI -->
        <p class="text-xs leading-relaxed text-slate-200">
          {{ message }}
        </p>

        <!-- Kotak Konfirmasi Eksekusi Perintah Bash (Red Tier - Opsi B) -->
        <div 
          v-if="currentState === 'confirm' && pendingCommand" 
          class="mt-2.5 p-2 rounded-lg bg-slate-950/90 border border-purple-500/40"
        >
          <div class="text-[10px] text-purple-300 font-semibold mb-1">
            ⚠️ Konfirmasi Perintah Terminal:
          </div>
          <code class="block text-[11px] font-mono text-emerald-400 bg-black/50 px-2 py-1 rounded break-all">
            $ {{ pendingCommand }}
          </code>
          <div class="flex gap-2 mt-2">
            <button 
              @click="triggerState('speaking', 'Menjalankan perintah: ' + pendingCommand)"
              class="flex-1 bg-emerald-600 hover:bg-emerald-500 text-white text-[10px] font-bold py-1 rounded transition"
            >
              [Alt+Y] Setuju
            </button>
            <button 
              @click="triggerState('idle', 'Perintah dibatalkan.')"
              class="flex-1 bg-rose-600/80 hover:bg-rose-600 text-white text-[10px] font-bold py-1 rounded transition"
            >
              [Alt+N] Batal
            </button>
          </div>
        </div>
      </div>

      <!-- AREA SPRITE AVATAR 2D (Bisa digeser dengan klik & tahan badan karakter) -->
      <div 
        @mousedown="startDrag"
        class="relative w-44 h-44 flex items-center justify-center cursor-move group"
      >
        <!-- Jika user sudah memasukkan file PNG/WebP di folder karakter -->
        <img 
          v-if="activeCustomFrame" 
          :src="activeCustomFrame" 
          alt="AI Avatar" 
          class="w-full h-full object-contain pointer-events-none select-none drop-shadow-2xl"
        />

        <!-- Default Built-in 2D Anime Avatar Frame (SVG Sprite Ringan) -->
        <svg 
          v-else 
          viewBox="0 0 200 200" 
          class="w-full h-full drop-shadow-xl transition-transform duration-200"
          :class="{
            'scale-105': currentState === 'listening',
            'animate-bounce': currentState === 'speaking' && isMouthOpen
          }"
        >
          <!-- Aura Belakang Sesuai State -->
          <circle 
            cx="100" cy="105" r="72" 
            class="transition-colors duration-300"
            :fill="currentState === 'listening' ? 'rgba(16, 185, 129, 0.22)' : 
                   currentState === 'thinking' ? 'rgba(245, 158, 11, 0.22)' : 
                   currentState === 'error' ? 'rgba(244, 63, 94, 0.25)' : 'rgba(56, 189, 248, 0.15)'"
          />

          <!-- Rambut Belakang -->
          <path d="M45 90 Q35 150 55 175 L145 175 Q165 150 155 90 Z" fill="#312e81" />

          <!-- Tubuh / Hoodie -->
          <path d="M60 150 Q100 140 140 150 L150 190 L50 190 Z" fill="#4f46e5" />
          <path d="M85 150 L100 175 L115 150" fill="#e0e7ff" />

          <!-- Wajah -->
          <circle cx="100" cy="102" r="46" fill="#ffedd5" />

          <!-- Rambut Depan / Poni Anime -->
          <path d="M52 95 Q55 50 100 50 Q145 50 148 95 Q130 70 115 90 Q100 68 85 90 Q70 70 52 95 Z" fill="#4338ca" />

          <!-- Telinga Kucing / Headset -->
          <polygon points="55,68 42,40 75,55" fill="#6366f1" />
          <polygon points="145,68 158,40 125,55" fill="#6366f1" />

          <!-- MATA (Berubah sesuai State & Blink Frame) -->
          <!-- Frame: Error (Mata X X) -->
          <g v-if="currentState === 'error'" stroke="#e11d48" stroke-width="3.5" stroke-linecap="round">
            <line x1="72" y1="96" x2="86" y2="110" />
            <line x1="86" y1="96" x2="72" y2="110" />
            <line x1="114" y1="96" x2="128" y2="110" />
            <line x1="128" y1="96" x2="114" y2="110" />
          </g>

          <!-- Frame: Berkedip (Mata Tertutup) -->
          <g v-else-if="isBlinking" stroke="#1e1b4b" stroke-width="3" fill="none" stroke-linecap="round">
            <path d="M70 104 Q79 110 88 104" />
            <path d="M112 104 Q121 110 130 104" />
          </g>

          <!-- Frame: Thinking (Mata Melihat ke Atas) -->
          <g v-else-if="currentState === 'thinking'">
            <ellipse cx="79" cy="100" rx="7" ry="8" fill="#1e1b4b" />
            <ellipse cx="121" cy="100" rx="7" ry="8" fill="#1e1b4b" />
            <circle cx="79" cy="96" r="3" fill="#ffffff" />
            <circle cx="121" cy="96" r="3" fill="#ffffff" />
          </g>

          <!-- Frame: Normal / Listening / Speaking (Mata Terbuka) -->
          <g v-else>
            <ellipse cx="79" cy="103" rx="7.5" ry="9" fill="#1e1b4b" />
            <ellipse cx="121" cy="103" rx="7.5" ry="9" fill="#1e1b4b" />
            <circle cx="76" cy="99" r="3" fill="#ffffff" />
            <circle cx="118" cy="99" r="3" fill="#ffffff" />
          </g>

          <!-- Pipi Merona -->
          <ellipse cx="66" cy="115" rx="6" ry="3" fill="#fda4af" opacity="0.7" />
          <ellipse cx="134" cy="115" rx="6" ry="3" fill="#fda4af" opacity="0.7" />

          <!-- MULUT (Berganti Frame saat Speaking / State lain) -->
          <!-- Mulut Terbuka (Speaking Frame 2) -->
          <ellipse 
            v-if="currentState === 'speaking' && isMouthOpen" 
            cx="100" cy="124" rx="7" ry="6" 
            fill="#f43f5e" 
          />
          <!-- Mulut Terkejut / Error -->
          <ellipse 
            v-else-if="currentState === 'error' || currentState === 'confirm'" 
            cx="100" cy="125" rx="5" ry="5" 
            fill="none" stroke="#1e1b4b" stroke-width="2.5" 
          />
          <!-- Mulut Tertutup / Tersenyum (Idle / Speaking Frame 1) -->
          <path 
            v-else 
            d="M93 122 Q100 128 107 122" 
            fill="none" stroke="#1e1b4b" stroke-width="2.5" stroke-linecap="round" 
          />

          <!-- Ikon Tambahan Sesuai State -->
          <!-- Indikator Mic Menyala (Listening) -->
          <g v-if="currentState === 'listening'">
            <circle cx="155" cy="65" r="14" fill="#10b981" />
            <rect x="151" y="56" width="8" height="12" rx="4" fill="white" />
            <path d="M148 64 Q155 73 162 64" fill="none" stroke="white" stroke-width="2" />
          </g>

          <!-- Indikator Balon Pikiran (Thinking) -->
          <g v-if="currentState === 'thinking'">
            <circle cx="152" cy="75" r="4" fill="#fbbf24" />
            <circle cx="162" cy="63" r="6" fill="#fbbf24" />
            <circle cx="172" cy="48" r="10" fill="#fbbf24" />
          </g>
        </svg>
      </div>

      <!-- PANEL TESTER STATE (Klik kanan pada avatar untuk menyembunyikan/memunculkan panel ini) -->
      <div 
        v-if="showDevToolbar" 
        class="w-full bg-slate-900/95 border border-slate-700/80 rounded-xl p-2 mt-1 shadow-lg"
      >
        <div class="text-[9px] text-slate-400 text-center mb-1.5">
          🛠️ Panel Uji Coba State (Klik Kanan untuk Sembunyikan)
        </div>
        <div class="grid grid-cols-3 gap-1">
          <button 
            @click="triggerState('idle', 'Standby... Tekan Super+V untuk bicara.')"
            class="text-[10px] py-1 rounded bg-slate-800 hover:bg-slate-700 text-slate-200"
          >
            Idle
          </button>
          <button 
            @click="triggerState('listening', 'Mendengarkan suaramu...')"
            class="text-[10px] py-1 rounded bg-emerald-900/60 hover:bg-emerald-800/80 text-emerald-200"
          >
            Listening
          </button>
          <button 
            @click="triggerState('thinking', 'Sedang memikirkan jawaban...')"
            class="text-[10px] py-1 rounded bg-amber-900/60 hover:bg-amber-800/80 text-amber-200"
          >
            Thinking
          </button>
          <button 
            @click="triggerState('speaking', 'Halo! Aku sedang berbicara dengan lip-sync 2 frame!')"
            class="text-[10px] py-1 rounded bg-sky-900/60 hover:bg-sky-800/80 text-sky-200"
          >
            Speaking
          </button>
          <button 
            @click="triggerState('confirm', 'Izinkan saya menghapus cache paket?', 'sudo pacman -Scc')"
            class="text-[10px] py-1 rounded bg-purple-900/60 hover:bg-purple-800/80 text-purple-200"
          >
            Confirm
          </button>
          <button 
            @click="triggerState('error', 'Waduh! Koneksi ke server LLM terputus.')"
            class="text-[10px] py-1 rounded bg-rose-900/60 hover:bg-rose-800/80 text-rose-200"
          >
            Error
          </button>
        </div>
      </div>

    </div>
  </div>
</template>

<style>
/* Pastikan latar belakang dokumen 100% transparan untuk Wayland / Hyprland */
html, body, #app {
  background: transparent !important;
  margin: 0;
  padding: 0;
  overflow: hidden;
}
</style>