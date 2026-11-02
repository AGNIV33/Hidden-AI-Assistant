import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { $, el, store, toast, uid } from "./util";
import { MODES, config, isLocalUrl, newSlot, saveConfig } from "./config";
import { buildContext } from "./context";
import * as S from "./sessions";
import { applyAppearance, applyOpacity, buildSettings, refreshKeys, restoreHotkeys } from "./settings";

interface AskResp { text: string; provider: string; model: string }
interface Loaded { name: string; kind: "text" | "image"; text: string; image: string }

const panel = $("panel"), chat = $("chat"), input = $<HTMLTextAreaElement>("input");
const sendBtn = $<HTMLButtonElement>("sendBtn");
const settingsEl = $("settings"), sessionsEl = $("sessions"), filesEl = $("files");

let mode: string = store.get("mode", "brief");
let busy = false;

const scrollDown = () => { chat.scrollTop = chat.scrollHeight; };
function autoGrow() { input.style.height = "auto"; input.style.height = Math.min(input.scrollHeight, 120) + "px"; }
/** "About me" text (Settings) + the session's files, within the File text limit. */
function ctx(query: string): string {
  const budget = store.get("ctxBudget", 12000);
  const prof = store.get<{ on?: boolean; text?: string }>("profile", {});
  const me = prof.on !== false && prof.text?.trim() ? `### About me (the user)\n${prof.text.trim().slice(0, 6000)}` : "";
  const files = buildContext(S.cur().files, query, Math.max(1000, budget - me.length));
  return [me, recentHeard(), files].filter(Boolean).join("\n\n");
}
const modelLabel = (r: AskResp) => `${r.provider} · ${r.model}`;

/* ---------- answer mode chips ---------- */
function renderChips() {
  const box = $("chips");
  box.replaceChildren();
  for (const m of MODES) {
    const b = el("button", "chip" + (m.id === mode ? " on" : ""), m.label);
    b.onclick = () => setMode(m.id);
    box.appendChild(b);
  }
}
function setMode(id: string, announce = false) {
  mode = id;
  store.set("mode", id);
  renderChips();
  if (announce) toast("Mode: " + MODES.find((m) => m.id === id)!.label, "info");
}

/* ---------- chat ---------- */
function emptyHint() {
  const e = el("div", "empty");
  e.append("Ask anything.", el("br"), el("small", "", "Enter = send · Shift+Enter = new line · 📎 add files · paste or drop an image"));
  return e;
}
/** Full-size images captured in this run (older chats only keep the small preview). */
const fullImages = new WeakMap<S.ChatMsg, string>();

function questionEl(m: S.ChatMsg) {
  if (!m.thumb) return el("div", m.auto ? "q auto" : "q", (m.shot ? "📷 " : "") + m.content);
  const box = el("div", "q shot");
  const img = el("img", "thumb");
  img.src = m.thumb;
  img.alt = "Screenshot";
  img.title = "Click to view larger";
  img.onclick = () => showImage(fullImages.get(m) ?? m.thumb!);
  box.append(img, el("div", "cap", m.content === "Screenshot" ? "📷 Screenshot" : m.content));
  return box;
}
function showImage(src: string) {
  const v = el("div", "viewer");
  const img = el("img");
  img.src = src;
  v.title = "Click or press Esc to close";
  v.appendChild(img);
  const close = () => { v.remove(); window.removeEventListener("keydown", onKey, true); };
  const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") { e.stopPropagation(); close(); } };
  v.onclick = close;
  window.addEventListener("keydown", onKey, true);
  panel.appendChild(v);
}
/** Small JPEG preview (about 360 px wide) so saved chats stay light. */
function makeThumb(b64: string): Promise<string> {
  return new Promise((resolve) => {
    const img = new Image();
    img.onload = () => {
      const w = Math.min(360, img.naturalWidth);
      const h = Math.max(1, Math.round((img.naturalHeight * w) / img.naturalWidth));
      const c = document.createElement("canvas");
      c.width = w;
      c.height = h;
      c.getContext("2d")?.drawImage(img, 0, 0, w, h);
      resolve(c.toDataURL("image/jpeg", 0.72));
    };
    img.onerror = () => resolve("");
    img.src = `data:image/jpeg;base64,${b64}`;
  });
}
function fillAnswer(e: HTMLElement, text: string, label: string) {
  e.className = "a";
  e.textContent = text;
  const meta = el("div", "meta");
  const copy = el("button", "", "Copy");
  copy.onclick = () => navigator.clipboard.writeText(text).then(() => toast("Copied", "info"));
  meta.append(el("span", "", label), copy);
  e.appendChild(meta);
}
function renderChat() {
  chat.replaceChildren();
  const msgs = S.cur().messages;
  if (!msgs.length) { chat.appendChild(emptyHint()); return; }
  for (const m of msgs) {
    if (m.heard) chat.appendChild(heardEl(m));
    else if (m.role === "user") chat.appendChild(questionEl(m));
    else { const a = el("div", "a"); fillAnswer(a, m.content, m.label ?? ""); chat.appendChild(a); }
  }
  scrollDown();
}
/** Save a message in the current session and show it. `existing` = the "Thinking…" box to fill. */
function addMsg(m: S.ChatMsg, existing?: HTMLElement): HTMLElement {
  S.cur().messages.push(m);
  S.save(m.role === "assistant");
  chat.querySelector(".empty")?.remove();
  let node = existing;
  if (node) fillAnswer(node, m.content, m.label ?? "");
  else {
    node = m.role === "user" ? questionEl(m) : el("div", "a");
    if (m.role === "assistant") fillAnswer(node, m.content, m.label ?? "");
    chat.appendChild(node);
  }
  scrollDown();
  return node;
}
function addPending(): HTMLElement {
  const e = el("div", "a pending", "Thinking…");
  chat.appendChild(e);
  scrollDown();
  return e;
}
/** What we send the AI as chat history (screenshots become a short text note). */
const historyForAI = () =>
  S.cur().messages.filter((m) => !m.heard).map((m) => ({ role: m.role, content: m.shot ? `[screenshot] ${m.content}` : m.content }));
/** The last things heard in the meeting (sent as background with every question). */
function recentHeard(): string {
  const lines = S.cur().messages.filter((m) => m.heard).slice(-8).map((m) => `${m.heard === "me" ? "Me" : "Them"}: ${m.content}`);
  const t = lines.join("\n").slice(-2500);
  return t ? `### What was said in the meeting just now\n${t}` : "";
}
/** If the window is closed, make the bubble glow. */
async function notifyIfHidden() {
  if (!(await getCurrentWindow().isVisible())) void emit("answer-ready");
}
function undoLast(userNode: HTMLElement, text: string) {
  S.cur().messages.pop();
  S.save();
  userNode.remove();
  input.value = text;
  autoGrow();
}

/* One AI request at a time, with a live timer, a Stop button and a time limit. */
let reqId = 0;
let active: { undo: () => void } | null = null;

function setBusy(on: boolean) {
  busy = on;
  sendBtn.textContent = on ? "■" : "➤";
  sendBtn.title = on ? "Stop waiting" : "Send";
}
function withTimeout<T>(p: Promise<T>, ms: number): Promise<T> {
  return Promise.race([
    p,
    new Promise<T>((_, reject) =>
      setTimeout(() => reject(new Error(`No answer after ${Math.round(ms / 1000)} s. Check your internet and the model name in Settings.`)), ms),
    ),
  ]);
}
/** Show the question, wait for the AI, show the answer. On failure the question goes back into the box. */
async function runAI(m: S.ChatMsg, restoreText: string, call: (streamId: string) => Promise<AskResp>, ms: number) {
  const userNode = addMsg(m);
  const id = ++reqId;
  const streamId = uid();
  setBusy(true);
  const pending = addPending();
  const t0 = Date.now();
  let streamed = "";
  const tick = window.setInterval(() => { if (!streamed) pending.textContent = `Thinking… ${Math.round((Date.now() - t0) / 1000)}s`; }, 1000);
  // Show the answer word by word while it is being written.
  const unlisten = await listen<{ id: string; text: string }>("ai-chunk", (e) => {
    if (e.payload.id !== streamId || id !== reqId) return;
    if (!streamed) pending.className = "a streaming";
    streamed += e.payload.text;
    pending.textContent = streamed;
    scrollDown();
  });
  const undo = () => { pending.remove(); undoLast(userNode, restoreText); };
  active = { undo };
  try {
    const r = await withTimeout(call(streamId), ms);
    if (id !== reqId) return; // stopped meanwhile
    addMsg({ role: "assistant", content: r.text, label: modelLabel(r) }, pending);
    void notifyIfHidden();
  } catch (e) {
    if (id !== reqId) return;
    undo();
    toast(e instanceof Error ? e.message : String(e));
  } finally {
    clearInterval(tick);
    try { const r = unlisten() as unknown; if (r instanceof Promise) r.catch(() => undefined); } catch { /* already gone */ }
    if (id === reqId) { setBusy(false); active = null; input.focus(); }
  }
}
function stopWaiting() {
  if (!active) return;
  const a = active;
  reqId++; // ignore whatever comes back
  active = null;
  a.undo();
  setBusy(false);
  toast("Stopped.", "info");
}

