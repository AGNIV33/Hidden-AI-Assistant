# Peek AI

A light, free Windows desktop AI assistant for meetings. Its windows are hidden from screen share.

## Features
- Chat with any OpenAI-compatible AI (Groq, Gemini, NVIDIA, OpenAI, OpenRouter, Mistral, DeepSeek, Ollama, …), with backup models.
- Screenshot / select-area / paste an image and ask about it.
- Listens to the meeting (speakers + optional mic): record-and-ask, or auto-detect questions.
- Hindi, Bengali and English speech, with optional "Convert to English".
- Files and an "About me" profile as context.
- Floating bubble, hotkeys, themes, opacity, sessions.
- Keys are stored only in Windows Credential Manager.

## Build the installer
Needs Rust and Node.js.

1. Double-click `Build Peek AI Installer.bat`.
2. Run `src-tauri\target\release\bundle\nsis\Peek AI_1.0.0_x64-setup.exe`.

## Optional: Local Whisper (offline speech-to-text)
Run `tools\Install Local Whisper.bat`.

Built with Tauri 2 (Rust) and TypeScript.
