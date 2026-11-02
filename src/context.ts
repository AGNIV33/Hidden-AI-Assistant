import type { CtxFile } from "./sessions";

const STOP = new Set(
  "the and for you your are was were with that this have has had not but from they their will would can could what when where which who how why about into than then them she him her his its our out all any one use used using also just like get got let may might shall should does did done been being more most some such only very over under again there here".split(" "),
);
const tokens = (s: string): string[] => (s.toLowerCase().match(/[\p{L}\p{N}]{3,}/gu) ?? []).filter((w) => !STOP.has(w));

/** Split text into pieces of about `size` characters, at paragraph or sentence ends. */
function chunk(text: string, size = 700): string[] {
  const out: string[] = [];
  let cur = "";
  const push = () => { if (cur.trim()) out.push(cur.trim()); cur = ""; };
  for (const para of text.split(/\n{2,}/)) {
    const parts = para.length > size * 1.5 ? (para.match(/[^.!?\n]+[.!?\n]*/g) ?? [para]) : [para];
    for (const part of parts) {
      if (cur.length + part.length > size) push();
      cur += (cur ? "\n" : "") + part;
      if (cur.length > size * 2) push();
    }
    if (cur.length > size / 2) push();
  }
  push();
  return out;
}

/**
 * Text from the session's files to send with a question.
 * Small files go in whole. Big ones: only the pieces that best match the question.
 */
export function buildContext(files: CtxFile[], query: string, budget: number): string {
  if (!files.length) return "";
  const total = files.reduce((n, f) => n + f.text.length + f.name.length + 10, 0);
  if (total <= budget) return files.map((f) => `### ${f.name}\n${f.text}`).join("\n\n");

  interface Piece { file: number; idx: number; text: string; score: number }
  const pieces: Piece[] = [];
  files.forEach((f, file) => chunk(f.text).forEach((text, idx) => pieces.push({ file, idx, text, score: 0 })));

  const words = [...new Set(tokens(query))];
  if (words.length) {
    const sets = pieces.map((p) => new Set(tokens(p.text)));
    const df = new Map(words.map((w) => [w, sets.filter((s) => s.has(w)).length] as const));
    pieces.forEach((p, i) => {
      for (const w of words) if (sets[i].has(w)) p.score += Math.log(1 + pieces.length / (1 + (df.get(w) ?? 0)));
    });
  }
  // Best match first. No match at all: take the start of every file.
  const ranked = [...pieces].sort((a, b) => b.score - a.score || a.idx - b.idx || a.file - b.file);
  const chosen: Piece[] = [];
  let used = 0;
  for (const p of ranked) {
    if (used + p.text.length > budget) continue;
    chosen.push(p);
    used += p.text.length + 4;
  }
  chosen.sort((a, b) => a.file - b.file || a.idx - b.idx);

  let out = "";
  let lastFile = -1;
  let lastIdx = -2;
  for (const p of chosen) {
    if (p.file !== lastFile) { out += `${out ? "\n\n" : ""}### ${files[p.file].name}\n`; lastIdx = -2; }
    else if (p.idx !== lastIdx + 1) out += "\n[…]\n";
    out += p.text + "\n";
    lastFile = p.file;
    lastIdx = p.idx;
  }
  return out.trim();
}
