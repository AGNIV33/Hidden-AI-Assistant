import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// Full-screen see-through overlay (hidden from screen share). Drag a box; the backend crops a screenshot to it.
const sel = document.getElementById("sel")!;
let dragging = false;
let sx = 0;
let sy = 0;

function box(e: MouseEvent) {
  const x = Math.min(sx, e.clientX), y = Math.min(sy, e.clientY);
  return { x, y, w: Math.abs(e.clientX - sx), h: Math.abs(e.clientY - sy) };
}
function reset() {
  dragging = false;
  sel.style.display = "none";
  document.body.classList.remove("selecting");
}
function cancel() {
  reset();
  void invoke("cancel_snip");
}

document.addEventListener("mousedown", (e) => {
  if (e.button !== 0) return;
  dragging = true;
  sx = e.clientX;
  sy = e.clientY;
  document.body.classList.add("selecting");
  Object.assign(sel.style, { display: "block", left: `${sx}px`, top: `${sy}px`, width: "0px", height: "0px" });
});
document.addEventListener("mousemove", (e) => {
  if (!dragging) return;
  const b = box(e);
  Object.assign(sel.style, { left: `${b.x}px`, top: `${b.y}px`, width: `${b.w}px`, height: `${b.h}px` });
});
document.addEventListener("mouseup", (e) => {
  if (!dragging) return;
  const b = box(e);
  reset();
  if (b.w < 6 || b.h < 6) { cancel(); return; }
  const d = window.devicePixelRatio || 1; // page pixels -> real screen pixels
  void invoke("finish_snip", { x: b.x * d, y: b.y * d, w: b.w * d, h: b.h * d });
});
document.addEventListener("keydown", (e) => { if (e.key === "Escape") cancel(); });
document.addEventListener("contextmenu", (e) => { e.preventDefault(); cancel(); });
void listen("snip-start", reset);
