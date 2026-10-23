#!/usr/bin/env bash
# ==============================================================================
# Aoi (AI Hypr-Assistant) - Setup & Launcher Helper
# Untuk Arch Linux / CachyOS + Hyprland (Wayland)
# ==============================================================================

set -e

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_PATH="$PROJECT_DIR/src-tauri/target/release/ai-hypr-assistant"
DESKTOP_DIR="$HOME/.local/share/applications"
CONFIG_DIR="$HOME/.config/aoi"

echo "🌸 Menyiapkan Aoi (AI Hypr-Assistant)..."

# 1. Pastikan direktori config ada
mkdir -p "$CONFIG_DIR"
mkdir -p "$DESKTOP_DIR"
mkdir -p "$HOME/.local/bin"
mkdir -p "$HOME/.local/share/icons/hicolor/512x512/apps"
mkdir -p "$HOME/.local/share/pixmaps"

# 2. Pasang icon aplikasi & .desktop launcher
if [ -f "$PROJECT_DIR/public/Aoi.png" ]; then
    cp "$PROJECT_DIR/public/Aoi.png" "$HOME/.local/share/icons/hicolor/512x512/apps/ai-hypr-assistant.png"
    cp "$PROJECT_DIR/public/Aoi.png" "$HOME/.local/share/pixmaps/ai-hypr-assistant.png"
    echo "✅ Ikon aplikasi dipasang di ~/.local/share/icons & ~/.local/share/pixmaps"
fi
cp "$PROJECT_DIR/com.rasshqa.ai-hypr-assistant.desktop" "$DESKTOP_DIR/"

# 3. Buat symlink binary ke ~/.local/bin jika sudah di-build
if [ -f "$BIN_PATH" ]; then
    ln -sf "$BIN_PATH" "$HOME/.local/bin/ai-hypr-assistant"
    echo "✅ Symlink binary dipasang di ~/.local/bin/ai-hypr-assistant"
else
    echo "ℹ️ Binary rilis belum di-build. Kamu bisa menjalankan 'npm run tauri dev' untuk pengujian,"
    echo "   atau jalankan 'npm run tauri build' untuk menghasilkan binary rilis."
fi

echo ""
echo "✨ Pengaturan Hyprland:"
echo "Tambahkan baris berikut ke ~/.config/hypr/hyprland.conf jika belum ada:"
echo "----------------------------------------------------------------------"
cat << 'EOF'
# Window Rules Overlay Aoi
windowrulev2 = float, title:^(AI Waifu Overlay)$
windowrulev2 = pin, title:^(AI Waifu Overlay)$
windowrulev2 = noborder, title:^(AI Waifu Overlay)$
windowrulev2 = noshadow, title:^(AI Waifu Overlay)$
windowrulev2 = noblur, title:^(AI Waifu Overlay)$
windowrulev2 = move 100%-360 100%-490, title:^(AI Waifu Overlay)$

# Keybind Interaksi Aoi
bind = $mainMod, V, exec, echo "toggle_mic" | socat - UNIX-CONNECT:/tmp/ai-assistant.sock
bind = $mainMod, H, exec, echo "toggle_hide" | socat - UNIX-CONNECT:/tmp/ai-assistant.sock
bind = ALT, Y, exec, echo "confirm_yes" | socat - UNIX-CONNECT:/tmp/ai-assistant.sock
bind = ALT, N, exec, echo "confirm_no" | socat - UNIX-CONNECT:/tmp/ai-assistant.sock
EOF
echo "----------------------------------------------------------------------"
echo "🎉 Selesai! Jalankan 'npm run tauri dev' untuk mulai!"
