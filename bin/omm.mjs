#!/usr/bin/env node
/**
 * Oh My Muse Code companion CLI (no runtime dependencies).
 * Real work: setup, doctor, and hud text snapshot. Other subcommands are honest stubs.
 */
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const VERSION = "0.1.0";
const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, "..");

const HELP = `oh-my-musecode (omm) ${VERSION}

The missing productivity layer for Meta Muse Code.
Companion CLI — plugin lives next to this file; install it with Muse.

Usage:
  omm setup              Print Muse install / approve commands
  omm doctor             Check muse binary + this plugin tree
  omm hud                Text snapshot of .omm/ (not a live TUI)
  omm team|ask|wait|mission|wiki|update
                         Planned stubs (plugin slash-commands are the live path)
  omm -h, --help         Show this help
  omm -V, --version      Print version

Environment:
  MUSE_EXPERIMENTAL_PLUGINS=1   required for Muse 1.0.1 plugin commands
  MUSE_NO_AUTO_UPDATE=1         recommended during validate
  OMM_DIR                       optional .omm/ path override (hud)
`;

function print(s) {
  process.stdout.write(String(s).endsWith("\n") ? s : s + "\n");
}

function pluginPathHint() {
  return ROOT;
}

function cmdSetup() {
  const path = pluginPathHint();
  print(`# Oh My Muse Code — Muse install (experimental plugins)

export MUSE_EXPERIMENTAL_PLUGINS=1
export MUSE_NO_AUTO_UPDATE=1
export MUSE_LOGIN=0

# 1) Validate this tree (does not install):
muse plugins validate "${path}" --json

# 2) Install from a local path (or marketplace once published):
muse plugins install "${path}"
# marketplace form (when listed):
# muse plugins marketplace install oh-my-musecode

# 3) Approve / enable for your Muse home:
muse plugins approve oh-my-musecode

Experimental flag is required on Muse 1.0.1-R2006.1.
This CLI does not install into your Muse home for you.
`);
}

function whichMuse() {
  const fromPath = spawnSync("command", ["-v", "muse"], {
    encoding: "utf8",
    shell: true,
  });
  if (fromPath.status === 0 && fromPath.stdout.trim()) {
    return fromPath.stdout.trim();
  }
  const candidates = [
    join(process.env.HOME || "", ".local/bin/muse"),
    join(process.env.HOME || "", "bin/muse"),
    "/usr/local/bin/muse",
    "/opt/muse/bin/muse",
  ];
  for (const c of candidates) {
    if (c && existsSync(c)) return c;
  }
  return null;
}

function checkTree() {
  const checks = [];
  const manifest = join(ROOT, ".muse-plugin", "plugin.json");
  checks.push({ name: "manifest", ok: existsSync(manifest), detail: manifest });
  let skillCount = 0;
  let commandCount = 0;
  const skillsDir = join(ROOT, "skills");
  if (existsSync(skillsDir)) {
    for (const id of readdirSync(skillsDir)) {
      const p = join(skillsDir, id, "SKILL.md");
      if (existsSync(p) && statSync(p).isFile()) skillCount += 1;
    }
  }
  const commandsDir = join(ROOT, "commands");
  if (existsSync(commandsDir)) {
    commandCount = readdirSync(commandsDir).filter((f) => f.endsWith(".md")).length;
  }
  const hooksDir = join(ROOT, "hooks");
  let hookScripts = 0;
  if (existsSync(hooksDir)) {
    hookScripts = readdirSync(hooksDir).filter((f) => f.endsWith(".py") && !f.startsWith("_")).length;
  }
  checks.push({ name: "skills", ok: skillCount === 19, detail: `${skillCount}/19 SKILL.md files` });
  checks.push({ name: "commands", ok: commandCount === 19, detail: `${commandCount}/19 command markdown files` });
  checks.push({ name: "hooks", ok: hookScripts === 8, detail: `${hookScripts}/8 hook scripts` });
  return { checks, manifest };
}

function cmdDoctor() {
  print(`omm doctor ${VERSION}`);
  print(`plugin root: ${ROOT}`);
  const muse = whichMuse();
  print(`muse binary: ${muse || "(not found on PATH or common locations)"}`);
  const { checks, manifest } = checkTree();
  for (const c of checks) {
    print(`${c.ok ? "ok" : "FAIL"}  ${c.name}: ${c.detail}`);
  }
  if (existsSync(manifest)) {
    try {
      const j = JSON.parse(readFileSync(manifest, "utf8"));
      print(`manifest name=${j.name} version=${j.version} schemaVersion=${j.schemaVersion}`);
    } catch (e) {
      print(`FAIL  manifest JSON: ${e.message}`);
    }
  }
  if (muse) {
    print("running: muse plugins validate … --json");
    const result = spawnSync(
      muse,
      ["plugins", "validate", ROOT, "--json"],
      {
        encoding: "utf8",
        env: {
          ...process.env,
          MUSE_EXPERIMENTAL_PLUGINS: "1",
          MUSE_NO_AUTO_UPDATE: "1",
          MUSE_LOGIN: "0",
        },
      },
    );
    if (result.stdout) print(result.stdout.trimEnd());
    if (result.stderr) print(result.stderr.trimEnd());
    print(`validate exit: ${result.status}`);
  } else {
    print("skip validate: no muse binary found");
  }
}

const SECRET_KEY_RE = /token|secret|password|api[_-]?key|authorization|credential|passwd|bearer/i;
const MAX_JSON_BYTES = 64 * 1024;

function resolveOmmDir() {
  const override = (process.env.OMM_DIR || "").trim();
  if (override) return resolve(override);
  return join(process.cwd(), ".omm");
}

function truncate(s, n = 96) {
  const t = String(s).replace(/\s+/g, " ").trim();
  if (t.length <= n) return t;
  return t.slice(0, Math.max(0, n - 1)) + "\u2026";
}

function readFileCapped(path, maxBytes = MAX_JSON_BYTES) {
  try {
    const st = statSync(path);
    if (!st.isFile()) return null;
    if (st.size > maxBytes) return { skipped: true, size: st.size };
    return { text: readFileSync(path, "utf8"), size: st.size };
  } catch {
    return null;
  }
}

function readJsonCapped(path) {
  const got = readFileCapped(path);
  if (!got) return null;
  if (got.skipped) return { skipped: true };
  try {
    return { value: JSON.parse(got.text) };
  } catch {
    return null;
  }
}

function countNonemptyLines(path) {
  const got = readFileCapped(path, 1024 * 1024);
  if (!got) return null;
  if (got.skipped) return "large";
  let n = 0;
  for (const line of got.text.split(/\r?\n/)) {
    if (line.trim()) n += 1;
  }
  return n;
}

function summarizePlan(plan) {
  if (Array.isArray(plan)) return plan.length + " steps";
  if (!plan || typeof plan !== "object") return truncate(plan, 80);
  const steps = plan.steps || plan.items || plan.todos;
  const n = Array.isArray(steps) ? steps.length : null;
  const title = plan.title || plan.name || plan.summary || plan.goal || "";
  const bits = [];
  if (title) bits.push(truncate(title, 72));
  if (n != null) bits.push(n + " steps");
  if (plan.status) bits.push(String(plan.status));
  return bits.join(", ") || "present";
}

function summarizeVerify(v) {
  if (Array.isArray(v)) return v.length + " results";
  if (!v || typeof v !== "object") return truncate(v, 80);
  const bits = [];
  if (v.status != null) bits.push(truncate(v.status, 40));
  if (typeof v.ok === "boolean") bits.push(v.ok ? "ok" : "not-ok");
  if (typeof v.passed === "boolean") bits.push(v.passed ? "passed" : "failed");
  if (v.summary) bits.push(truncate(v.summary, 72));
  return bits.join(", ") || "present";
}

function summarizeTeam(teamDir) {
  let files = [];
  try {
    files = readdirSync(teamDir).filter((name) => {
      try { return statSync(join(teamDir, name)).isFile(); } catch { return false; }
    });
  } catch { files = []; }
  let mission = "";
  const md = join(teamDir, "mission.md");
  const mj = join(teamDir, "mission.json");
  if (existsSync(md)) {
    const got = readFileCapped(md, 8192);
    if (got && got.text) {
      const line = got.text.split(/\r?\n/).find((l) => l.trim()) || "";
      mission = truncate(line.replace(/^#+\s*/, ""), 72);
    }
  } else if (existsSync(mj)) {
    const got = readJsonCapped(mj);
    if (got && got.value && typeof got.value === "object") {
      const name = got.value.name || got.value.mission || got.value.title;
      if (name) mission = truncate(name, 72);
    }
  }
  const bits = [files.length + " files"];
  if (mission) bits.push("mission: " + mission);
  return bits.join(", ");
}

function summarizeMemory(dir) {
  const bits = [];
  const pairs = [["memory.md", join(dir, "memory.md")], ["memory.jsonl", join(dir, "memory.jsonl")], ["notes.md", join(dir, "notes.md")]];
  for (const [label, p] of pairs) {
    if (!existsSync(p)) continue;
    const n = countNonemptyLines(p);
    if (n === "large") bits.push(label + " large");
    else if (n != null) bits.push(label + " " + n + " lines");
  }
  if (!bits.length) return "";
  return "memory: " + bits.join(", ");
}

function safeStr(key, val, n = 80) {
  if (SECRET_KEY_RE.test(String(key))) return "";
  if (val == null) return "";
  if (typeof val === "object") return "";
  return truncate(val, n);
}

function cmdHud() {
  const dir = resolveOmmDir();
  if (!existsSync(dir) || !statSync(dir).isDirectory()) {
    print(".omm/ is absent — omm hud is a text snapshot, not a live bar.");
    return 0;
  }
  const lines = [];
  lines.push("omm hud — text snapshot of .omm/ (not a live TUI)");
  lines.push("path: " + dir);
  const modeGot = readJsonCapped(join(dir, "mode.json"));
  if (modeGot && modeGot.skipped) lines.push("mode: (file too large, skipped)");
  else if (modeGot && modeGot.value && typeof modeGot.value === "object") {
    const mode = safeStr("mode", modeGot.value.mode || modeGot.value.name) || "(unknown)";
    lines.push("mode: " + mode);
  }
  const ralphGot = readJsonCapped(join(dir, "ralph.json"));
  if (ralphGot && ralphGot.skipped) lines.push("ralph: (file too large, skipped)");
  else if (ralphGot && ralphGot.value && typeof ralphGot.value === "object") {
    const r = ralphGot.value;
    const active = r.active === true || r.active === "true";
    const it = r.iterations ?? r.iteration ?? "?";
    const mx = r.max ?? r.maxIterations ?? "?";
    const goal = safeStr("goal", r.goal, 80);
    let row = "ralph: active=" + active + " iterations=" + it + "/" + mx;
    if (goal) row += " goal: " + goal;
    lines.push(row);
  }
  const planGot = readJsonCapped(join(dir, "plan.json"));
  if (planGot && planGot.skipped) lines.push("plan: (file too large, skipped)");
  else if (planGot && planGot.value != null) lines.push("plan: " + summarizePlan(planGot.value));
  const verifyGot = readJsonCapped(join(dir, "verify.json"));
  if (verifyGot && verifyGot.skipped) lines.push("verify: (file too large, skipped)");
  else if (verifyGot && verifyGot.value != null) lines.push("verify: " + summarizeVerify(verifyGot.value));
  const teamDir = join(dir, "team");
  if (existsSync(teamDir) && statSync(teamDir).isDirectory()) {
    lines.push("team: " + summarizeTeam(teamDir));
  }
  const memLine = summarizeMemory(dir);
  if (memLine) lines.push(memLine);
  const hooksPath = join(dir, "hooks.jsonl");
  if (existsSync(hooksPath)) {
    const n = countNonemptyLines(hooksPath);
    if (n === "large") lines.push("hooks.jsonl: large (not counted)");
    else if (n != null) lines.push("hooks.jsonl: " + n + " lines");
  }
  print(lines.join("\n"));
  return 0;
}

function stub(name) {
  print(`planned: omm ${name} is a CLI stub — use the matching Muse slash-command in-plugin.`);
}

function main(argv) {
  const args = argv.slice(2);
  const cmd = args[0];
  if (!cmd || cmd === "-h" || cmd === "--help" || cmd === "help") {
    print(HELP);
    return 0;
  }
  if (cmd === "-V" || cmd === "--version" || cmd === "version") {
    print(VERSION);
    return 0;
  }
  switch (cmd) {
    case "setup":
      cmdSetup();
      return 0;
    case "doctor":
      cmdDoctor();
      return 0;
    case "hud":
      cmdHud();
      return 0;
    case "team":
    case "ask":
    case "wait":
    case "mission":
    case "wiki":
    case "update":
      stub(cmd);
      return 0;
    default:
      print(`unknown command: ${cmd}`);
      print("try: omm --help");
      return 1;
  }
}

process.exit(main(process.argv));