async function send() {
  const text = input.value.trim();
  if (busy) { if (text || pendingImg) toast("Still waiting for the last answer. Press ■ to stop it."); return; }
  if (pendingImg) { await askImage(pendingImg.b64, pendingImg.thumb); return; }
  if (!text) return;
  if (!config.answer.length) { toast("Add an Answers model in Settings first."); openSettings(true); return; }
  const history = [...historyForAI(), { role: "user", content: text }];
  input.value = "";
  autoGrow();
  await runAI(
    { role: "user", content: text },
    text,
    (streamId) => invoke<AskResp>("ask", { req: { messages: history, mode, slots: config.answer, context: ctx(text), streamId } }),
    160_000,
  );
}

/* ---------- screenshots and images ---------- */
// A screenshot or pasted image waits above the box until you press Enter.
let pendingImg: { b64: string; thumb?: string } | null = null;
const imgBar = $<HTMLDivElement>("imgBar");
function renderImg() {
  imgBar.replaceChildren();
  imgBar.hidden = !pendingImg;
  if (!pendingImg) return;
  const img = el("img") as HTMLImageElement;
  img.src = pendingImg.thumb || `data:image/jpeg;base64,${pendingImg.b64}`;
  const x = el("button", "ib", "✕") as HTMLButtonElement;
  x.title = "Remove image";
  x.onclick = () => { pendingImg = null; renderImg(); input.focus(); };
  imgBar.append(img, el("span", "", "Type your question and press Enter"), x);
}
async function attachImage(b64: string) {
  pendingImg = { b64 };
  const t = await makeThumb(b64);
  if (t && pendingImg?.b64 === b64) pendingImg.thumb = t;
  renderImg();
  input.focus();
}
async function askImage(b64: string, thumb?: string) {
  if (busy) { toast("Wait for the current answer first (or press ■)."); return; }
  const slots = config.vision.length ? config.vision : config.answer;
  if (!slots.length) { toast("Add a Vision or Answers model in Settings first."); openSettings(true); return; }
  const instruction = input.value.trim();
  input.value = "";
  autoGrow();
  pendingImg = null;
  renderImg();
  const msg: S.ChatMsg = { role: "user", content: instruction || "Screenshot", shot: true };
  const t = thumb ?? (await makeThumb(b64));
  if (t) msg.thumb = t;
  fullImages.set(msg, `data:image/jpeg;base64,${b64}`);
  await runAI(
    msg,
    instruction,
    (streamId) => invoke<AskResp>("ask_image", { req: { image: b64, instruction, mode, slots, context: ctx(instruction || "question"), streamId } }),
    160_000,
  );
}
async function screenshotNow() {
  try { await attachImage(await invoke<string>("take_screenshot")); } catch (e) { toast(String(e)); }
}

