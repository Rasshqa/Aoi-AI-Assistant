<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow, LogicalSize } from '@tauri-apps/api/window';
import type { AssistantState, StatePayload, AppConfig, AvatarPresetMeta, LipSyncPayload } from '../types';

// State Reaktif
const currentState = ref<AssistantState>('idle');
const message = ref<string>('Halo! Tekan Super+V atau klik untuk mulai.');
const pendingCommand = ref<string | null>(null);
const isMouthOpen = ref(false);
const isBlinking = ref(false);
const showContextMenu = ref(false);
const avatarPresets = ref<AvatarPresetMeta[]>([]);

// Avatar Size
const avatarSize = ref<number>(
  Number(localStorage.getItem('aoi_avatar_size')) || 144
);

const configData = ref<AppConfig>({
  llm_provider: 'gemini',
  gemini_api_key: '',
  groq_api_key: '',
  openai_api_key: '',
  openrouter_api_key: '',
  ollama_base_url: '',
  custom_api_key: '',
  custom_api_endpoint: '',
  model_name: '',
  tts_model_path: '',
  whisper_model_path: '',
  avatar_folder_path: '',
  assistant_name: 'Aoi',
  user_name: 'Kak',
  active_avatar_preset: 'default',
});

// Custom Avatar Pack
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

// Active Custom Frame (PNG/WebP jika ada, fallback ke SVG Chibi)
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

// Drag Handler
let isMouseDownOnAvatar = false;
let avatarDragStarted = false;
let avatarMouseDownPos = { x: 0, y: 0 };

const handleAvatarMouseDown = (e: MouseEvent) => {
  if (e.button !== 0) return; // Hanya klik kiri
  if ((e.target as HTMLElement)?.closest('button, a, input, [data-no-drag]')) return;

  isMouseDownOnAvatar = true;
  avatarDragStarted = false;
  avatarMouseDownPos = { x: e.screenX, y: e.screenY };

  const onMouseMove = (moveEvent: MouseEvent) => {
    if (!isMouseDownOnAvatar) return;
    const dist = Math.hypot(moveEvent.screenX - avatarMouseDownPos.x, moveEvent.screenY - avatarMouseDownPos.y);
    if (dist > 5 && !avatarDragStarted) {
      avatarDragStarted = true;
      cleanup();
      try {
        getCurrentWindow().startDragging();
      } catch (err) {
        console.warn('Drag error:', err);
      }
    }
  };

  const onMouseUp = () => {
    cleanup();
    isMouseDownOnAvatar = false;
  };

  const cleanup = () => {
    window.removeEventListener('mousemove', onMouseMove);
    window.removeEventListener('mouseup', onMouseUp);
  };

  window.addEventListener('mousemove', onMouseMove);
  window.addEventListener('mouseup', onMouseUp);
};

// Drag-to-Resize Handler
const isResizing = ref(false);

const startResizeDrag = (e: MouseEvent) => {
  if (e.button !== 0) return;
  e.preventDefault();
  e.stopPropagation();

  isResizing.value = true;
  const startMouseX = e.screenX;
  const startMouseY = e.screenY;
  const startW = window.innerWidth;
  const startH = window.innerHeight;
  const aspectRatio = startH / Math.max(startW, 1);

  const onMouseMove = async (moveEvent: MouseEvent) => {
    // Geser ke arah kiri-atas berarti memperbesar ukuran jendela
    const deltaX = startMouseX - moveEvent.screenX;
    const deltaY = startMouseY - moveEvent.screenY;
    const delta = Math.abs(deltaX) > Math.abs(deltaY) ? deltaX : deltaY;

    const newW = Math.max(120, Math.min(1200, Math.round(startW + delta)));
    const newH = Math.max(150, Math.min(1600, Math.round(newW * aspectRatio)));

    avatarSize.value = newW;
    try {
      await getCurrentWindow().setSize(new LogicalSize(newW, newH));
    } catch (err) {
      console.warn('Resize error:', err);
    }
  };

  const onMouseUp = () => {
    isResizing.value = false;
    window.removeEventListener('mousemove', onMouseMove);
    window.removeEventListener('mouseup', onMouseUp);
    const finalW = window.innerWidth;
    avatarSize.value = finalW;
    localStorage.setItem('aoi_avatar_size', String(finalW));
  };

  window.addEventListener('mousemove', onMouseMove);
  window.addEventListener('mouseup', onMouseUp);
};

