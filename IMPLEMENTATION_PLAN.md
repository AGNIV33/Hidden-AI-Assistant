# IMPLEMENTATION PLAN — Build Order

Tick `[x]` when done.

## Phase 0 — Setup
- [x] Install: Node.js LTS, Rust (rustup), VS C++ Build Tools, WebView2 (already in Win 10/11)
- [x] Tauri v2 project scaffold (Vite + vanilla TS)
- [x] Docs in repo

## Phase 1 — Core overlay (F3, F4, F5, F6, F7, F9, F10)
- [x] Two windows: `bubble` + `panel`, transparent, no frame, always on top (code done, needs Windows test)
- [x] `contentProtected: true` + `skipTaskbar: true` on both (code done, needs Windows test)
- [x] Bubble drag + click toggles panel (code done, needs Windows test)
- [x] Panel UI: header, mode chips, chat, input (code done, needs Windows test)
- [x] Opacity slider (code done, needs Windows test)
- [x] API keys saved in Credential Manager (code done, needs Windows test)
- [x] Provider chain with fallback (Groq → Gemini → OpenRouter) (code done, needs Windows test)
- [x] Global hotkeys (default set: Space, M, B, Q) (code done, needs Windows test)
- [x] Tray icon with Quit, single instance, hide-on-close (code done, needs Windows test)
- [x] Round bubble + hide button (code done, needs Windows test)
- [x] Windows installer (.exe) builds and installs (code done, needs Windows test)
- [x] Answer modes + human-like prompts (F13) (code done, needs Windows test)
- [ ] **Test:** hidden in Teams / Zoom / Meet / WhatsApp

## Phase 2 — Sessions, files, images (F1, F2, F11)
- [x] SQLite DB + sessions CRUD (code done, needs Windows test)
- [x] Chat history saved per session (code done, needs Windows test)
- [x] Add files (PDF / DOCX / TXT / MD) → text extracted → used as context (code done, needs Windows test)
- [x] Screenshot hotkey → vision model → answer (code done, needs Windows test)
- [x] Paste / drop image into input (code done, needs Windows test)

## Phase 3 — Listening mode (F8, F12)
- [x] (code done, needs Windows test) Mic + loopback capture (cpal)
- [x] (code done, needs Windows test) VAD chunking, 16 kHz WAV
- [x] (code done, needs Windows test) Groq Whisper STT (auto language)
- [x] (code done, needs Windows test) Live transcript strip
- [x] (code done, needs Windows test) Question detection + auto answer
- [x] (code done, needs Windows test) Answer language setting (same as speaker / English / custom)

## Phase 4 — Customization & polish (F14)
- [x] Themes + accent picker + font size (code done, needs Windows test)
- [x] Editable hotkeys (code done, needs Windows test)
- [x] Remember window position / size (code done, needs Windows test)
- [x] Autostart option (code done, needs Windows test)
- [x] Memory check (< 120 MB idle) (code done, needs Windows test)
- [x] Release build + installer (`npm run tauri build`) (code done, needs Windows test)
