export const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

export const store = {
  get<T>(k: string, d: T): T {
    try {
      const v = localStorage.getItem(k);
      return v === null ? d : (JSON.parse(v) as T);
    } catch {
      return d;
    }
  },
  set(k: string, v: unknown) {
    try {
      localStorage.setItem(k, JSON.stringify(v));
    } catch {
      /* ignore */
    }
  },
};

export function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls = "", text = ""): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text) e.textContent = text;
  return e;
}

export const uid = () => Date.now().toString(36) + Math.random().toString(36).slice(2, 6);

let toastTimer: number | undefined;
export function toast(text: string, kind: "err" | "info" = "err") {
  const t = $("toast");
  t.textContent = text;
  t.className = kind === "info" ? "info" : "";
  t.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => (t.hidden = true), kind === "info" ? 1800 : 7000);
}