// Zoom Avatar via Scroll Wheel
const handleAvatarWheel = async (e: WheelEvent) => {
  const delta = e.deltaY < 0 ? 24 : -24;
  const curW = window.innerWidth;
  const curH = window.innerHeight;
  const aspectRatio = curH / Math.max(curW, 1);
  const newW = Math.max(120, Math.min(1200, curW + delta));
  const newH = Math.max(150, Math.min(1600, Math.round(newW * aspectRatio)));

  avatarSize.value = newW;
  try {
    await getCurrentWindow().setSize(new LogicalSize(newW, newH));
    localStorage.setItem('aoi_avatar_size', String(newW));
  } catch (err) {
    console.warn('Wheel resize error:', err);
  }
};

// Buka Pengaturan di Window Terpisah
const openSettingsWindow = async () => {
  showContextMenu.value = false;
  try {
    await invoke('open_settings_window');
  } catch (e) {
    console.error('Gagal membuka jendela pengaturan:', e);
  }
};

// Toggle Mic
const toggleMic = async () => {
  showContextMenu.value = false;
  try {
    await invoke('trigger_mic');
  } catch (e) {
    console.error('Trigger mic failed:', e);
  }
};

// Reset Memori
const clearMemory = async () => {
  showContextMenu.value = false;
  try {
    await invoke('clear_chat_memory');
  } catch (e) {
    console.error('Clear memory failed:', e);
  }
};

// Sembunyikan & Tampilkan Avatar (Double Click)
const isMinimized = ref(false);
let preMinimizeSize = { w: 320, h: 420 };

const hideAvatar = async () => {
  if (isMinimized.value) return;
  preMinimizeSize = {
    w: window.innerWidth || 320,
    h: window.innerHeight || 420,
  };
  isMinimized.value = true;
  showContextMenu.value = false;
  try {
    await invoke('toggle_minimize');
    await getCurrentWindow().setSize(new LogicalSize(110, 42));
  } catch (e) {
    console.warn('Hide avatar error:', e);
  }
};

const restoreAvatar = async () => {
  if (!isMinimized.value) return;
  isMinimized.value = false;
  showContextMenu.value = false;
  try {
    await invoke('toggle_minimize');
    const targetW = Math.max(120, preMinimizeSize.w);
    const targetH = Math.max(160, preMinimizeSize.h);
    await getCurrentWindow().setSize(new LogicalSize(targetW, targetH));
  } catch (e) {
    console.warn('Restore avatar error:', e);
  }
};

const toggleAvatarVisibility = () => {
  if (isMinimized.value) {
    restoreAvatar();
  } else {
    hideAvatar();
  }
};

// Konfirmasi Bash Red Tier
const confirmBash = async () => {
  try {
    await invoke('confirm_pending_command');
  } catch (e) {
    console.error('Confirm bash error:', e);
  }
};

const cancelBash = async () => {
  try {
    await invoke('cancel_pending_command');
  } catch (e) {
    console.error('Cancel bash error:', e);
  }
};

// Ganti Preset
const selectPreset = async (id: string) => {
  showContextMenu.value = false;
  try {
    const pack = await invoke<Record<string, string>>('switch_avatar_preset', { presetId: id });
    customAvatarPack.value = { ...customAvatarPack.value, ...pack };
    configData.value.active_avatar_preset = id;
  } catch (e) {
    console.error('Switch preset failed:', e);
  }
};

// Load Presets & Pack
const loadPresets = async () => {
  try {
    avatarPresets.value = await invoke<AvatarPresetMeta[]>('list_avatar_presets');
    const cfg = await invoke<AppConfig>('get_config');
    configData.value = { ...configData.value, ...cfg };
    const pack = await invoke<Record<string, string>>('load_avatar_pack');
    if (pack) {
      customAvatarPack.value = { ...customAvatarPack.value, ...pack };
    }
  } catch (e) {
    console.warn('Load avatar data warning:', e);
  }
};

