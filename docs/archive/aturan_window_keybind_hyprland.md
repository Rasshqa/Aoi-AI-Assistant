# ============================================================================
# Tambahkan baris berikut ke dalam ~/.config/hypr/hyprland.conf milikmu
# ============================================================================

# 1. Window Rules agar Overlay Transparan, Floating, dan Sticky di semua Workspace
windowrulev2 = float, title:^(AI Waifu Overlay)$
windowrulev2 = pin, title:^(AI Waifu Overlay)$
windowrulev2 = noborder, title:^(AI Waifu Overlay)$
windowrulev2 = noshadow, title:^(AI Waifu Overlay)$
windowrulev2 = noblur, title:^(AI Waifu Overlay)$
windowrulev2 = move 100%-360 100%-450, title:^(AI Waifu Overlay)$ # Pojok kanan bawah

# 2. Smart Keybind (Super + V) untuk Mulai Bicara / Stop / Menyela Pembicaraan (Barge-In)
bind = $mainMod, V, exec, echo "toggle_mic" | socat - UNIX-CONNECT:/tmp/ai-assistant.sock

# 3. Keybind (Super + H) untuk Minimize / Sembunyikan Avatar
bind = $mainMod, H, exec, echo "toggle_hide" | socat - UNIX-CONNECT:/tmp/ai-assistant.sock

# 4. Keybind Konfirmasi Cepat Perintah Bash [Alt + Y / Alt + N]
bind = ALT, Y, exec, echo "confirm_yes" | socat - UNIX-CONNECT:/tmp/ai-assistant.sock
bind = ALT, N, exec, echo "confirm_no" | socat - UNIX-CONNECT:/tmp/ai-assistant.sock