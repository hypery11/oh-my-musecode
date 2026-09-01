#!/usr/bin/env node
/**
 * Oh My Muse Code companion CLI (no runtime dependencies).
 * Real work: setup, doctor, hud snapshot, plus file-based team/ask/wait/mission/wiki/update.
 */
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, statSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const VERSION = "0.1.1";
const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, "..");

const HELP = `oh-my-musecode (omm) ${VERSION}

The missing productivity layer for Meta Muse Code.
Companion CLI — plugin lives next to this file; install it with Muse.

Usage:
  omm setup              Print Muse install / approve commands
  omm doctor             Check muse binary + this plugin tree
  omm hud                Text snapshot of .omm/ (not a live TUI)
  omm team [mission...]  Init .omm/team files; no args lists roster + log
  omm ask [question...]  Pick a bundled skill by keyword overlap
  omm wait [seconds]     Poll .omm/team/log.jsonl mtime (default 5)
  omm mission [text...]  Queue items in .omm/mission/queue.json
  omm wiki list|show|write
                         File wiki under .omm/wiki/
  omm update             Print muse plugins update/approve; optional registry check
  omm -h, --help         Show this help
  omm -V, --version      Print version

Environment:
  MUSE_EXPERIMENTAL_PLUGINS=1   required for Muse 1.0.1 plugin commands
  MUSE_NO_AUTO_UPDATE=1         recommended during validate
  OMM_DIR                       optional .omm/ path override
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

function ensureDir(p) {
  mkdirSync(p, { recursive: true });
  return p;
}

function writeJsonPretty(path, obj) {
  ensureDir(dirname(path));
  writeFileSync(path, JSON.stringify(obj, null, 2) + "\n", "utf8");
}

function readJson(path, fallback) {
  if (!existsSync(path)) return fallback;
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch {
    return fallback;
  }
}

function sleepMs(ms) {
  if (ms <= 0) return;
  const sab = new SharedArrayBuffer(4);
  Atomics.wait(new Int32Array(sab), 0, 0, ms);
}

function lastNonemptyLine(path) {
  const got = readFileCapped(path, 1024 * 1024);
  if (!got || got.skipped || !got.text) return "";
  const lines = got.text.split(/\r?\n/).map((l) => l.trimEnd()).filter((l) => l.trim());
  return lines.length ? lines[lines.length - 1] : "";
}

function cmdTeam(args) {
  const dir = join(resolveOmmDir(), "team");
  ensureDir(dir);
  const missionText = args.join(" ").trim();
  if (missionText) {
    writeFileSync(join(dir, "mission.md"), "# Mission\n\n" + missionText + "\n", "utf8");
    const roster = {
      ts: new Date().toISOString(),
      mission: missionText,
      roles: [
        { role: "architect", skill: "architect" },
        { role: "planner", skill: "planner" },
        { role: "executor", skill: "executor" },
      ],
    };
    writeJsonPretty(join(dir, "roster.json"), roster);
    print("initialized .omm/team/mission.md and roster.json");
    print("mission: " + missionText);
    print("note: not a tmux dashboard — file-based roster only");
    return 0;
  }
  const roster = readJson(join(dir, "roster.json"), null);
  if (roster && typeof roster === "object") {
    print("roster: " + JSON.stringify(roster.roles || roster));
    if (roster.mission) print("mission: " + truncate(roster.mission, 120));
  } else {
    print("roster: (none — run omm team <mission>)");
  }
  const logPath = join(dir, "log.jsonl");
  if (existsSync(logPath)) {
    const got = readFileCapped(logPath, 256 * 1024);
    if (got && got.text) {
      const lines = got.text.split(/\r?\n/).filter((l) => l.trim());
      const tail = lines.slice(-5);
      print("log (" + lines.length + " lines, last " + tail.length + "):");
      for (const line of tail) print(line);
    } else {
      print("log: (empty or skipped)");
    }
  } else {
    print("log: (no .omm/team/log.jsonl)");
  }
  return 0;
}

const SKILL_KEYWORDS = {
  architect: ["architect", "architecture", "system", "module", "layer", "adr", "structure"],
  planner: ["plan", "planner", "roadmap", "steps", "milestone", "schedule", "breakdown", "interview"],
  executor: ["executor", "execute", "implement", "land", "apply", "patch", "code"],
  explore: ["explore", "search", "find", "locate", "codebase", "where", "lookup"],
  analyst: ["analyst", "analyze", "analysis", "data", "metrics", "compare", "trend"],
  designer: ["designer", "design", "ui", "ux", "layout", "visual", "css"],
  debugger: ["debugger", "debug", "bug", "crash", "stacktrace", "exception", "error"],
  tracer: ["tracer", "trace", "flow", "callgraph", "path"],
  critic: ["critic", "critique", "tradeoff", "objection", "review-design"],
  "code-reviewer": ["review", "reviewer", "pr", "diff", "comment", "nit"],
  "security-reviewer": ["security", "vuln", "xss", "injection", "cve", "auth", "secret"],
  "code-simplifier": ["simplify", "simplifier", "refactor", "cleanup", "dead"],
  "test-engineer": ["test", "unit", "coverage", "pytest", "jest", "spec"],
  "qa-tester": ["qa", "regression", "acceptance", "smoke", "e2e"],
  verifier: ["verifier", "verify", "evidence", "acceptance-criteria", "done-definition"],
  scientist: ["scientist", "experiment", "hypothesis", "ablation", "benchmark"],
  "document-specialist": ["docs", "documentation", "readme", "changelog", "api-doc"],
  writer: ["writer", "prose", "copy", "wording", "blog", "narrative"],
  "git-master": ["git", "commit", "branch", "rebase", "merge", "blame", "cherry"],
};

function scoreSkill(question) {
  const q = question.toLowerCase();
  const tokens = new Set(q.split(/[^a-z0-9+.-]+/).filter(Boolean));
  let bestId = "explore";
  let bestScore = 0;
  let bestHits = [];
  for (const [id, kws] of Object.entries(SKILL_KEYWORDS)) {
    const hits = [];
    let score = 0;
    for (const kw of kws) {
      if (q.includes(kw) || tokens.has(kw)) {
        score += kw.includes(" ") || kw.length > 6 ? 2 : 1;
        hits.push(kw);
      }
    }
    if (tokens.has(id) || q.includes(id)) {
      score += 3;
      hits.push(id);
    }
    if (score > bestScore) {
      bestScore = score;
      bestId = id;
      bestHits = hits;
    }
  }
  return { id: bestId, score: bestScore, hits: bestHits };
}

function cmdAsk(args) {
  const question = args.join(" ").trim();
  if (!question) {
    print("usage: omm ask [question...]");
    return 1;
  }
  const picked = scoreSkill(question);
  const reason =
    picked.score > 0
      ? "keyword overlap with " + picked.hits.slice(0, 6).join(", ")
      : "default explore (no keyword overlap)";
  const rec = {
    ts: new Date().toISOString(),
    question,
    skill: picked.id,
    score: picked.score,
    reason,
  };
  writeJsonPretty(join(resolveOmmDir(), "ask", "last.json"), rec);
  print(picked.id);
  print(reason);
  return 0;
}

function cmdWait(args) {
  const raw = args[0];
  let seconds = 5;
  if (raw != null && String(raw).trim() !== "") {
    const n = Number(raw);
    if (!Number.isFinite(n) || n < 0) {
      print("usage: omm wait [seconds]");
      return 1;
    }
    seconds = n;
  }
  const logPath = join(resolveOmmDir(), "team", "log.jsonl");
  const started = Date.now();
  const deadline = started + seconds * 1000;
  let lastMtime = 0;
  if (existsSync(logPath)) {
    try { lastMtime = statSync(logPath).mtimeMs; } catch { lastMtime = 0; }
  }
  let changed = false;
  while (Date.now() < deadline) {
    if (existsSync(logPath)) {
      let m = 0;
      try { m = statSync(logPath).mtimeMs; } catch { m = 0; }
      if (m > lastMtime) {
        changed = true;
        break;
      }
    }
    const remain = deadline - Date.now();
    if (remain <= 0) break;
    sleepMs(Math.min(100, remain));
  }
  const line = lastNonemptyLine(logPath);
  if (changed && line) {
    print(line);
    return 0;
  }
  if (line) {
    print(line);
    if (!changed) print("timeout");
    return 0;
  }
  print("timeout");
  return 1;
}
function loadQueue(path) {
  const data = readJson(path, []);
  return Array.isArray(data) ? data : [];
}

function cmdMission(args) {
  const dir = join(resolveOmmDir(), "mission");
  ensureDir(dir);
  const queuePath = join(dir, "queue.json");
  let queue = loadQueue(queuePath);
  if (args[0] === "done") {
    const id = String(args[1] || "").trim();
    if (!id) {
      print("usage: omm mission done <id>");
      return 1;
    }
    const item = queue.find((x) => x && String(x.id) === id);
    if (!item) {
      print("unknown mission id: " + id);
      return 1;
    }
    item.status = "done";
    writeJsonPretty(queuePath, queue);
    print("done " + id);
    return 0;
  }
  const text = args.join(" ").trim();
  if (!text) {
    if (!queue.length) {
      print("(empty mission queue)");
      return 0;
    }
    for (const item of queue) {
      print(String(item.id) + "\t" + (item.status || "pending") + "\t" + (item.text || ""));
    }
    return 0;
  }
  let maxId = 0;
  for (const item of queue) {
    const n = Number(item && item.id);
    if (Number.isFinite(n) && n > maxId) maxId = n;
  }
  const rec = { id: String(maxId + 1), text, status: "pending" };
  queue.push(rec);
  writeJsonPretty(queuePath, queue);
  print("queued " + rec.id + ": " + text);
  return 0;
}
function wikiFileName(raw) {
  const base = String(raw || "page").replace(/\\/g, "/").split("/").pop();
  let slug = String(base).replace(/[^A-Za-z0-9._-]+/g, "-").replace(/^-+|-+$/g, "");
  if (!slug) slug = "page";
  return slug.endsWith(".md") ? slug : slug + ".md";
}

function cmdWiki(args) {
  const wikiDir = join(resolveOmmDir(), "wiki");
  ensureDir(wikiDir);
  const sub = args[0] || "list";
  if (sub === "list") {
    const files = readdirSync(wikiDir).filter((f) => f.endsWith(".md")).sort();
    if (!files.length) {
      print("(empty wiki)");
      return 0;
    }
    for (const f of files) print(f.replace(/\.md$/, ""));
    return 0;
  }
  if (sub === "show") {
    const page = args[1];
    if (!page) {
      print("usage: omm wiki show <page>");
      return 1;
    }
    const path = join(wikiDir, wikiFileName(page));
    if (!existsSync(path)) {
      print("missing wiki page: " + page);
      return 1;
    }
    print(readFileSync(path, "utf8").replace(/\n$/, ""));
    return 0;
  }
  if (sub === "write") {
    const page = args[1];
    if (!page) {
      print("usage: omm wiki write <page> [body...]");
      return 1;
    }
    const bodyArgs = args.slice(2).join(" ").trim();
    const title = String(page).replace(/\.md$/i, "");
    const body = bodyArgs || ("# " + title + "\n\nstub\n");
    const path = join(wikiDir, wikiFileName(page));
    writeFileSync(path, body.endsWith("\n") ? body : body + "\n", "utf8");
    print("wrote " + path);
    return 0;
  }
  print("usage: omm wiki [list|show <page>|write <page>]");
  return 1;
}
function cmdUpdate() {
  const path = pluginPathHint();
  print("# Oh My Muse Code — refresh hashes / version");
  print("");
  print("export MUSE_EXPERIMENTAL_PLUGINS=1");
  print("muse plugins update oh-my-musecode --json");
  print("muse plugins approve oh-my-musecode --json");
  print("");
  print("plugin tree: " + path);
  print("local version: " + VERSION);
  const npmUrl = "https://registry.npmjs.org/oh-my-musecode/latest";
  const curl = spawnSync("curl", ["-fsS", "--max-time", "5", npmUrl], { encoding: "utf8" });
  if (curl.status === 0 && curl.stdout) {
    try {
      const j = JSON.parse(curl.stdout);
      const remoteVer = j.version || "(unknown)";
      print("registry version: " + remoteVer);
      if (j.version && j.version !== VERSION) print("note: local VERSION differs from registry");
    } catch {
      print("registry: unreadable (non-fatal)");
    }
  } else {
    print("registry: offline (non-fatal)");
  }
  return 0;
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
      return cmdTeam(args.slice(1));
    case "ask":
      return cmdAsk(args.slice(1));
    case "wait":
      return cmdWait(args.slice(1));
    case "mission":
      return cmdMission(args.slice(1));
    case "wiki":
      return cmdWiki(args.slice(1));
    case "update":
      return cmdUpdate();
    default:
      print(`unknown command: ${cmd}`);
      print("try: omm --help");
      return 1;
  }
}

process.exit(main(process.argv));