/* ---------- files for context ---------- */
const short = (n: string) => (n.length > 22 ? n.slice(0, 19) + "…" : n);
function renderFiles() {
  const files = S.cur().files;
  filesEl.hidden = !files.length;
  filesEl.replaceChildren();
  for (const f of files) {
    const chip = el("span", "fchip");
    chip.title = `${f.name} · ${f.text.length.toLocaleString()} characters`;
    const x = el("button", "", "✕");
    x.onclick = () => { S.cur().files = S.cur().files.filter((g) => g.id !== f.id); S.save(true); renderFiles(); };
    chip.append(el("span", "", short(f.name)), x);
    filesEl.appendChild(chip);
  }
}
async function handleLoaded(l: Loaded) {
  if (l.kind === "image") { await attachImage(l.image); return; }
  const s = S.cur();
  s.files = s.files.filter((f) => f.name !== l.name);
  s.files.push({ id: uid(), name: l.name, text: l.text });
  S.save(true);
  renderFiles();
  toast(`Added ${l.name} (${l.text.length.toLocaleString()} characters)`, "info");
}
function toBase64(file: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(String(r.result).split(",")[1] ?? "");
    r.onerror = () => reject(new Error("Could not read the file"));
    r.readAsDataURL(file);
  });
}
async function loadFromBrowserFile(file: File | Blob, name: string) {
  try {
    if (file.size > 12 * 1024 * 1024) { toast(`${name} is too big (max 12 MB)`); return; }
    await handleLoaded(await invoke<Loaded>("load_bytes", { name, data: await toBase64(file) }));
  } catch (e) { toast(String(e)); }
}

/* ---------- sessions ---------- */
function updateSessName() { $("sessName").textContent = S.cur().name; }
function afterSwitch() { renderChat(); renderFiles(); updateSessName(); }
function showSessions(open: boolean) {
  sessionsEl.hidden = !open;
  if (open) { settingsEl.hidden = true; renderSessions(); }
}
function openSettings(open: boolean) {
  settingsEl.hidden = !open;
  if (open) { sessionsEl.hidden = true; void refreshKeys(); }
}
function renderSessions() {
  sessionsEl.replaceChildren();
  const add = el("button", "btn", "+ New session");
  add.onclick = () => { S.create(); afterSwitch(); showSessions(false); input.focus(); };
  sessionsEl.append(add, el("div", "hint", "Each session keeps its own chat and files."));
  for (const s of S.all()) {
    const rowEl = el("div", "srow" + (s.id === S.cur().id ? " on" : ""));
    const open = el("button", "sname");
    open.append(el("span", "", s.name), el("small", "", `${s.messages.length} messages · ${s.files.length} files`));
    open.onclick = () => { S.switchTo(s.id); afterSwitch(); showSessions(false); };
    const ren = el("button", "ib", "✎");
    ren.title = "Rename";
    ren.onclick = () => {
      const f = el("input"); f.type = "text"; f.value = s.name; f.className = "rename";
      open.replaceWith(f);
      f.focus(); f.select();
      const done = () => { S.rename(s.id, f.value); updateSessName(); renderSessions(); };
      f.onkeydown = (e) => { if (e.key === "Enter") done(); if (e.key === "Escape") renderSessions(); };
      f.onblur = done;
    };
    const del = el("button", "ib", "🗑");
    del.title = "Delete";
    let armed = false;
    del.onclick = () => {
      if (!armed) { armed = true; del.textContent = "Sure?"; setTimeout(() => { armed = false; del.textContent = "🗑"; }, 2500); return; }
      S.remove(s.id);
      afterSwitch();
      renderSessions();
    };
    rowEl.append(open, ren, del);
    sessionsEl.append(rowEl);
  }
}

/* ---------- listening mode ----------
 * Click the mic: what people say appears live in the chat (as "Them" / "Me" bubbles).
 * Each bubble has "Answer this". Auto mode also answers questions by itself.
 */
