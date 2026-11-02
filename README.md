# Peek AI

A light, free AI assistant for Windows. It floats on your screen during meetings, and **its windows are hidden when you share your screen**.

- Ask questions by typing, from a screenshot, or by listening to the call
- Works with Hindi, Bengali and English or any other languages and can convert speech to English
- Use any AI you like (Groq, Google Gemini, NVIDIA, OpenAI, OpenRouter, …), with free options
- Your API keys stay in Windows Credential Manager, never in files

---

## 1. Install

### Option A: Download the installer (easiest)
1. Go to **[Releases](https://github.com/AGNIV33/Hidden-AI-Assistant/releases/latest)** and download **`Peek.AI_1.0.0_x64-setup.exe`**.
2. Double-click the downloaded file.
3. If Windows shows a blue box ("Windows protected your PC"), click **More info → Run anyway**. It shows because the app is not signed, which is normal for free apps.
4. Follow the steps, like any other software:
   - **Install for:** "Only for me" (no admin needed) or "Anyone who uses this computer" (asks for admin).
   - **Install location:** keep the default or click **Browse** to pick a folder (for example on D:).
   - **Start Menu folder:** keep "Peek AI".
   - Click **Install**, then **Finish**.
5. Open **Peek AI** from the Start Menu.

### Option B: Build the installer yourself (from this code)
You need these once:
1. **Rust**: https://rustup.rs. When it asks, install the **Visual Studio C++ Build Tools**.
2. **Node.js** (LTS): https://nodejs.org

Then:
1. Download this repo: green **Code** button → **Download ZIP**. Unzip it.
2. Double-click **`Build Peek AI Installer.bat`**. The first time takes 5–15 minutes.
3. A folder opens with **`Peek AI_1.0.0_x64-setup.exe`**. Install it as in Option A.

You can share that `.exe` with anyone. It includes everything the app needs.

---

## 2. First setup (2 minutes)

Open Peek AI → click **⚙ Settings**. There are 3 kinds of AI connections:

| Box | What it does | Good free choice |
|---|---|---|
| **Answers** | Writes the replies | Groq (`llama-3.3-70b-versatile`) or Gemini |
| **Vision (screenshots)** | Reads screenshots and images | Google Gemini (`gemini-3.1-flash-lite`) |
| **Speech-to-text** | Turns the call audio into text | Groq (`whisper-large-v3`) or Local Whisper |

For each box:
1. Paste your **API key**. The app works out the provider and model by itself.
2. Click **+ Add backup** to add a second one. If the first one fails, the next one is used.
3. Drag a card by its **⠿** handle to change the order. The top one is the main one.

**Where to get free keys:**
- Groq: https://console.groq.com/keys
- Google Gemini: https://aistudio.google.com/apikey
- NVIDIA: https://build.nvidia.com

> Never share your API keys or paste them in chats.

---

## 3. How to use

### The bubble and the window
- The round **bubble** floats on your screen. Click it to open the window.
- Close the window (✕) and the bubble comes back.
- The **tray icon** (near the clock) has Open, Hide/show in screen share, and **Quit**.

### Ask by typing
- Type in the box and press **Enter**. **Shift+Enter** adds a new line.
- Answer length chips: **Point / Brief / Detail / Long**.

### Ask about the screen
- **📷** takes a screenshot of the whole screen.
- The **area button** (the square of corners, next to 📷) lets you drag over just one part of the screen.
- You can also **paste** (Ctrl+V) or **drop** an image.
- The image waits above the box. Type your question and press **Enter**.

### Listen to the call (live transcript)
Click the **🎤 mic**. What people say appears **live in the chat** as "Them" bubbles. Click the mic again to stop.
- Each bubble has an **Answer this** button. Click it to get a reply you can say.
- A **sound meter** at the top shows the app is hearing the call.
- **Auto mode** (Settings → Listening → Mode) also answers questions by itself.

Listening options (Settings → Listening):
- **People speak**: Auto (Hindi, Bengali, English), or one language. Picking one gives the best results.
- **Convert to English**: Bengali or Hindi speech becomes English text.
- **My mic**: also hear your own voice. Use headphones with it.

### Files and "About me"
- **📎** adds PDF, Word or text files. The AI uses them when answering.
- Settings → **About me**: write a few lines about yourself (job, skills). Answers then sound like you.

### Hide or show in screen share
- The **eye button** at the top switches it. By default Peek AI is **hidden** in screen share.
- If you see the yellow bar "⚠ VISIBLE in screen share", others can see it.

---

## 4. Hotkeys

| Keys | Action |
|---|---|
| **Ctrl+Alt+Space** | Show / hide the window |
| **Ctrl+Alt+S** | Screenshot |
| **Ctrl+Alt+X** | Select an area |
| **Ctrl+Alt+L** | Start / stop listening |
| **Ctrl+Alt+M** | Change answer length |
| **Ctrl+Alt+B** | Show / hide the bubble |
| **Ctrl+Alt+H** | Hide / show in screen share |
| **Ctrl+Alt+Q** | Quit |

You can change them in Settings → Hotkeys.

---

## 5. Optional: Local Whisper (free, offline speech-to-text)

Runs speech-to-text on your own PC. It works best with an NVIDIA graphics card.

1. Double-click **`tools\Install Local Whisper.bat`**. It downloads about 1 GB to `D:\whisper`.
2. In Settings → Speech-to-text, choose **Local Whisper (this PC)**.
3. Peek AI starts and stops it by itself.

---

## 6. Problems?

| Problem | Fix |
|---|---|
| "HTTP 404" | Wrong model name for that provider. Paste the key again so the app picks a model, or choose one from the list. |
| "HTTP 429" / "503" | The free limit is reached or the service is busy. Wait a minute, or add a backup. |
| Vision is slow | Put Gemini first in Vision. NVIDIA free models can be slow. |
| Nothing heard | Sound must play on your **default** speakers or headphones. |
| Netflix / Prime video is black in screenshots | These are copy-protected. No app can capture them. |
| Blue "Windows protected your PC" box | Click **More info → Run anyway**. |

**Note:** screen-share hiding uses a Windows feature that Zoom, Google Meet, Teams and others respect. Always test once with a friend before an important meeting.

---

Built with Tauri 2 (Rust) and TypeScript. Docs: `PRD.md`, `ARCHITECTURE.md`, `DESIGN.md`, `TESTING.md`.
