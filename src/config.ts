import { store, uid } from "./util";

export type Role = "answer" | "vision" | "stt";
/** One AI connection. The API key is NOT stored here (it lives in Windows Credential Manager). */
export interface Slot { id: string; preset: string; baseUrl: string; model: string }
export type Config = Record<Role, Slot[]>;

export const ROLES: { id: Role; title: string; hint: string }[] = [
  { id: "answer", title: "Answers", hint: "Writes the replies you read. 1st = main. Others = automatic backup if the main one fails." },
  { id: "vision", title: "Vision (screenshots)", hint: "Reads screenshots and pasted images. Needs a model that can see images. If you add none, your Answers models are tried." },
  { id: "stt", title: "Speech-to-text (listening)", hint: "Turns the call audio into text for listening mode. Needs an OpenAI-style transcription service: Groq (whisper-large-v3-turbo) or OpenAI (whisper-1) work. Other providers may not." },
];

// Any OpenAI-compatible service works. Pick "Custom" for anything not listed.
export const PRESETS = [
  { id: "groq", name: "Groq", url: "https://api.groq.com/openai/v1" },
  { id: "gemini", name: "Google Gemini", url: "https://generativelanguage.googleapis.com/v1beta/openai" },
  { id: "openai", name: "OpenAI", url: "https://api.openai.com/v1" },
  { id: "anthropic", name: "Anthropic (Claude)", url: "https://api.anthropic.com/v1" },
  { id: "nvidia", name: "NVIDIA (build.nvidia.com)", url: "https://integrate.api.nvidia.com/v1" },
  { id: "openrouter", name: "OpenRouter", url: "https://openrouter.ai/api/v1" },
  { id: "mistral", name: "Mistral", url: "https://api.mistral.ai/v1" },
  { id: "deepseek", name: "DeepSeek", url: "https://api.deepseek.com/v1" },
  { id: "together", name: "Together AI", url: "https://api.together.xyz/v1" },
  { id: "ollama", name: "Ollama (local)", url: "http://localhost:11434/v1" },
  { id: "localwhisper", name: "Local Whisper (this PC)", url: "http://127.0.0.1:8080/v1" },
  { id: "custom", name: "Custom (any OpenAI-compatible)", url: "" },
];

export const DEFAULT_MODEL: Record<string, Partial<Record<Role, string>>> = {
  groq: { answer: "llama-3.3-70b-versatile", stt: "whisper-large-v3-turbo" },
  gemini: { answer: "gemini-3.1-flash-lite", vision: "gemini-3.1-flash-lite" },
  nvidia: { answer: "meta/llama-3.3-70b-instruct", vision: "meta/llama-3.2-11b-vision-instruct" },
  localwhisper: { stt: "whisper" },
};

export const MODES = [
  { id: "point", label: "Point" },
  { id: "brief", label: "Brief" },
  { id: "detailed", label: "Detail" },
  { id: "long", label: "Long" },
];

function loadConfig(): Config {
  const c = store.get<Partial<Config>>("config", {});
  const cfg: Config = { answer: c.answer ?? [], vision: c.vision ?? [], stt: c.stt ?? [] };
  // Google stopped serving gemini-2.5-flash to new keys (404). Move old saved slots to a working default.
  // Groq retired its Llama 4 Scout vision model.
  for (const s of cfg.vision) if (s.model === "meta-llama/llama-4-scout-17b-16e-instruct") s.model = "";
  for (const role of ["answer", "vision"] as const) {
    for (const s of cfg[role]) {
      // Gemini only serves gemini-* / gemma-* models. Fix old names left over from another provider.
      if (s.preset === "gemini" && (s.model === "gemini-2.5-flash" || !/^(models\/)?(gemini|gemma)/i.test(s.model))) {
        s.model = DEFAULT_MODEL.gemini[role] ?? "gemini-3.1-flash-lite";
      }
    }
  }
  return cfg;
}

export const config: Config = loadConfig();
export const saveConfig = () => store.set("config", config);
export const allSlots = () => [...config.answer, ...config.vision, ...config.stt];
export const isLocalUrl = (u: string) => /^http:\/\/(localhost|127\.0\.0\.1)(:|\/|$)/i.test(u.trim());

export function newSlot(role: Role, preset = "groq"): Slot {
  const p = PRESETS.find((x) => x.id === preset)!;
  return { id: `${role}-${uid()}`, preset, baseUrl: p.url, model: DEFAULT_MODEL[preset]?.[role] ?? "" };
}
