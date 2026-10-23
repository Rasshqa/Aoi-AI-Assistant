<script setup lang="ts">
import { ref, onMounted, onErrorCaptured } from 'vue';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import AvatarOverlay from './components/AvatarOverlay.vue';
import SettingsWindow from './components/SettingsWindow.vue';

const isSettings = ref(false);
const renderError = ref<string | null>(null);

onErrorCaptured((err) => {
  console.error('App-level error captured:', err);
  renderError.value = String(err);
  return false;
});

const checkWindowMode = () => {
  try {
    const currentWin = getCurrentWebviewWindow();
    if (currentWin && currentWin.label === 'settings') {
      isSettings.value = true;
      return;
    }
  } catch (e) {
    console.warn('getCurrentWebviewWindow check:', e);
  }

  const urlParams = new URLSearchParams(window.location.search);
  isSettings.value =
    urlParams.get('window') === 'settings' ||
    window.location.hash === '#settings' ||
    window.location.href.includes('window=settings') ||
    window.location.href.includes('settings');
};

checkWindowMode();

onMounted(() => {
  checkWindowMode();
  window.addEventListener('hashchange', checkWindowMode);
});
</script>

<template>
  <div v-if="renderError" class="w-full h-full bg-zinc-950 text-red-400 p-6 font-mono text-xs overflow-auto select-text">
    <div class="font-bold text-sm mb-2 text-red-300">Terjadi kesalahan pada antarmuka:</div>
    <pre class="bg-zinc-900 p-4 rounded border border-red-500/30 whitespace-pre-wrap">{{ renderError }}</pre>
  </div>
  <SettingsWindow v-else-if="isSettings" />
  <AvatarOverlay v-else />
</template>

<style>
html,
body,
#app {
  width: 100%;
  height: 100%;
  margin: 0;
  padding: 0;
  overflow: hidden;
  background: transparent;
  font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  user-select: none;
}

input,
textarea,
code,
p {
  user-select: text;
}

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
</style>