import { invoke } from "@tauri-apps/api/core";
import { uid } from "./util";

export interface ChatMsg {
  role: "user" | "assistant";
  content: string;
  label?: string; // which model answered
  auto?: boolean; // question heard in a meeting
  shot?: boolean; // came from a screenshot / image
  thumb?: string; // small preview of that image (data URL)
}
export interface CtxFile { id: string; name: string; text: string }
export interface Session { id: string; name: string; created: number; messages: ChatMsg[]; files: CtxFile[] }
interface State { current: string; sessions: Session[] }

let state: State = { current: "", sessions: [] };
let timer: number | undefined;

const fix = (s: Partial<Session>): Session => ({
  id: s.id ?? uid(),
  name: s.name ?? "Session",
  created: s.created ?? Date.now(),
  messages: Array.isArray(s.messages) ? s.messages : [],
  files: Array.isArray(s.files) ? s.files : [],
});

export async function load() {
  try {
    const raw = await invoke<string>("load_state");
    if (raw) {
      const p = JSON.parse(raw) as Partial<State>;
      if (Array.isArray(p.sessions)) state = { current: p.current ?? "", sessions: p.sessions.map(fix) };
    }
  } catch {
    /* start fresh */
  }
  if (!state.sessions.length) create();
  if (!state.sessions.some((s) => s.id === state.current)) state.current = state.sessions[0].id;
}

/** Save to disk. Quick changes are batched; `now` saves immediately. */
export function save(now = false) {
  clearTimeout(timer);
  const write = () => {
    for (const s of state.sessions) if (s.messages.length > 300) s.messages.splice(0, s.messages.length - 300);
    void invoke("save_state", { json: JSON.stringify(state) }).catch(() => undefined);
  };
  if (now) write();
  else timer = window.setTimeout(write, 300);
}

export const all = () => state.sessions;
export const cur = (): Session => state.sessions.find((s) => s.id === state.current) ?? state.sessions[0];

export function create(name?: string): Session {
  const s: Session = { id: uid(), name: name || `Session ${state.sessions.length + 1}`, created: Date.now(), messages: [], files: [] };
  state.sessions.unshift(s);
  state.current = s.id;
  save(true);
  return s;
}
export function switchTo(id: string) {
  if (state.sessions.some((s) => s.id === id)) state.current = id;
  save();
}
export function rename(id: string, name: string) {
  const s = state.sessions.find((x) => x.id === id);
  if (s && name.trim()) s.name = name.trim().slice(0, 60);
  save(true);
}
export function remove(id: string) {
  state.sessions = state.sessions.filter((s) => s.id !== id);
  if (!state.sessions.length) create();
  if (!state.sessions.some((s) => s.id === state.current)) state.current = state.sessions[0].id;
  save(true);
}
