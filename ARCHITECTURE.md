# ARCHITECTURE — Peek AI

## 1. Stack
| Layer | Choice | Why |
|---|---|---|
| App shell | Tauri v2 | Uses Windows WebView2 (built-in) → small + low RAM |
| Backend | Rust | Fast, low memory, safe |
| Frontend | Vite + vanilla TypeScript + CSS | No framework weight |
| Storage | JSON files in `%APPDATA%\\com.agniv.hiddenai` (`sessions.json`, `window.json`); settings in the page's localStorage | Light, no database engine to load |
| Secrets | `keyring` (Windows Credential Manager) | Safe API key storage |
| Audio | `cpal` (WASAPI mic + loopback) + `hound` (WAV) | Capture both sides of the call |
| Screenshot | `xcap` (screen under the mouse; our windows are excluded from capture), `image` → JPEG ≤1600 px | Sent to the Vision slot |
| Autostart | `tauri-plugin-autostart` | "Start with Windows" |
| Files | `pdf-extract` (PDF), `zip` + XML scan (DOCX), `image` (pictures) | Text extraction; a broken PDF cannot crash the app |
| HTTP | `reqwest` (rustls) | API calls |
| Hotkeys | `tauri-plugin-global-shortcut` | Global hotkeys |

## 2. Windows
| Window | Size | Purpose |
|---|---|---|
| `bubble` | 56×56 | Floating icon, draggable, always on top |
| `panel` | ~420×600 | Main UI: chat, sessions, settings |

Both: `transparent`, `decorations: false`, `alwaysOnTop`, `skipTaskbar`, `contentProtected: true`.
`contentProtected` → Windows `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` → hidden in all capture apps using Windows capture APIs.

### Tray, installer, single instance
- **Bubble ↔ window swap:** click bubble → bubble hides, panel opens at the bubble's spot. Close panel (✕, Alt+F4, hotkey, tray) → panel hides, bubble returns at the panel's spot (unless the user hid the bubble with "–").
- **Screen-share hiding switch:** `set_content_protected(on)` on both windows. State is not saved: the app always starts hidden (ON). Eye button, Ctrl+Shift+H and a tray item toggle it; `protection-changed` event updates the red warning bar and bubble ring.
- **Tray icon** (Tauri `tray-icon`): left-click = open/hide panel; right-click menu = Open assistant · Show/hide bubble · **Quit**.
- Closing a window only hides it (`CloseRequested` is prevented). The app ends only via tray **Quit** or Ctrl+Shift+Q.
- `tauri-plugin-single-instance`: launching the app again shows the running one.
- **Installer:** NSIS (`npm run tauri build`) → `src-tauri\target\release\bundle\nsis\*-setup.exe`. Per-user install (`installMode: currentUser`), no admin. Creates Start Menu shortcut with the app icon.
- Icon source: `app-icon.png` (1024×1024) → `npm run tauri icon app-icon.png` regenerates `src-tauri/icons/*`.

## 3. Rust modules (`src-tauri/src/`)
```
main.rs / lib.rs   app setup, windows, plugins
commands.rs        Tauri commands called from UI
providers.rs       LLM providers + fallback chain
prompts.rs         system prompts (answer modes, question detection)
db.rs              SQLite schema + queries
secrets.rs         API key save/load (keyring)
files.rs           PDF/DOCX/TXT text extraction
capture.rs         screenshot
audio.rs           mic + loopback capture, VAD, chunking
stt.rs             speech-to-text (Groq Whisper)
listener.rs        transcript buffer + question detection loop
hotkeys.rs         register / update global shortcuts
```

## 4. AI connections + fallback (F10, F15)
The user adds their **own** AI connections in Settings. Nothing is hardcoded.

| Slot | Used for | Needs |
|---|---|---|
| Answers | Writing replies | Any chat model |
| Vision | Reading screenshots (F11) | A model that accepts images |
| Speech-to-text | Listening mode (F8, F12) | An audio transcription model |

Each slot holds an **ordered list** of connections: 1st = main, the rest = automatic backups.
A connection = `{ id, preset, baseUrl, model }` (saved in the app) + API key (saved in Windows Credential Manager under `id`).

