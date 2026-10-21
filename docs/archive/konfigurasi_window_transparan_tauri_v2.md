{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "ai-hypr-assistant",
  "version": "0.1.0",
  "identifier": "com.cachyos.ai-hypr-assistant",
  "build": {
    "beforeDevCommand": "npm run dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "npm run build",
    "frontendDist": "../dist"
  },
  "app": {
    "macOSPrivateApi": true,
    "withGlobalTauri": true,
    "windows": [
      {
        "label": "main",
        "title": "AI Waifu Overlay",
        "width": 340,
        "height": 420,
        "resizable": false,
        "decorations": false,
        "transparent": true,
        "alwaysOnTop": true,
        "skipTaskbar": true
      }
    ],
    "security": {
      "csp": null
    }
  }
}