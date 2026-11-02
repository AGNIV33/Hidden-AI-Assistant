# TESTING — Safety Net & Bug Log

## 1. Screen-share invisibility (most important)
Share **entire screen** in each app. App should NOT appear for the viewer.
Test with a second device or a recording.

| App | Bubble hidden | Panel hidden | Date | Result |
|---|---|---|---|---|
| Microsoft Teams (desktop) | | | | |
| Zoom (desktop) | | | | |
| Google Meet (Chrome) | | | | |
| Google Meet (Edge) | | | | |
| WhatsApp desktop call | | | | |
| OBS / Win+Alt+R recording | | | | |
| Snipping Tool screenshot | | | | |

Also check: not in taskbar, not in Alt+Tab.

## 2. Performance
| Check | Target | Result |
|---|---|---|
| Idle RAM (Task Manager, all processes incl. WebView2) | < 120 MB | |
| RAM while listening | < 180 MB | |
| Cold start time | < 2 s | |
| Installer size | < 15 MB | |

## 2b. Installer + tray
- [ ] `npm run tauri build` makes `Peek AI_0.1.0_x64-setup.exe`
- [ ] Installs without admin; Start Menu shortcut has the app icon
- [ ] Launching from Start Menu shows the bubble
- [ ] Tray icon visible; right-click → Quit fully closes the app (check Task Manager)
- [ ] Launching a 2nd time does not start a 2nd copy
- [ ] Click bubble → bubble vanishes, window opens in its place; close window → bubble returns
- [ ] Bubble is a circle; "–" button hides it; tray / Ctrl+Alt+B brings it back
- [ ] Uninstall from Windows Settings → Apps works

## 2c. Hide / show switch
- [ ] Eye button is green (hidden). One click only shows a warning; the second click makes it visible
- [ ] While visible: red bar in the window, red ring on the bubble; a screen share DOES show them
- [ ] Click again / Ctrl+Alt+H / tray menu: hidden again (verify in the share)
- [ ] After quitting and restarting, it is hidden again by itself

## 2d. Sending messages
- [ ] Type + Enter sends; Shift+Enter makes a new line
- [ ] While waiting, "Thinking… Ns" counts up and the send button shows ■
- [ ] ■ stops the wait and puts your question back in the box
- [ ] Enter while waiting shows a message (does not freeze or do nothing silently)
- [ ] A dead network / wrong model gives an error within ~1 minute, not a hang

## 3. Features
- [ ] Typed question answered in every mode (Point / Brief / Descriptive / Long)
- [ ] Answers sound human, no "As an AI"
- [ ] Fallback: wrong Groq key → Gemini answers; label shows model
- [ ] All providers fail → clear error toast, app does not crash
- [ ] Opacity slider 20–100% works
- [ ] Hotkeys work while another app is focused
- [ ] Sessions: create / rename / delete / switch
- [ ] Files: PDF, DOCX, TXT, MD text used in answers
- [ ] Screenshot of a question → correct answer
- [ ] Listening: English, Hindi, other language → transcript correct
- [ ] Question detected only for real questions (not small talk)
- [ ] Themes + accent applied and saved after restart

## 3b. Listening mode
- [ ] Mic button → turns red; "Listening…" strip appears
- [ ] Play a YouTube video with speech: "Them:" lines appear in the strip
- [ ] Ask a question aloud in a call / video: question + answer appear in chat within ~3–5 s
- [ ] Small talk ("hi, how are you") does not trigger an answer
- [ ] Same question is not answered twice
- [ ] Non-English speech (Hindi etc.) is transcribed; answer follows the "Answer in" setting
- [ ] "My mic" on: own voice shows as "Me:"
- [ ] Window closed while listening: bubble shows ring + red dot on a new answer
- [ ] Ctrl+Alt+L starts / stops listening
- [ ] Wrong STT key → one clear error toast (not a flood)
- [ ] Stop listening → no more transcript lines

## 3c. Sessions, files, screenshots
- [ ] + creates a new session; switching shows that session's own chat and files
- [ ] Rename and delete (🗑 needs a second click) work; chats are still there after quitting and reopening
- [ ] 📎 add a PDF, a .docx and a .txt; chips appear; asking about their content gets a correct answer
- [ ] A big PDF (50+ pages): answers still use the right part of it
- [ ] A scanned (image-only) PDF shows a clear "no readable text" message
- [ ] Drag a file onto the window adds it; drag an image asks about it
- [ ] Paste an image (Win+Shift+S then Ctrl+V in the box) asks about it
- [ ] Screenshot hotkey / 📷 on a page with a question: answer is correct; the assistant itself is NOT in the screenshot
- [ ] Window closed + screenshot hotkey: bubble glows when the answer is ready

## 3d. Appearance, hotkeys, startup
- [ ] Each theme looks readable (text vs background), accent colour and text size change live
- [ ] Change a hotkey; the new one works, the old one does not; clash with another app shows a clear message
- [ ] Reset hotkeys restores the defaults
- [ ] Move the bubble, resize the window, quit and reopen: both are where you left them
- [ ] "Start with Windows" on: after a restart the app starts by itself (turn it off again after testing)

## 3e. Latest features
- [ ] Answer text appears word by word, then shows the model label + Copy
- [ ] Settings → About me: type or "Fill from file" (resume); answers use it
- [ ] Speech-to-text = Local Whisper: pressing the mic starts it hidden (no black window); quitting the app stops it (check Task Manager: no whisper-server.exe)
- [ ] New hotkeys work: Ctrl+Alt+Space / S / X / L / M / B / H / Q; Teams Ctrl+Shift+M mute still works
- [ ] Drag ⠿ to reorder models; the top one answers first
- [ ] Ctrl+Alt+X area select: overlay not visible to viewers; selected part answered; thumbnail in chat

## 4. Security
- [ ] API keys not in DB, files or logs
- [ ] Keys never sent to webview JS

## 5. Bug log
| # | Date | Bug | Steps | Status | Fix |
|---|---|---|---|---|---|
| | | | | | |
