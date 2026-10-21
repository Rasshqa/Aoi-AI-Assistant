# STREAMING_CHUNK:Configuring Rust dependencies and Tauri features...
[package]
name = "ai-hypr-assistant"
version = "0.1.0"
description = "Lightweight 2D AI Desktop Assistant for CachyOS + Hyprland"
authors = ["User"]
edition = "2021"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = ["macos-private-api", "tray-icon"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }