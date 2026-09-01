import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

export const SECRET_KEY_RE = /token|secret|password|api[_-]?key|authorization|credential|passwd|bearer/i;
export const REMEMBER_SECRET_RE = /token|secret|password|api[_-]?key/i;
export const MAX_JSON_BYTES = 64 * 1024;

export function resolveOmmDir() {
  const override = (process.env.OMM_DIR || "").trim();
  if (override) return resolve(override);
  return join(process.cwd(), ".omm");
}

export function ensureDir(p) {
  mkdirSync(p, { recursive: true });
  return p;
}

export function writeJsonPretty(path, obj) {
  ensureDir(dirname(path));
  writeFileSync(path, JSON.stringify(obj, null, 2) + "\n", "utf8");
}

export function readJson(path, fallback) {
  if (!existsSync(path)) return fallback;
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch {
    return fallback;
  }
}

export function truncate(s, n = 96) {
  const t = String(s).replace(/\s+/g, " ").trim();
  if (t.length <= n) return t;
  return t.slice(0, Math.max(0, n - 1)) + "\u2026";
}

export function readFileCapped(path, maxBytes = MAX_JSON_BYTES) {
  try {
    const st = statSync(path);
    if (!st.isFile()) return null;
    if (st.size > maxBytes) return { skipped: true, size: st.size };
    return { text: readFileSync(path, "utf8"), size: st.size };
  } catch {
    return null;
  }
}

export function readJsonCapped(path) {
  const got = readFileCapped(path);
  if (!got) return null;
  if (got.skipped) return { skipped: true };
  try {
    return { value: JSON.parse(got.text) };
  } catch {
    return null;
  }
}

export function countNonemptyLines(path) {
  const got = readFileCapped(path, 1024 * 1024);
  if (!got) return null;
  if (got.skipped) return "large";
  let n = 0;
  for (const line of got.text.split(/\r?\n/)) {
    if (line.trim()) n += 1;
  }
  return n;
}

export function lastNonemptyLine(path) {
  const got = readFileCapped(path, 1024 * 1024);
  if (!got || got.skipped || !got.text) return "";
  const lines = got.text.split(/\r?\n/).map((l) => l.trimEnd()).filter((l) => l.trim());
  return lines.length ? lines[lines.length - 1] : "";
}

export function sleepMs(ms) {
  if (ms <= 0) return;
  const sab = new SharedArrayBuffer(4);
  Atomics.wait(new Int32Array(sab), 0, 0, ms);
}

export function utcStamp() {
  return new Date().toISOString().replace(/[-:]/g, "").replace(".", "");
}

export function slugify(raw, fallback = "item") {
  let slug = String(raw || "")
    .toLowerCase()
    .replace(/\\/g, "/")
    .split("/")
    .pop();
  slug = String(slug).replace(/[^a-z0-9-]+/g, "-").replace(/^-+|-+$/g, "");
  return slug || fallback;
}

export function isPortableSlug(slug) {
  return /^[a-z0-9-]+$/.test(String(slug || ""));
}

export function looksSecretText(text) {
  return REMEMBER_SECRET_RE.test(String(text || ""));
}

export function stripSecretKeys(obj) {
  if (Array.isArray(obj)) return obj.map(stripSecretKeys);
  if (!obj || typeof obj !== "object") return obj;
  const out = {};
  for (const [k, v] of Object.entries(obj)) {
    if (SECRET_KEY_RE.test(k)) continue;
    if (typeof v === "string" && looksSecretText(v)) continue;
    out[k] = typeof v === "object" ? stripSecretKeys(v) : v;
  }
  return out;
}

export function safeStr(key, val, n = 80) {
  if (SECRET_KEY_RE.test(String(key))) return "";
  if (val == null) return "";
  if (typeof val === "object") return "";
  if (looksSecretText(String(val))) return "";
  return truncate(val, n);
}
