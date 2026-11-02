# DESIGN — Visual & UI System

## 1. Style
Minimal, glassy, calm. Dark by default. Small and out of the way.

## 2. Tokens (CSS variables)
```css
--bg:        rgba(18, 18, 24, 0.82);
--surface:   rgba(255, 255, 255, 0.06);
--border:    rgba(255, 255, 255, 0.10);
--text:      #EDEDF2;
--text-dim:  #9A9AA8;
--accent:    #7C6CFF;   /* user can change */
--danger:    #FF5C6C;
--success:   #3DDC97;
--radius:    14px;
--radius-sm: 8px;
--blur:      18px;
--font:      "Segoe UI Variable", "Segoe UI", system-ui, sans-serif;
--font-size: 14px;      /* user can change 12–18 */
--opacity:   1;         /* window opacity 0.2–1 */
```

## 3. Themes
| Theme | bg | text | accent |
|---|---|---|---|
| Midnight (default) | rgba(18,18,24,.82) | #EDEDF2 | #7C6CFF |
| Light Glass | rgba(250,250,252,.85) | #1B1B22 | #5B4CFF |
| Ocean | rgba(10,25,40,.85) | #E6F2FF | #2BB3FF |
| Forest | rgba(14,26,20,.85) | #E8F5EC | #3DDC97 |
| Rose | rgba(30,16,22,.85) | #FCEAF0 | #FF6FA1 |
Plus custom accent colour picker.

## 4. Layout
### Bubble (56×56)
- Round, glass background, accent ring.
- Ring **pulses** when listening; shows a dot when a new answer arrives.
- Drag to move; click → open/close panel.

### Panel (420×600, resizable)
```
┌──────────────────────────────────┐
│ ● Session name ▾     🎙  📷  ⚙  ✕ │  header (drag area)
├──────────────────────────────────┤
│ [Point][Brief][Detail][Long]     │  answer mode chips
├──────────────────────────────────┤
│  Live: "Them: what's your ..."   │  transcript strip (when listening)
│                                  │
│  Q: ...                          │  chat
│  A: ...            gemini-2.5 ▪  │  model label
│                                  │
├──────────────────────────────────┤
│ 📎  Ask anything...          ➤   │  input
└──────────────────────────────────┘
```
### Settings (inside panel, tabs)
General · AI Keys & Models · Hotkeys · Appearance · Listening

## 5. Components
- **Chip** – pill buttons, active = accent fill.
- **Icon button** – 32×32, hover = surface.
- **Message** – question small + dim; answer normal; copy button on hover.
- **Toast** – bottom, 3 s, for errors.
- **Slider** – opacity, font size.
- **Hotkey field** – click → press keys → saved.

## 6. Motion
- 150–200 ms ease-out for open/close, hover.
- Answer text streams in (typing feel).
- No heavy animations (memory).

## 7. Default hotkeys
| Action | Key |
|---|---|
| Show / hide panel | Ctrl+Shift+Space |
| Screenshot & ask | Ctrl+Shift+S |
| Toggle listening | Ctrl+Shift+L |
| Cycle answer mode | Ctrl+Shift+M |
| Quit | Ctrl+Shift+Q |

## 8. Answer tone
- Natural, spoken, first person, no "As an AI".
- No heavy markdown in Point/Brief modes.
- Modes: **Point** 1–2 lines · **Brief** 3–5 lines · **Descriptive** 1–2 short paragraphs · **Long** full explanation with steps/examples.
