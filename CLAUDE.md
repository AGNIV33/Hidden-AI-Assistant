# CLAUDE.md / AGENTS.md — Instructions for AI coding agents

Read this before changing code.

## Project
Peek AI — Windows desktop app (Tauri v2 + Rust + Vite/TypeScript). See PRD.md, ARCHITECTURE.md, DESIGN.md.

## Rules
1. **Windows first.** Do not add Mac/Linux code paths unless asked.
2. **Every window must be capture-protected.** Any new window: `contentProtected: true`, `skipTaskbar: true`. Never remove this.
3. **Keep it light.** No React/Vue/heavy UI libs. Vanilla TypeScript + CSS. Ask before adding any npm or crate dependency > 1 MB.
4. **Bring your own AI.** Provider URL, model and key are chosen by the user in Settings (Answers / Vision / Speech-to-text). Never hardcode a provider in logic; defaults live only in the Settings presets. Keep it free-friendly.
5. **Secrets.** API keys go in Windows Credential Manager (`keyring` crate). Never in files, logs or git.
6. **AI calls run in Rust**, not in the webview (keys never reach JS).
7. **One provider interface.** Every connection uses the OpenAI-compatible API (`providers.rs`). Keys only over https (or localhost).
8. **Errors never crash the app.** Show a small toast; log to `TESTING.md` bug log if found in testing.
9. Follow IMPLEMENTATION_PLAN.md order. Tick items when done.
10. Simple code, short functions, comments only where needed.

## Commands
- Dev: `npm run tauri dev`
- Build: `npm run tauri build`
- Rust check: `cargo check` (in `src-tauri`)

## Folder map
```
src/            frontend (html, ts, css)
src-tauri/src/  Rust backend
docs (root)     PRD, ARCHITECTURE, DESIGN, IMPLEMENTATION_PLAN, TESTING
```