// Kedip Mata Otomatis saat Idle
let blinkInterval: ReturnType<typeof setInterval> | null = null;
const setupBlink = () => {
  blinkInterval = setInterval(() => {
    if (currentState.value === 'idle') {
      isBlinking.value = true;
      setTimeout(() => {
        isBlinking.value = false;
      }, 180);
    }
  }, 4200);
};

// Unlisten Listeners
let unlistenState: UnlistenFn | null = null;
let unlistenLipSync: UnlistenFn | null = null;
let unlistenConfig: UnlistenFn | null = null;
let unlistenPreset: UnlistenFn | null = null;
let unlistenSprite: UnlistenFn | null = null;

// Global Keydown (Alt+Y / Alt+N)
const handleKeydown = (e: KeyboardEvent) => {
  if (currentState.value === 'confirm') {
    if (e.altKey && (e.key === 'y' || e.key === 'Y')) {
      e.preventDefault();
      confirmBash();
    } else if (e.altKey && (e.key === 'n' || e.key === 'N')) {
      e.preventDefault();
      cancelBash();
    }
  }
  if (e.key === 'Escape') {
    showContextMenu.value = false;
  }
};

onMounted(async () => {
  window.addEventListener('keydown', handleKeydown);
  setupBlink();
  await loadPresets();

  try {
    const initial = await invoke<StatePayload>('get_current_state');
    currentState.value = initial.state;
    message.value = initial.message;
    pendingCommand.value = initial.pending_command;
    isMinimized.value = initial.minimized;

    unlistenState = await listen<StatePayload>('assistant-state-changed', async (event) => {
      currentState.value = event.payload.state;
      message.value = event.payload.message;
      pendingCommand.value = event.payload.pending_command;
      if (isMinimized.value !== event.payload.minimized) {
        if (event.payload.minimized) {
          preMinimizeSize = {
            w: window.innerWidth || 320,
            h: window.innerHeight || 420,
          };
          isMinimized.value = true;
          try {
            await getCurrentWindow().setSize(new LogicalSize(110, 42));
          } catch (e) {}
        } else {
          isMinimized.value = false;
          try {
            const targetW = Math.max(120, preMinimizeSize.w);
            const targetH = Math.max(160, preMinimizeSize.h);
            await getCurrentWindow().setSize(new LogicalSize(targetW, targetH));
          } catch (e) {}
        }
      }
    });

    unlistenLipSync = await listen<LipSyncPayload>('lip-sync-update', (event) => {
      isMouthOpen.value = event.payload.is_mouth_open;
    });

    unlistenConfig = await listen<AppConfig>('config-updated', (event) => {
      configData.value = { ...configData.value, ...event.payload };
    });

    unlistenPreset = await listen<{ presetId: string; pack: Record<string, string> }>('avatar-preset-changed', (event) => {
      configData.value.active_avatar_preset = event.payload.presetId;
      customAvatarPack.value = { ...customAvatarPack.value, ...event.payload.pack };
    });

    unlistenSprite = await listen<{ frameKey?: string; dataUrl?: string; reset?: boolean }>('avatar-sprite-updated', async (event) => {
      if (event.payload.reset) {
        for (const k of Object.keys(customAvatarPack.value)) {
          customAvatarPack.value[k] = '';
        }
      } else if (event.payload.frameKey && event.payload.dataUrl) {
        customAvatarPack.value[event.payload.frameKey] = event.payload.dataUrl;
      } else {
        await loadPresets();
      }
    });

    // Pulihkan ukuran jendela avatar yang tersimpan
    const savedSize = Number(localStorage.getItem('aoi_avatar_size'));
    if (savedSize && savedSize >= 120 && savedSize <= 1200) {
      avatarSize.value = savedSize;
      const targetW = savedSize;
      const targetH = Math.round(savedSize * 1.35);
      await getCurrentWindow().setSize(new LogicalSize(targetW, targetH));
    }
  } catch (e) {
    console.info('Running in preview mode');
  }
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeydown);
  if (blinkInterval) clearInterval(blinkInterval);
  if (unlistenState) unlistenState();
  if (unlistenLipSync) unlistenLipSync();
  if (unlistenConfig) unlistenConfig();
  if (unlistenPreset) unlistenPreset();
  if (unlistenSprite) unlistenSprite();
});
</script>

