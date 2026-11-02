# PRD — Peek AI

## 1. What it is
A small Windows desktop app. An AI assistant that floats on screen during meetings and is **invisible in screen share** (Teams, Zoom, Google Meet, WhatsApp, etc.). It can listen to the call, read the screen, and give human-like answers.

## 2. Goals
- Fast start, low memory (target: < 120 MB RAM idle, < 15 MB installer).
- 100% free to build and run (free tools + free AI tiers).
- Simple, elegant UI.

## 3. Non-goals (for v1)
- Mac / Linux.
- Paid AI models.
- Cloud sync / accounts.

## 4. Users
- Owner (Agniv) in business / client meetings.

## 5. Features (v1)
| ID | Feature | Detail |
|---|---|---|
| F1 | Sessions | Create, rename, delete, switch sessions. Each has its own chat + files. |
| F2 | Context files | Add PDF, DOCX, TXT, MD to a session. Text is used as context for answers. |
| F3 | Minimal elegant UI | Glass-style panel, clean fonts, smooth animations. |
| F4 | Custom hotkeys | Show/hide, screenshot-ask, toggle listening, quit, change answer mode. Editable in Settings. |
| F5 | Floating icon | Small draggable bubble; click opens panel. |
| F6 | Hidden in screen share | All app windows excluded from capture (`WDA_EXCLUDEFROMCAPTURE`). Not in taskbar. |
| F7 | Low memory | Tauri + Rust backend, no bundled browser. |
| F8 | Any language | Multilingual speech-to-text (Whisper). Answer in user-chosen language. |
| F9 | Opacity slider | 20%–100% window opacity. |
| F10 | Model fallback | Each slot has a main connection + backups; if one fails / hits limit → next one automatically. |
| F11 | Image understanding | Screenshot or pasted image → find question → answer. |
| F12 | Smart question detection | Listen to other speakers (system audio) + user mic separately; AI detects real questions from transcript. |
| F13 | Answer modes | To-the-point / Brief / Descriptive / Long detailed. Human-like, natural speech tone. |
| F14 | Customization | Themes, accent colour, font size, panel size, position. |
| F15 | Bring your own AI | In Settings, anyone can paste their own API key and pick any provider + model, separately for Answers, Vision and Speech-to-text. |
| F16 | Installer + tray | One `.exe` installer (no admin needed) with Start Menu shortcut and app icon. Runs in the system tray: right-click → Open assistant / Show-hide bubble / **Quit**. Opening the app twice just opens the running one. |
| F17 | Bubble | Perfect circle. Click it: the bubble disappears and the assistant window opens in its place. Close the window: the bubble comes back. Hover shows a small "–" button that hides the bubble (bring back from tray or Ctrl+Shift+B). |
| F18 | Screen-share hiding switch | An eye button in the window header (and Ctrl+Shift+H, and the tray menu) turns "hidden from screen share" off or on. Always starts ON. Turning it OFF needs a second click, and while OFF a red warning bar shows in the window and a red ring on the bubble. |

## 6. Success criteria
- Window not visible in Teams, Zoom, Meet (browser), WhatsApp desktop share — tested.
- Answer shown within ~3 s after a question ends.
- Idle RAM below target.
- Works with only free API keys.

## 7. Risks
- Free API limits may change → fallback + configurable model names.
- Free tiers may use data for training → warn user in Settings.
- Phone camera can still see the screen (out of scope).
| F19 | Streaming answers | Answers appear word by word while being written (all providers, with backups). |
| F20 | Local Whisper auto | If Speech-to-text uses Local Whisper, the app starts it hidden (graphics card first, CPU fallback) and stops it on quit. |
| F21 | About me | A personal profile (typed or filled from a resume file) sent with every question in every session. |
| F22 | Safe hotkeys | Defaults use Ctrl+Alt so they never block Teams/Zoom shortcuts (e.g. Teams Ctrl+Shift+M mute). |
| F23 | Area screenshot + thumbnails | Select-area capture (Ctrl+Alt+X), screenshots shown as previews in chat, click to enlarge. |
| F24 | Mic recorder | Click mic to record, click again to stop; what was heard goes into the question box. Optional auto-answer mode. |

## 8. Installer
- One file: `Peek AI_1.0.0_x64-setup.exe` (NSIS). Double-click to install, no admin rights needed (per-user install).
- Adds Peek AI to the Start Menu with the app icon; uninstall from Windows Settings → Apps.
- Includes the WebView2 setup inside, so it also installs on PCs that do not have it.
- Build it with `Build Peek AI Installer.bat` (runs `npm install` + `npm run tauri build`).
- Not code-signed (signing costs money), so Windows SmartScreen may warn the first time: More info → Run anyway.