interface Heard { speaker: "them" | "me"; text: string }
interface DetectResp { question: string | null; answer: string | null; provider: string; model: string }
const listenBtn = $("listenBtn"), live = $("live");
type Phase = "off" | "on" | "finishing";
let phase: Phase = "off";
let heard: Heard[] = [];
let doneSources = 0;
let lastQ = "";
let detecting = false;
let rerun = false;
let detTimer: number | undefined;
let finishTimer: number | undefined;
const listenCfg = () => ({ mic: false, language: "", mode: "manual", spoken: "auto3", toEnglish: true, ...store.get<Record<string, unknown>>("listen", {}) }) as { mic: boolean; language: string; mode: string; spoken: string; toEnglish: boolean };
/** Language for answers: the one set in Settings, else English when "Convert to English" is on. */
const answerLang = () => listenCfg().language || (listenCfg().toEnglish ? "English" : "");
/** True when the text has letters that are not English (Bengali, Hindi...). */
const notEnglish = (t: string) => /[^\p{Script=Latin}\P{L}]/u.test(t);
const isAuto = () => listenCfg().mode === "auto";
const normQ = (s: string) => s.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, "");

/* Status strip: sound meter + state (the words themselves go into the chat). */
const meter = { them: 0, me: 0 };
let lastSound = 0;
let listenStart = 0;
let statusTick: number | undefined;
function renderLive(status = "") {
  listenBtn.classList.toggle("rec", phase === "on");
  listenBtn.title = phase === "on" ? "Stop listening" : "Start listening";
  live.hidden = phase === "off";
  if (phase === "off") return;
  live.replaceChildren();
  live.classList.remove("rec-mode");
  if (phase === "finishing") { live.append(el("i", "", status || "Finishing the last sentence…")); return; }
  const bar = (v: number) => { const b = el("span", "meter"); const f = el("span"); f.style.width = `${Math.round(v * 100)}%`; b.append(f); return b; };
  live.append(el("b", "", "● Live"), " Them ", bar(meter.them));
  if (listenCfg().mic) live.append(" Me ", bar(meter.me));
  const quietFor = Date.now() - Math.max(lastSound, listenStart);
  if (quietFor > 8000) live.append(el("br"), el("small", "", "No sound from your speakers. Is the meeting playing on your default speakers/headphones?"));
  else if (status) live.append(" ", el("i", "", status));
}
function renderLiveMsg(msg: string) {
  live.hidden = false;
  live.replaceChildren(el("i", "", msg));
}

/* Live transcript bubbles in the chat. A bubble keeps growing while the same person talks. */
let liveMsg: S.ChatMsg | null = null;
let liveNode: HTMLElement | null = null;
let liveAt = 0;
let trQueue: Promise<void> = Promise.resolve();
function heardEl(m: S.ChatMsg): HTMLElement {
  const box = el("div", `heard ${m.heard}`);
  const head = el("div", "who", m.heard === "me" ? "Me" : "Them");
  const ans = el("button", "", "Answer this");
  ans.title = "Ask the AI to answer what was said here";
  ans.onclick = () => void answerHeard(m);
  head.append(ans);
  box.append(head, el("div", "txt", m.content));
  return box;
}
function updateHeard() {
  if (!liveMsg || !liveNode) return;
  const t = liveNode.querySelector(".txt");
  if (t) t.textContent = liveMsg.content;
  scrollDown();
}
function addHeard(h: Heard) {
  const now = Date.now();
  const same = liveMsg && liveMsg.heard === h.speaker && now - liveAt < 8000 && S.cur().messages[S.cur().messages.length - 1] === liveMsg && liveMsg.content.length < 1200;
  if (!same) {
    liveMsg = { role: "user", content: "", heard: h.speaker };
    S.cur().messages.push(liveMsg);
    chat.querySelector(".empty")?.remove();
    liveNode = heardEl(liveMsg);
    chat.appendChild(liveNode);
  }
  const msg = liveMsg!;
  const node = liveNode!;
  liveAt = now;
  // Show the words at once; swap in English when the conversion is back (in order).
  const pieceStart = msg.content.length ? msg.content.length + 1 : 0;
  msg.content = msg.content ? `${msg.content} ${h.text}` : h.text;
  updateHeard();
  S.save();
  if (listenCfg().toEnglish && notEnglish(h.text) && config.answer.length) {
    const original = h.text;
    trQueue = trQueue.then(async () => {
      try {
        const en = (await withTimeout(invoke<string>("translate_text", { req: { text: original, slots: config.answer } }), 20_000)).trim();
        if (!en) return;
        const at = msg.content.indexOf(original, Math.max(0, pieceStart - 5));
        if (at >= 0) msg.content = msg.content.slice(0, at) + en + msg.content.slice(at + original.length);
        const t = node.querySelector(".txt");
        if (t) t.textContent = msg.content;
        S.save();
        if (isAuto() && msg.heard === "them") { heard.push({ speaker: "them", text: en }); scheduleDetect(); }
      } catch { /* keep the original words */ }
    });
  } else if (isAuto() && h.speaker === "them") {
    heard.push({ speaker: "them", text: h.text });
    scheduleDetect();
  }
  if (h.speaker === "me") heard.push(h);
  if (heard.length > 30) heard.splice(0, heard.length - 30);
}
/** "Answer this": send what was heard as a question. */
async function answerHeard(m: S.ChatMsg) {
  if (busy) { toast("Wait for the current answer first (or press ■)."); return; }
  if (!config.answer.length) { toast("Add an Answers model in Settings first."); openSettings(true); return; }
  const q = m.content.trim();
  if (!q) return;
  const lang = answerLang();
  const history = [...historyForAI(), { role: "user", content: `Someone in my meeting just said: "${q}"\nGive me what I should reply${lang ? `, in ${lang}` : ""}.` }];
  await runAI(
    { role: "user", content: q, auto: true },
    "",
    (streamId) => invoke<AskResp>("ask", { req: { messages: history, mode, slots: config.answer, context: ctx(q), streamId } }),
    160_000,
  );
}