<template>
  <div
    class="w-full h-full flex flex-col justify-end items-center select-none overflow-hidden bg-transparent font-sans text-zinc-100"
    @contextmenu.prevent="showContextMenu = !showContextMenu"
    @click="showContextMenu = false"
  >
    <!-- ════════════════════════════════════════════════════════ -->
    <!-- FLOATING BASH CONFIRMATION PILL (Hanya muncul saat Red Tier) -->
    <!-- ════════════════════════════════════════════════════════ -->
    <div
      v-if="currentState === 'confirm' && pendingCommand"
      class="mb-1 p-1.5 rounded-lg bg-zinc-900 border border-purple-500/50 shadow-xl text-[9px] max-w-[180px] flex flex-col gap-1 z-30"
      data-no-drag
      @mousedown.stop
    >
      <div class="text-[8px] font-semibold text-purple-300 flex items-center gap-1">
        <span>⚡</span>
        <span class="truncate">$ {{ pendingCommand }}</span>
      </div>
      <div class="flex gap-1">
        <button
          @click="confirmBash"
          class="flex-1 bg-purple-600 hover:bg-purple-500 text-white font-bold py-0.5 rounded text-[8px] transition"
        >
          [Alt+Y] Ya
        </button>
        <button
          @click="cancelBash"
          class="flex-1 bg-zinc-800 hover:bg-zinc-700 text-zinc-300 py-0.5 rounded text-[8px] transition"
        >
          [Alt+N] Batal
        </button>
      </div>
    </div>

    <!-- ════════════════════════════════════════════════════════ -->
    <!-- MINIMIZED COMPACT DOCK PILL (Muncul saat Avatar di-hide)  -->
    <!-- ════════════════════════════════════════════════════════ -->
    <div
      v-if="isMinimized"
      @click="restoreAvatar"
      title="Klik untuk menampilkan avatar Aoi kembali (atau tekan Super+H)"
      class="w-full h-full flex items-end justify-end p-1 select-none"
    >
      <button
        class="px-2.5 py-1.5 rounded-full bg-zinc-950/90 hover:bg-zinc-900 border border-zinc-700/80 shadow-2xl flex items-center gap-1.5 transition-all duration-200 hover:scale-105 active:scale-95 text-zinc-200 hover:text-white group cursor-pointer"
      >
        <img src="/Aoi.png" alt="Aoi" class="w-4 h-4 rounded-full object-cover border border-indigo-400/60" />
        <span class="text-[10px] font-medium font-sans tracking-wide">Aoi</span>
        <span class="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse"></span>
      </button>
    </div>

    <!-- ════════════════════════════════════════════════════════ -->
    <!-- SPRITE AVATAR 2D DESKTOP (Murni Karakter, Bebas Border)  -->
    <!-- ════════════════════════════════════════════════════════ -->
    <div
      v-else
      @mousedown="handleAvatarMouseDown"
      @dblclick="hideAvatar"
      @wheel.prevent="handleAvatarWheel"
      class="relative group w-full h-full flex items-end justify-center cursor-move aoi-avatar-container flex-shrink-0 select-none bg-transparent"
      :title="'Tahan & geser untuk memindahkan • Klik 2x untuk sembunyikan • Drag sudut kiri atas untuk atur ukuran • Scroll untuk zoom • Klik kanan untuk menu'"
    >
      <!-- Corner Drag-to-Resize Handle (Top-Left) -->
      <div
        @mousedown.stop="startResizeDrag"
        title="Tahan & tarik untuk mengubah ukuran avatar"
        class="absolute top-2 left-2 z-30 opacity-0 group-hover:opacity-90 hover:!opacity-100 transition-opacity cursor-nwse-resize p-1.5 rounded-xl bg-zinc-950/85 hover:bg-zinc-800 text-zinc-300 hover:text-white border border-zinc-700/70 shadow-2xl flex items-center gap-1.5 text-[10px]"
        data-no-drag
      >
        <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7" />
        </svg>
        <span class="text-[9px] font-mono select-none">Ubah Ukuran</span>
      </div>

      <!-- Edge Resize Zones (Top Edge & Left Edge) -->
      <div
        class="absolute top-0 left-0 right-0 h-3 cursor-ns-resize z-20"
        @mousedown.stop="startResizeDrag"
        data-no-drag
        title="Tarik untuk ubah ukuran"
      />
      <div
        class="absolute top-0 bottom-0 left-0 w-3 cursor-ew-resize z-20"
        @mousedown.stop="startResizeDrag"
        data-no-drag
        title="Tarik untuk ubah ukuran"
      />

      <!-- Quick Hover Actions (Gear Icon Buka Pengaturan) -->
      <div
        class="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity z-20 flex items-center gap-1"
        data-no-drag
        @mousedown.stop
      >
        <button
          @click.stop="openSettingsWindow"
          title="Buka Jendela Pengaturan"
          class="w-6 h-6 rounded-full bg-zinc-950/90 hover:bg-zinc-800 text-zinc-300 hover:text-white border border-zinc-700/80 shadow-lg flex items-center justify-center text-xs transition"
        >
          ⚙
        </button>
      </div>

      <!-- Custom PNG / WebP Avatar -->
      <img
        v-if="activeCustomFrame"
        :src="activeCustomFrame"
        alt="Avatar Aoi"
        class="w-full h-full max-w-full max-h-full object-contain pointer-events-none select-none transition-all duration-150 drop-shadow-xl"
      />

      <!-- Built-in Karakter Anime Chibi SVG -->
      <svg
        v-else
        viewBox="0 0 200 200"
        class="w-full h-full max-w-full max-h-full object-contain transition-transform duration-200 pointer-events-none drop-shadow-xl"
        :class="{
          'scale-105': currentState === 'listening',
          'translate-y-[-2px]': currentState === 'speaking' && isMouthOpen,
        }"
      >
        <!-- Dynamic State Aura Glow (Hanya aktif saat non-idle) -->
        <circle
          v-if="currentState !== 'idle'"
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
              : 'transparent'
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

        <!-- Mata Kiri -->
        <g v-if="!isBlinking">
          <ellipse cx="80" cy="98" rx="7" ry="10" fill="#1e1b4b" />
          <ellipse cx="79" cy="95" rx="3.5" ry="5.5" fill="#38bdf8" />
          <circle cx="77" cy="93" r="2.5" fill="#ffffff" />
          <circle cx="82" cy="101" r="1.2" fill="#ffffff" />
        </g>
        <path v-else d="M73 98 Q80 104 87 98" stroke="#1e1b4b" stroke-width="2.5" stroke-linecap="round" fill="none" />

        <!-- Mata Kanan -->
        <g v-if="!isBlinking">
          <ellipse cx="120" cy="98" rx="7" ry="10" fill="#1e1b4b" />
          <ellipse cx="119" cy="95" rx="3.5" ry="5.5" fill="#38bdf8" />
          <circle cx="117" cy="93" r="2.5" fill="#ffffff" />
          <circle cx="122" cy="101" r="1.2" fill="#ffffff" />
        </g>
        <path v-else d="M113 98 Q120 104 127 98" stroke="#1e1b4b" stroke-width="2.5" stroke-linecap="round" fill="none" />

        <!-- Pipi Merona -->
        <ellipse cx="69" cy="107" rx="6" ry="3.5" fill="#f43f5e" opacity="0.4" />
        <ellipse cx="131" cy="107" rx="6" ry="3.5" fill="#f43f5e" opacity="0.4" />

        <!-- Mulut Dinamis (Lip-Sync Real-Time) -->
        <ellipse
          v-if="currentState === 'speaking' && isMouthOpen"
          cx="100"
          cy="114"
          rx="5.5"
          ry="7"
          fill="#be123c"
        />
        <path
          v-else-if="currentState === 'listening'"
          d="M96 112 Q100 117 104 112"
          stroke="#9f1239"
          stroke-width="2"
          stroke-linecap="round"
          fill="none"
        />
        <path
          v-else-if="currentState === 'thinking'"
          d="M97 113 Q100 113 103 113"
          stroke="#9f1239"
          stroke-width="2"
          stroke-linecap="round"
          fill="none"
        />
        <path
          v-else
          d="M96 112 Q100 116 104 112"
          stroke="#9f1239"
          stroke-width="2"
          stroke-linecap="round"
          fill="none"
        />

        <!-- Aksesoris Jepit Rambut -->
        <path d="M54 75 L62 83 M54 83 L62 75" stroke="#f43f5e" stroke-width="2.5" stroke-linecap="round" />
      </svg>
    </div>

    <!-- ════════════════════════════════════════════════════════ -->
    <!-- MINIMALIST CONTEXT MENU (Klik Kanan pada Avatar)          -->
    <!-- ════════════════════════════════════════════════════════ -->
    <div
      v-if="showContextMenu"
      class="fixed bottom-3 right-3 w-48 bg-zinc-950/98 border border-zinc-750 rounded-2xl shadow-2xl p-1.5 z-50 text-[10px] backdrop-blur-xl animate-fade-in flex flex-col gap-0.5"
      data-no-drag
      @mousedown.stop
      @click.stop
    >
      <div class="px-2.5 py-1 text-[8px] font-bold text-zinc-500 uppercase tracking-wider border-b border-zinc-800">
        {{ configData.assistant_name || 'Aoi' }} • Menu
      </div>

      <button
        @click="openSettingsWindow"
        class="w-full text-left px-2.5 py-1.5 rounded-lg text-zinc-200 hover:bg-zinc-800 hover:text-white transition flex items-center gap-2"
      >
        <span>⚙</span>
        <span>Buka Pengaturan</span>
      </button>

      <button
        @click="toggleMic"
        class="w-full text-left px-2.5 py-1.5 rounded-lg text-zinc-200 hover:bg-zinc-800 hover:text-white transition flex items-center gap-2"
      >
        <span>🎤</span>
        <span>{{ currentState === 'listening' ? 'Hentikan Suara' : 'Bicara (Super+V)' }}</span>
      </button>

      <button
        @click="clearMemory"
        class="w-full text-left px-2.5 py-1.5 rounded-lg text-zinc-200 hover:bg-zinc-800 hover:text-white transition flex items-center gap-2"
      >
        <span>🧹</span>
        <span>Bersihkan Ingatan Chat</span>
      </button>

      <!-- Submenu Preset -->
      <div class="border-t border-zinc-850 pt-1">
        <div class="px-2 py-0.5 text-[8px] text-zinc-500 font-semibold uppercase">Pilih Karakter:</div>
        <div class="max-h-24 overflow-y-auto custom-scrollbar flex flex-col gap-0.5">
          <button
            v-for="p in avatarPresets"
            :key="p.id"
            @click="selectPreset(p.id)"
            class="w-full text-left px-2.5 py-1 rounded-lg text-zinc-300 hover:bg-zinc-800 hover:text-white transition flex items-center justify-between"
            :class="configData.active_avatar_preset === p.id ? 'text-indigo-400 font-bold bg-zinc-900' : ''"
          >
            <span class="truncate">{{ p.name }}</span>
            <span v-if="configData.active_avatar_preset === p.id" class="text-indigo-400 text-[9px]">✓</span>
          </button>
        </div>
      </div>

      <div class="border-t border-zinc-850 pt-1">
        <button
          @click="toggleAvatarVisibility"
          class="w-full text-left px-2.5 py-1.5 rounded-lg text-zinc-400 hover:bg-zinc-900 hover:text-rose-300 transition flex items-center gap-2"
        >
          <span>_</span>
          <span>{{ isMinimized ? 'Tampilkan Avatar' : 'Sembunyikan Avatar (Klik 2x)' }}</span>
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
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

@keyframes fade-in {
  from {
    opacity: 0;
    transform: scale(0.95);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}

.animate-fade-in {
  animation: fade-in 0.15s ease-out forwards;
}
</style>