- Any **OpenAI-compatible** service works (`POST {baseUrl}/chat/completions`, `GET {baseUrl}/models`).
- Presets: Groq, Gemini, OpenAI, Anthropic (compat endpoint), NVIDIA, OpenRouter, Mistral, DeepSeek, Together, Ollama (local), Custom.
- "Load" button calls `GET /models` to test the key and fill a model picker.
- Keys are sent only over `https://` (plain `http://` allowed only for `localhost` / `127.0.0.1`).
- Fallback rule: try connection 1, on error / timeout (15 s) / 429 / 5xx try the next. Skip connections with no key or no model. Show which one answered.

## 5. Data flow
**Typed question:** UI → `ask` command → build prompt (mode + session files + last 10 messages) → provider chain → save to DB → UI.

**Screenshot (F11):** hotkey → `xcap` capture → PNG base64 → vision model with "find the question in this image and answer it" → UI.

**Listening (F12):**
```
mic stream ─┐                       ┌─> label "Me"
            ├─ VAD (energy) ─ chunk ─ Groq Whisper (multilingual) ─ transcript buffer
loopback ───┘                       └─> label "Them"
transcript buffer ─(on silence after "Them" speech)─> LLM detect: is there a question? ─ yes ─> answer ─> UI
```
- Chunks: cut on ~0.8 s silence, max 15 s.
- Audio resampled to 16 kHz mono WAV.
- Detection prompt returns JSON `{ "question": "..." | null }`.

### Listening mode as built (Phase 3)
- `audio.rs`: one thread per source. **Them** = default output device opened as input (WASAPI loopback). **Me** = default mic (optional, off by default).
- Audio is mixed to mono, resampled to 16 kHz, cut into chunks by a simple loudness detector (chunk ends after 0.8 s of quiet, max 15 s), saved as a WAV in memory.
- Each chunk goes to the Speech-to-text slots in order (`POST {baseUrl}/audio/transcriptions`) → `transcript` event `{speaker, text}`.
- The panel keeps the last ~12 lines and, 0.9 s after "Them" speaks, calls `detect_answer`. One AI call returns either `NONE` or `QUESTION: … ANSWER: …` using the current answer mode. Same question is not answered twice.
- If the window is closed, the bubble gets a glowing ring + red dot when a new answer arrives.
- Hotkey Ctrl+Shift+L toggles listening.

### Sessions, files, screenshots, hotkeys (Phase 2 + 4)
- **Sessions:** `{ id, name, messages[], files[] }`, saved to `sessions.json` (atomic write, max 300 messages each). Switch / rename / delete in the header menu.
- **Files:** 📎 button, drag-and-drop, or paste. Text is stored with the session. Per question, `context.ts` sends all text if it fits the limit (Settings → File text limit, default 12,000 characters); otherwise only the pieces that best match the question (keyword score). Files are sent as information, never as instructions.
- **Screenshot:** hotkey or 📷 → `xcap` → JPEG → `ask_image` → Vision slots (falls back to Answers slots). Pasted/dropped images use the same path.
- **Hotkeys:** 6 actions, changeable in Settings. `hotkeys.rs` re-registers them; while recording a new combo all hotkeys are paused. Must include Ctrl or Alt.
- **Appearance:** 5 themes + accent colour + text size + opacity, applied as CSS variables.
- **Window memory:** bubble position and panel size saved to `window.json` (only restored if still on a screen).

## 6. Data storage (replaces the old SQLite plan)
```sql
sessions(id INTEGER PK, name TEXT, created_at INTEGER)
messages(id INTEGER PK, session_id INTEGER, role TEXT, content TEXT, model TEXT, created_at INTEGER)
files(id INTEGER PK, session_id INTEGER, name TEXT, text TEXT, added_at INTEGER)
transcript(id INTEGER PK, session_id INTEGER, speaker TEXT, text TEXT, at INTEGER)
settings(key TEXT PK, value TEXT)   -- JSON values
```
DB path: `%APPDATA%\HiddenAIAssistant\data.db`.

## 7. Context limits
- File text total cap per prompt: ~20k chars (trim longest first).
- Transcript window for detection: last ~90 s.

## 8. Memory budget
- WebView2 + Rust idle: target < 120 MB.
- Audio buffers freed after each chunk is sent.
- No local AI models.