async function startListening() {
  if (phase !== "off") return;
  if (isAuto() && !config.answer.length) { toast("Add an Answers model in Settings first."); openSettings(true); return; }
  if (!config.stt.length) {
    config.stt.push(newSlot("stt")); saveConfig(); buildSettings(); openSettings(true);
    toast("Paste a key for Speech-to-text (Groq works), then press the mic again.", "info");
    return;
  }
  if (config.stt.every((s) => !["groq", "openai", "custom", "ollama", "localwhisper"].includes(s.preset))) {
    openSettings(true);
    toast("Speech-to-text needs Groq, OpenAI or Local Whisper (Gemini cannot do it).");
    return;
  }
  const st = await refreshKeys();
  if (!config.stt.some((s) => st[s.id] || isLocalUrl(s.baseUrl))) {
    openSettings(true);
    toast("Paste an API key for Speech-to-text, then press the mic again.", "info");
    return;
  }
  // Local Whisper chosen? Start it hidden (if it is the main one, wait until it is ready).
  const lw = config.stt.findIndex((s) => s.preset === "localwhisper");
  const lcfg = store.get<{ autoWhisper?: boolean; whisperDir?: string }>("listen", {});
  if (lw >= 0 && lcfg.autoWhisper !== false) {
    const dir = lcfg.whisperDir || "D:\\whisper";
    if (lw === 0) {
      phase = "finishing"; // show the strip while it loads
      renderLiveMsg("Starting Local Whisper… (first time can take ~10–30 s)");
      try { toast(await invoke<string>("ensure_local_whisper", { dir }), "info"); }
      catch (e) { toast(String(e)); }
      phase = "off";
      renderLive();
    } else {
      void invoke("ensure_local_whisper", { dir }).catch(() => undefined); // backup: start quietly
    }
  }
  try {
    await invoke("start_listening", { stt: config.stt, mic: listenCfg().mic, spoken: listenCfg().spoken, whole: false });
    phase = "on"; heard = []; lastQ = ""; doneSources = 0; liveMsg = null;
    listenStart = Date.now(); lastSound = 0; meter.them = meter.me = 0;
    clearInterval(statusTick);
    statusTick = window.setInterval(() => { if (phase === "on") renderLive(); }, 1000);
    renderLive();
  } catch (e) { toast(String(e)); }
}
async function stopListening() {
  if (phase !== "on") return;
  clearTimeout(detTimer);
  clearInterval(statusTick);
  phase = "finishing";
  renderLive();
  try { await invoke("stop_listening"); } catch { /* ignore */ }
  clearTimeout(finishTimer);
  finishTimer = window.setTimeout(finishListening, 15_000);
}
function finishListening() {
  if (phase !== "finishing") return;
  clearTimeout(finishTimer);
  phase = "off";
  liveMsg = null;
  renderLive();
  S.save(true);
}
const toggleListening = () => void (phase === "on" ? stopListening() : startListening());

