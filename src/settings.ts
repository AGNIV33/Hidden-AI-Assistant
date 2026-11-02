import { invoke } from "@tauri-apps/api/core";
import { $, el, store, toast } from "./util";
import { PRESETS, ROLES, DEFAULT_MODEL, allSlots, config, isLocalUrl, newSlot, saveConfig, type Role, type Slot } from "./config";

/* ---------- appearance ---------- */
const THEMES: Record<string, { name: string; v: Record<string, string> }> = {
  midnight: { name: "Midnight", v: { "--bg": "rgba(18,18,24,.94)", "--solid": "#121218", "--field": "#1b1b24", "--surface": "rgba(255,255,255,.06)", "--border": "rgba(255,255,255,.10)", "--text": "#EDEDF2", "--dim": "#9A9AA8", "--accent": "#7C6CFF" } },
  light: { name: "Light", v: { "--bg": "rgba(250,250,252,.96)", "--solid": "#FAFAFC", "--field": "#FFFFFF", "--surface": "rgba(0,0,0,.05)", "--border": "rgba(0,0,0,.13)", "--text": "#1B1B22", "--dim": "#666675", "--accent": "#5B4CFF" } },
  ocean: { name: "Ocean", v: { "--bg": "rgba(10,25,40,.94)", "--solid": "#0A1928", "--field": "#12263A", "--surface": "rgba(255,255,255,.06)", "--border": "rgba(255,255,255,.10)", "--text": "#E6F2FF", "--dim": "#8FA9C2", "--accent": "#2BB3FF" } },
  forest: { name: "Forest", v: { "--bg": "rgba(14,26,20,.94)", "--solid": "#0E1A14", "--field": "#162A20", "--surface": "rgba(255,255,255,.06)", "--border": "rgba(255,255,255,.10)", "--text": "#E8F5EC", "--dim": "#8FB09B", "--accent": "#3DDC97" } },
  rose: { name: "Rose", v: { "--bg": "rgba(30,16,22,.94)", "--solid": "#1E1016", "--field": "#2C1A22", "--surface": "rgba(255,255,255,.06)", "--border": "rgba(255,255,255,.10)", "--text": "#FCEAF0", "--dim": "#C79BAB", "--accent": "#FF6FA1" } },
};
interface Appearance { theme: string; accent: string; fontSize: number }
const appearance = (): Appearance => ({ theme: "midnight", accent: "", fontSize: 14, ...store.get<Partial<Appearance>>("appearance", {}) });

export function applyAppearance() {
  const a = appearance();
  const root = document.documentElement.style;
  const t = THEMES[a.theme] ?? THEMES.midnight;
  for (const [k, v] of Object.entries(t.v)) root.setProperty(k, v);
  if (a.accent) root.setProperty("--accent", a.accent);
  root.setProperty("--fs", `${a.fontSize}px`);
}
export function applyOpacity() {
  $("panel").style.opacity = String(store.get("opacity", 100) / 100);
}

/* ---------- hotkeys ---------- */
const HOTKEYS = [
  { id: "toggle", label: "Show / hide", def: "Ctrl+Alt+Space" },
  { id: "screenshot", label: "Screenshot", def: "Ctrl+Alt+S" },
  { id: "snip", label: "Select area", def: "Ctrl+Alt+X" },
  { id: "listen", label: "Listening", def: "Ctrl+Alt+L" },
  { id: "mode", label: "Answer mode", def: "Ctrl+Alt+M" },
  { id: "bubble", label: "Bubble", def: "Ctrl+Alt+B" },
  { id: "protect", label: "Hide from share", def: "Ctrl+Alt+H" },
  { id: "quit", label: "Quit app", def: "Ctrl+Alt+Q" },
];
const savedKeys = () => store.get<Record<string, string>>("hotkeys", {});
const pretty = (combo: string) => combo.replace(/Key([A-Z])/g, "$1").replace(/Digit(\d)/g, "$1");

/** Tell the backend which hotkeys to use (called at start-up and after changes). */
export async function applyHotkeys(): Promise<Record<string, string>> {
  return invoke<Record<string, string>>("set_hotkeys", { map: savedKeys() });
}
export async function restoreHotkeys() {
  if (!Object.keys(savedKeys()).length) return;
  const errors = await applyHotkeys();
  const first = Object.values(errors)[0];
  if (first) toast(first);
}

