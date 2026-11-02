import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

// Click = open/hide panel. Press and move = drag the bubble.
const orb = document.getElementById("orb")!;
let down = false;
let dragging = false;
let sx = 0;
let sy = 0;

orb.addEventListener("mousedown", (e) => {
  if (e.button !== 0) return;
  down = true;
  dragging = false;
  sx = e.screenX;
  sy = e.screenY;
});

window.addEventListener("mousemove", (e) => {
  if (!down || dragging) return;
  if (Math.abs(e.screenX - sx) + Math.abs(e.screenY - sy) > 4) {
    dragging = true;
    down = false;
    void getCurrentWindow().startDragging();
  }
});

window.addEventListener("mouseup", () => {
  if (down && !dragging) void invoke("toggle_panel");
  down = false;
});

// Small "–" button (shows on hover): hides the bubble. Get it back from the tray icon.
document.getElementById("min")!.addEventListener("click", () => void invoke("hide_bubble"));

// A new answer arrived while the window was closed: show a glowing ring + red dot.
void listen("answer-ready", () => orb.classList.add("ping"));
void listen("panel-seen", () => orb.classList.remove("ping"));
orb.addEventListener("mousedown", () => orb.classList.remove("ping"));

// Red ring while the assistant is set to be VISIBLE in screen share.
void listen<boolean>("protection-changed", (e) => orb.classList.toggle("exposed", !e.payload));