function onHeard(h: Heard) {
  if (phase === "off") return;
  addHeard(h);
}
function scheduleDetect() {
  if (phase !== "on" || !isAuto()) return;
  clearTimeout(detTimer);
  detTimer = window.setTimeout(() => void detect(), 500);
}
function onSourceDone() {
  doneSources++;
  if (phase === "finishing" && doneSources >= (listenCfg().mic ? 2 : 1)) finishListening();
}
// Auto mode only: did "Them" just ask something for me? If yes, show the question + answer.
async function detect() {
  if (phase !== "on" || !config.answer.length) return;
  if (detecting) { rerun = true; return; }
  const lastThem = [...heard].reverse().find((x) => x.speaker === "them");
  if (!lastThem || lastThem.text.split(/\s+/).length < 2) return;
  detecting = true;
  renderLive("thinking…");
  const transcript = heard.slice(-12).map((x) => `${x.speaker === "them" ? "Them" : "Me"}: ${x.text}`).join("\n").slice(-2000);
  const recentThem = heard.filter((x) => x.speaker === "them").slice(-3).map((x) => x.text).join(" ");
  try {
    const r = await invoke<DetectResp>("detect_answer", {
      req: { transcript, mode, slots: config.answer, language: answerLang(), english: listenCfg().toEnglish, context: ctx(recentThem) },
    });
    if (r.question && r.answer && normQ(r.question) !== lastQ) {
      lastQ = normQ(r.question);
      addMsg({ role: "user", content: r.question, auto: true });
      addMsg({ role: "assistant", content: r.answer, label: `${r.provider} · ${r.model}` });
      void notifyIfHidden();
    }
  } catch (e) {
    toast(String(e));
  } finally {
    detecting = false;
    renderLive();
    if (rerun) { rerun = false; detTimer = window.setTimeout(() => void detect(), 300); }
  }
}

/** Attach a click handler. A missing element is reported instead of breaking every other button. */
function onClick(id: string, fn: () => void) {
  const e = document.getElementById(id);
  if (e) e.onclick = fn;
  else console.error(`Missing #${id}: index.html is out of date`);
}

/* ---------- hidden from screen share: on / off ---------- */
const protBtn = document.getElementById("protBtn") ?? el("button");
let armed = false;
function showProtection(hidden: boolean) {
  protBtn.classList.toggle("exposed", !hidden);
  panel.classList.toggle("exposed", !hidden);
  const bar = document.getElementById("exposedBar");
  if (bar) bar.hidden = hidden;
  protBtn.title = hidden ? "Hidden from screen share. Click to make it visible." : "VISIBLE in screen share. Click to hide it.";
}
protBtn.onclick = () => {
  const exposed = protBtn.classList.contains("exposed");
  if (exposed) { void invoke("set_protection", { on: true }); return; } // hiding again: no questions asked
  // Making it visible: ask for a second click so it can never happen by accident.
  if (!armed) {
    armed = true;
    toast("Click the eye again to make the assistant VISIBLE in screen share.");
    setTimeout(() => { armed = false; }, 3000);
    return;
  }
  armed = false;
  void invoke("set_protection", { on: false });
};
void listen<boolean>("protection-changed", (e) => showProtection(e.payload));

/* ---------- wiring ---------- */
onClick("closeBtn", () => void invoke("hide_panel"));
// Chat button: close Settings / Sessions and go back to the chat.
onClick("chatBtn", () => { openSettings(false); showSessions(false); input.focus(); });
onClick("snipBtn", () => void invoke("start_snip").catch((e) => toast(String(e))));
onClick("newBtn", () => { S.create(); afterSwitch(); showSessions(false); input.focus(); });
onClick("sessBtn", () => showSessions(sessionsEl.hidden));
onClick("settingsBtn", () => openSettings(settingsEl.hidden));
onClick("attachBtn", () => $<HTMLInputElement>("filePick").click());
onClick("shotBtn", () => void screenshotNow());
listenBtn.onclick = toggleListening;
sendBtn.onclick = () => (busy ? stopWaiting() : void send());