/* ---------- API key status ---------- */
export async function refreshKeys() {
  const slots = allSlots();
  const st = slots.length ? await invoke<Record<string, boolean>>("key_status", { slots }) : {};
  for (const s of allSlots()) {
    document.getElementById(`dot-${s.id}`)?.classList.toggle("ok", !!st[s.id]);
    document.getElementById(`key-${s.id}`)?.setAttribute(
      "placeholder",
      st[s.id] ? "Saved ✓ (paste to replace)" : isLocalUrl(s.baseUrl) ? "Not needed for local" : "Paste API key",
    );
  }
  return st;
}

/* ---------- small builders ---------- */
function row(label: string, ...kids: HTMLElement[]) {
  const r = el("div", "row");
  r.append(el("label", "", label), ...kids);
  return r;
}
function range(min: number, max: number, value: number, onInput: (v: number) => void) {
  const i = el("input");
  i.type = "range";
  i.min = String(min);
  i.max = String(max);
  i.value = String(value);
  i.oninput = () => onInput(Number(i.value));
  return i;
}

/* ---------- smart setup: provider from the key, best model from the list ---------- */
const KEY_PREFIX: [RegExp, string][] = [
  [/^AIza/, "gemini"], [/^gsk_/, "groq"], [/^nvapi-/, "nvidia"], [/^sk-ant-/, "anthropic"],
  [/^sk-or-/, "openrouter"], [/^sk-/, "openai"],
];
const NOT_CHAT = /embed|tts|imagen|image-|veo|whisper|transcrib|guard|moderation|live|audio|realtime|lyria|aqa|search|robotics|computer-use/i;
const PREFER: Record<Role, RegExp[]> = {
  answer: [/^gemini-[\d.]+-flash-lite$/, /^gemini-[\d.]+-flash$/, /llama-3\.3-70b/, /gpt-[\w.]*mini/, /claude-[\w-]*(haiku|sonnet)/, /llama.*70b.*instruct/, /./],
  vision: [/^gemini-[\d.]+-flash-lite$/, /^gemini-[\d.]+-flash$/, /llama-3\.2-11b-vision/, /llama-3\.2-90b-vision/, /vision|-vl|omni|gpt-4o/, /claude-[\w-]*(haiku|sonnet)/],
  stt: [/whisper-large-v3-turbo/, /whisper/, /transcrib/],
};
/** Bigger version numbers first; stable before preview/experimental. */
function newest(names: string[]): string | undefined {
  const ver = (n: string) => (n.match(/\d+(\.\d+)?/g) ?? []).map(Number);
  return [...names].sort((a, b) => {
    const pa = /preview|exp/i.test(a) ? 1 : 0, pb = /preview|exp/i.test(b) ? 1 : 0;
    if (pa !== pb) return pa - pb;
    const va = ver(a), vb = ver(b);
    for (let i = 0; i < Math.max(va.length, vb.length); i++) if ((vb[i] ?? 0) !== (va[i] ?? 0)) return (vb[i] ?? 0) - (va[i] ?? 0);
    return a.localeCompare(b);
  })[0];
}
function pickModel(role: Role, names: string[]): string | undefined {
  const pool = role === "stt" ? names : names.filter((n) => !NOT_CHAT.test(n));
  for (const re of PREFER[role]) {
    const hit = newest(pool.filter((n) => re.test(n)));
    if (hit) return hit;
  }
  return undefined;
}

/* ---------- one AI connection card ---------- */
function slotCard(role: Role, slot: Slot, i: number, rebuild: () => void): HTMLElement {
  const card = el("div", "slot");
  card.dataset.id = slot.id;
  const grip = el("span", "grip", "⠿");
  grip.title = "Drag up or down. The top one is the Main model.";
  const dot = el("span", "dot");
  dot.id = `dot-${slot.id}`;
  const badge = el("span", "badge", i === 0 ? "Main" : `Backup ${i}`);
  const preset = el("select");
  for (const p of PRESETS) { const o = el("option", "", p.name); o.value = p.id; preset.append(o); }
  preset.value = slot.preset;
  const rm = el("button", "btn", "✕");
  rm.title = "Remove this connection";
  const top = el("div", "row");
  top.append(grip, dot, badge, preset, rm);

  const url = el("input"); url.type = "text"; url.value = slot.baseUrl; url.placeholder = "https://…/v1";
  const key = el("input"); key.type = "password"; key.id = `key-${slot.id}`; key.autocomplete = "off";
  const saveBtn = el("button", "btn", "Save");
  const model = el("input"); model.type = "text"; model.value = slot.model; model.placeholder = "model name";
  const list = el("datalist"); list.id = `dl-${slot.id}`; model.setAttribute("list", list.id);
  const loadBtn = el("button", "btn", "Load");
  loadBtn.title = "Test the key and load this provider's model names";

  const saveKey = async () => {
    const typed = key.value.trim();
    // Switch provider automatically when the key clearly belongs to another one.
    const guess = KEY_PREFIX.find(([re]) => re.test(typed))?.[1];
    if (role === "stt" && guess && !["groq", "openai"].includes(guess)) {
      throw new Error("That key is for a provider without speech-to-text. Use a Groq key (console.groq.com/keys) here.");
    }
    if (guess && guess !== slot.preset) {
      const p = PRESETS.find((x) => x.id === guess)!;
      slot.preset = guess; preset.value = guess;
      slot.baseUrl = p.url; url.value = p.url;
      saveConfig();
    }
    await invoke("save_key", { id: slot.id, key: typed, baseUrl: slot.baseUrl });
    key.value = "";
    await refreshKeys();
  };
  /** Ask the provider which models this key can use, and pick the best one if the current one is not available. */
  const autoModel = async (): Promise<string> => {
    const names = await invoke<string[]>("list_models", { slot });
    list.replaceChildren(...names.map((n) => { const o = el("option"); o.value = n; return o; }));
    if (!names.includes(slot.model)) {
      const best = pickModel(role, names);
      if (best) { slot.model = best; model.value = best; saveConfig(); }
    }
    return slot.model;
  };
  preset.onchange = () => {
    if (role === "stt" && !["groq", "openai", "custom", "ollama", "localwhisper"].includes(preset.value)) {
      toast("Speech-to-text needs Groq or OpenAI. Google Gemini, Claude and NVIDIA do not offer it here.");
    }
    const oldDefault = DEFAULT_MODEL[slot.preset]?.[role];
    slot.preset = preset.value;
    const p = PRESETS.find((x) => x.id === slot.preset)!;
    if (p.url) { slot.baseUrl = p.url; url.value = p.url; }
    // A model name from the old provider will not exist on the new one: swap it for the new default (or clear it).
    if (!slot.model || slot.model === oldDefault) {
      slot.model = DEFAULT_MODEL[slot.preset]?.[role] ?? "";
      model.value = slot.model;
      if (!slot.model) toast("Now click Load and pick a model for this provider.", "info");
    }
    saveConfig();
    void refreshKeys();
  };
  url.onchange = () => { slot.baseUrl = url.value.trim(); saveConfig(); void refreshKeys(); };
  model.onchange = () => {
    slot.model = model.value.trim();
    saveConfig();
    if (/\s/.test(slot.model)) toast("Model names have no spaces (e.g. gemini-3.1-flash-lite). Click Load and pick one from the list.");
  };
  const setupNow = async () => {
    try {
      if (key.value.trim()) await saveKey();
      slot.baseUrl = url.value.trim();
      saveConfig();
      const m = await autoModel();
      toast(m ? `Ready ✓ Using ${m}` : "Key works, but no suitable model found. Pick one in the Model box.", m ? "info" : "err");
    } catch (e) { toast(String(e)); }
  };
  saveBtn.onclick = setupNow;
  loadBtn.onclick = () => {
    if (slot.preset === "localwhisper") { toast("Local Whisper needs no key or model list. Just start it from the Desktop shortcut.", "info"); return; }
    void setupNow();
  };
  key.onkeydown = (e) => { if (e.key === "Enter") void setupNow(); };
  rm.onclick = () => {
    void invoke("delete_key", { id: slot.id }).catch(() => undefined);
    config[role].splice(i, 1);
    saveConfig();
    rebuild();
  };

  card.append(top, row("URL", url), row("API key", key, saveBtn), row("Model", model, loadBtn), list);
  return card;
}