$<HTMLInputElement>("filePick").onchange = async (e) => {
  const picker = e.target as HTMLInputElement;
  for (const f of Array.from(picker.files ?? [])) await loadFromBrowserFile(f, f.name);
  picker.value = "";
};
input.addEventListener("input", autoGrow);
input.addEventListener("keydown", (e) => {
  if (e.key === "Enter" && !e.shiftKey && !e.isComposing) { e.preventDefault(); void send(); }
});
// Paste an image straight into the box.
input.addEventListener("paste", (e) => {
  for (const item of Array.from(e.clipboardData?.items ?? [])) {
    if (item.type.startsWith("image/")) {
      const f = item.getAsFile();
      if (f) { e.preventDefault(); void loadFromBrowserFile(f, `pasted.${item.type.split("/")[1] || "png"}`); }
      return;
    }
  }
});
// Drop files / images onto the window.
void getCurrentWebview().onDragDropEvent((ev) => {
  const p = ev.payload;
  if (p.type === "enter" || p.type === "over") panel.classList.add("dropping");
  else panel.classList.remove("dropping");
  if (p.type === "drop") {
    for (const path of p.paths) {
      invoke<Loaded>("load_path", { path }).then(handleLoaded).catch((err) => toast(String(err)));
    }
  }
});

window.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    if (!settingsEl.hidden || !sessionsEl.hidden) { openSettings(false); showSessions(false); input.focus(); }
    else void invoke("hide_panel");
  } else if ((e.key === "PageUp" || e.key === "PageDown") && document.activeElement === input) {
    e.preventDefault();
    chat.scrollBy({ top: (e.key === "PageUp" ? -1 : 1) * chat.clientHeight * 0.8 });
  }
});
window.addEventListener("focus", () => {
  if (settingsEl.hidden && sessionsEl.hidden) input.focus();
  void emit("panel-seen");
});
void listen("cycle-mode", () => {
  const i = MODES.findIndex((m) => m.id === mode);
  setMode(MODES[(i + 1) % MODES.length].id, true);
});
void listen<Heard>("transcript", (e) => onHeard(e.payload));
void listen<string>("listen-error", (e) => {
  toast(String(e.payload));
  if (String(e.payload).startsWith("Audio stopped")) { if (phase === "on") void stopListening(); else finishListening(); }
});
void listen<string>("app-error", (e) => toast(String(e.payload)));
void listen("toggle-listening", toggleListening);
void listen<string>("listen-done", () => onSourceDone());
void listen<{ speaker: "them" | "me"; level: number }>("listen-meter", (e) => {
  meter[e.payload.speaker === "me" ? "me" : "them"] = e.payload.level;
  if (e.payload.level > 0.03) lastSound = Date.now();
  if (phase === "on") renderLive();
});
void listen<string>("screenshot", (e) => void attachImage(e.payload));

/* ---------- start ---------- */
async function init() {
  applyAppearance();
  applyOpacity();
  await S.load();
  // First run: one empty Groq answer slot, so you only have to paste a key.
  if (!config.answer.length && !config.vision.length && !config.stt.length) { config.answer.push(newSlot("answer")); saveConfig(); }
  renderChips();
  buildSettings();
  afterSwitch();
  void restoreHotkeys();
  await invoke("share_keys", { slots: [...config.answer, ...config.vision, ...config.stt] }).catch(() => undefined);
  void invoke<boolean>("get_protection").then(showProtection).catch(() => undefined);
  const st = await refreshKeys();
  if (!config.answer.some((s) => st[s.id] || isLocalUrl(s.baseUrl))) {
    openSettings(true);
    toast("Paste an API key for your Answers model to start.", "info");
  }
}
void init();