/** Drag a card by its ⠿ handle to change the order (top = Main, the rest = backups). */
function makeSortable(list: HTMLElement, role: Role, rebuild: () => void) {
  list.querySelectorAll<HTMLElement>(".grip").forEach((grip) => {
    grip.onpointerdown = (e) => {
      if (e.button !== 0) return;
      const card = grip.closest(".slot") as HTMLElement;
      const scroller = $("settings");
      e.preventDefault();
      card.classList.add("dragging");
      // Listen on the whole window: moving the card in the page would break pointer capture.
      const move = (ev: PointerEvent) => {
        const others = [...list.querySelectorAll<HTMLElement>(".slot:not(.dragging)")];
        const before = others.find((o) => { const r = o.getBoundingClientRect(); return ev.clientY < r.top + r.height / 2; }) ?? null;
        if (card.nextElementSibling !== before && card !== before) list.insertBefore(card, before);
        const b = scroller.getBoundingClientRect(); // scroll while dragging near the edges
        if (ev.clientY < b.top + 30) scroller.scrollTop -= 12;
        else if (ev.clientY > b.bottom - 30) scroller.scrollTop += 12;
      };
      const up = () => {
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", up);
        window.removeEventListener("pointercancel", up);
        card.classList.remove("dragging");
        const order = [...list.querySelectorAll<HTMLElement>(".slot")].map((c) => c.dataset.id ?? "");
        const was = config[role].map((x) => x.id).join();
        config[role].sort((a, b) => order.indexOf(a.id) - order.indexOf(b.id));
        if (config[role].map((x) => x.id).join() !== was) {
          saveConfig();
          toast(`Main model is now ${config[role][0].model || config[role][0].preset}`, "info");
        }
        rebuild();
      };
      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", up);
      window.addEventListener("pointercancel", up);
    };
  });
}

/* ---------- the whole Settings screen ---------- */
export function buildSettings() {
  const box = $("settings");
  const scroll = box.scrollTop;
  box.replaceChildren();
  const rebuild = () => buildSettings();

  // Appearance
  const a = appearance();
  const theme = el("select");
  for (const [id, t] of Object.entries(THEMES)) { const o = el("option", "", t.name); o.value = id; theme.append(o); }
  theme.value = a.theme in THEMES ? a.theme : "midnight";
  const accent = el("input"); accent.type = "color";
  accent.value = a.accent || (THEMES[theme.value] ?? THEMES.midnight).v["--accent"];
  const accentReset = el("button", "btn", "Reset");
  theme.onchange = () => { store.set("appearance", { ...appearance(), theme: theme.value }); applyAppearance(); accent.value = appearance().accent || THEMES[theme.value].v["--accent"]; };
  accent.oninput = () => { store.set("appearance", { ...appearance(), accent: accent.value }); applyAppearance(); };
  accentReset.onclick = () => { store.set("appearance", { ...appearance(), accent: "" }); applyAppearance(); accent.value = THEMES[appearance().theme]?.v["--accent"] ?? "#7C6CFF"; };
  box.append(
    el("h3", "", "Appearance"),
    row("Theme", theme),
    row("Accent", accent, accentReset),
    row("Text size", range(12, 18, a.fontSize, (v) => { store.set("appearance", { ...appearance(), fontSize: v }); applyAppearance(); })),
    row("Opacity", range(25, 100, store.get("opacity", 100), (v) => { store.set("opacity", v); applyOpacity(); })),
  );

  // Hotkeys
  box.append(el("h3", "", "Hotkeys"), el("div", "hint", "Click a box, then press the keys you want. Must include Ctrl or Alt. Defaults use Ctrl+Alt so they do not clash with Teams or Zoom shortcuts."));
  const resume = () => void applyHotkeys();
  for (const h of HOTKEYS) {
    const inp = el("input");
    inp.type = "text";
    inp.readOnly = true;
    inp.className = "hk";
    inp.value = pretty(savedKeys()[h.id] || h.def);
    inp.onfocus = () => { inp.value = "Press keys…"; void invoke("pause_hotkeys"); };
    inp.onblur = () => { inp.value = pretty(savedKeys()[h.id] || h.def); resume(); };
    inp.onkeydown = async (e) => {
      e.preventDefault();
      if (["Control", "Shift", "Alt", "Meta"].includes(e.key)) return;
      if (e.key === "Escape") { inp.blur(); return; }
      if (!e.ctrlKey && !e.altKey) { toast("Include Ctrl or Alt in the combination."); return; }
      const combo = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.code].filter(Boolean).join("+");
      const prev = savedKeys();
      store.set("hotkeys", { ...prev, [h.id]: combo });
      const errors = await applyHotkeys();
      if (errors[h.id]) { toast(errors[h.id]); store.set("hotkeys", prev); await applyHotkeys(); }
      else toast(`${h.label}: ${pretty(combo)}`, "info");
      inp.blur();
    };
    box.append(row(h.label, inp));
  }
  const resetKeys = el("button", "btn", "Reset hotkeys");
  resetKeys.onclick = async () => { store.set("hotkeys", {}); await applyHotkeys(); rebuild(); };
  box.append(resetKeys);

  // General
  const auto = el("input"); auto.type = "checkbox";
  void invoke<boolean>("get_autostart").then((on) => { auto.checked = on; }).catch(() => undefined);
  auto.onchange = async () => {
    try { await invoke("set_autostart", { on: auto.checked }); toast(auto.checked ? "Will start with Windows" : "Will not start with Windows", "info"); }
    catch (e) { auto.checked = !auto.checked; toast(String(e)); }
  };
  const budget = el("input"); budget.type = "number"; budget.min = "2000"; budget.max = "30000"; budget.step = "1000";
  budget.value = String(store.get("ctxBudget", 12000));
  budget.onchange = () => { const v = Math.min(30000, Math.max(2000, Number(budget.value) || 12000)); budget.value = String(v); store.set("ctxBudget", v); };
  box.append(
    el("h3", "", "General"),
    row("Start with Windows", auto),
    row("File text limit", budget),
    el("div", "hint", "File text limit = most characters from your files sent with each question. Bigger files send only the parts that match the question. Lower it if your free plan hits token limits."),
  );

  // About me: always sent along, so answers fit you (your background, projects, how you talk)
  const prof = { on: true, text: "", ...store.get<Record<string, unknown>>("profile", {}) } as { on: boolean; text: string };
  const profOn = el("input"); profOn.type = "checkbox"; profOn.checked = prof.on;
  const profText = el("textarea", "profile");
  profText.placeholder = "Who you are, your role, skills, projects, company, how you like to answer… (or fill from your resume file)";
  profText.value = prof.text;
  profText.maxLength = 6000;
  const saveProf = () => store.set("profile", { on: profOn.checked, text: profText.value.slice(0, 6000) });
  profOn.onchange = saveProf;
  profText.onchange = saveProf;
  const fromFile = el("button", "btn", "Fill from file");
  const picker = el("input"); picker.type = "file"; picker.accept = ".pdf,.docx,.txt,.md"; picker.hidden = true;
  fromFile.onclick = () => picker.click();
  picker.onchange = async () => {
    const f = picker.files?.[0];
    picker.value = "";
    if (!f) return;
    try {
      const data = await new Promise<string>((res, rej) => { const r = new FileReader(); r.onload = () => res(String(r.result).split(",")[1] ?? ""); r.onerror = () => rej(new Error("Could not read the file")); r.readAsDataURL(f); });
      const l = await invoke<{ kind: string; text: string }>("load_bytes", { name: f.name, data });
      if (l.kind !== "text") throw new Error("Pick a PDF, Word or text file");
      profText.value = l.text.slice(0, 6000);
      saveProf();
      toast(`Filled from ${f.name}${l.text.length > 6000 ? " (first 6,000 characters)" : ""}`, "info");
    } catch (e) { toast(e instanceof Error ? e.message : String(e)); }
  };
  box.append(
    el("h3", "", "About me"),
    el("div", "hint", "Sent with every question in every session, so answers sound like you and use your real background. Up to 6,000 characters."),
    row("Use it", profOn, fromFile),
    profText,
    picker,
  );

  // AI connections
  for (const r of ROLES) {
    box.append(el("h3", "", r.title), el("div", "hint", r.hint));
    const listEl = el("div", "slots");
    config[r.id].forEach((s, i) => listEl.append(slotCard(r.id, s, i, rebuild)));
    box.append(listEl);
    makeSortable(listEl, r.id, rebuild);
    if (config[r.id].length > 1) box.append(el("div", "hint", "Drag ⠿ to change the order. The top one is used first."));
    const add = el("button", "btn", config[r.id].length ? "+ Add backup" : "+ Add model");
    add.onclick = () => { config[r.id].push(newSlot(r.id)); saveConfig(); rebuild(); };
    box.append(add);
  }

  // Listening
  const cfg = { mic: false, language: "", mode: "manual", autoWhisper: true, whisperDir: "D:\\whisper", ...store.get<Record<string, unknown>>("listen", {}) } as { mic: boolean; language: string; mode: string; autoWhisper: boolean; whisperDir: string };
  const autoW = el("input"); autoW.type = "checkbox"; autoW.checked = cfg.autoWhisper;
  autoW.onchange = () => store.set("listen", { ...store.get("listen", cfg), autoWhisper: autoW.checked });
  const wDir = el("input"); wDir.type = "text"; wDir.value = cfg.whisperDir; wDir.placeholder = "D:\\whisper";
  wDir.onchange = () => store.set("listen", { ...store.get("listen", cfg), whisperDir: wDir.value.trim() || "D:\\whisper" });
  const how = el("select");
  for (const [v, t] of [["manual", "Live transcript (tap \u2018Answer this\u2019 on any line)"], ["auto", "Live transcript + answer questions by itself"]]) {
    const o = el("option", "", t); o.value = v; how.append(o);
  }
  how.value = cfg.mode === "auto" ? "auto" : "manual";
  how.onchange = () => store.set("listen", { ...store.get("listen", cfg), mode: how.value });
  const mic = el("input"); mic.type = "checkbox"; mic.checked = cfg.mic;
  mic.onchange = () => store.set("listen", { ...store.get("listen", cfg), mic: mic.checked });
  const spoken = el("select");
  for (const [v, t] of [["auto3", "Auto: Hindi, Bengali, English (mixed is ok)"], ["en", "English only"], ["hi", "Hindi only"], ["bn", "Bengali only"], ["any", "Any language"]]) {
    const o = el("option", "", t); o.value = v; spoken.append(o);
  }
  spoken.value = String(store.get<Record<string, unknown>>("listen", {}).spoken ?? "auto3");
  spoken.onchange = () => store.set("listen", { ...store.get("listen", cfg), spoken: spoken.value });
  const toEn = el("input"); toEn.type = "checkbox"; toEn.checked = store.get<Record<string, unknown>>("listen", {}).toEnglish !== false;
  toEn.onchange = () => store.set("listen", { ...store.get("listen", cfg), toEnglish: toEn.checked });
  const lang = el("input"); lang.type = "text"; lang.placeholder = "English (or same as speaker if Convert is off)"; lang.value = cfg.language;
  lang.onchange = () => store.set("listen", { ...store.get("listen", cfg), language: lang.value.trim() });
  box.append(
    el("h3", "", "Listening"),
    el("div", "hint", "Hears whatever plays on your default speakers or headphones. 'My mic' also hears you. Use headphones with it, or the other person may be heard twice."),
    row("Mode", how),
    row("My mic", mic),
    row("People speak", spoken),
    el("div", "hint", "If a meeting is all in one language, pick it here. That gives the best results."),
    row("Convert to English", toEn),
    el("div", "hint", "Bengali or Hindi speech is turned into English text, and answers come in English (unless 'Answer in' says otherwise)."),
    row("Answer in", lang),
    el("div", "hint", "Local Whisper: if you use it for Speech-to-text, the app starts it hidden in the background when you press the mic, and stops it when you quit. No black window needed."),
    row("Auto-start", autoW),
    row("Whisper folder", wDir),
  );

  const warn = el("div", "warn");
  warn.append(
    "Keys are stored in Windows Credential Manager, never in plain files. ",
    "Free AI plans may keep or use what you send, so avoid confidential client data. ",
    "Chats and file text are saved on this computer only.",
  );
  box.append(warn);
  box.scrollTop = scroll;
  void refreshKeys();
}
