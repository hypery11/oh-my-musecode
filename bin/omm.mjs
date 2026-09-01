#!/usr/bin/env node
/**
 * Oh My Muse Code companion CLI (no runtime dependencies).
 * File-based engines for .omm/ state; slash-commands still run in-session.
 */
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  countNonemptyLines,
  ensureDir,
  lastNonemptyLine,
  readFileCapped,
  readJson,
  readJsonCapped,
  resolveOmmDir,
  safeStr,
  sleepMs,
  writeJsonPretty,
} from "./lib/fs.mjs";
import { loadSkillCatalog, pickSkill } from "./lib/skills.mjs";
import {
  cmdAutopilot,
  cmdDebug,
  cmdExecute,
  cmdHandoff,
  cmdInterview,
  cmdRalplan,
  cmdRemember,
  cmdSkillify,
  cmdTrace,
  cmdUltragoal,
  cmdVerify,
  extraHudLines,
  summarizeMemory,
  summarizePlan,
  summarizeTeam,
  summarizeVerify,
} from "./lib/engines.mjs";

const VERSION = "0.3.0";
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
  omm ask [question...]  Route to a bundled skill (SKILL.md description + id)
  omm wait [seconds]     Poll .omm/team/log.jsonl mtime (default 5)
  omm mission [text...]  Queue items in .omm/mission/queue.json
  omm wiki list|show|write
                         File wiki under .omm/wiki/
  omm update             Print muse plugins update/approve; optional registry check
  omm ralplan [topic...] Ralph-oriented plan stub + inactive ralph.json
  omm interview [subject...]
  omm deep-interview [subject...]
                         Write .omm/interview/<utc-stamp>.md + requirements.md
  omm ultragoal [goal...] mode.json + ultragoal.md + milestone-1 plan.json
  omm handoff [focus...] Summarize mode/plan/verify/team into handoff.md
  omm skillify [workflow-name...] [--apply]
                         Draft .omm/skillify/<slug>.md; --apply writes skills/ if portable
  omm verify [claim...]  Write pending verify.json; pass|fail [note]; no args prints
  omm autopilot [goal...] mode.json + autopilot.json (Stop chain after todo)
  omm execute [step-or-task...]
                         Next pending plan.json step in-progress; done marks it done
  omm remember [note...] Append memory.md + memory.jsonl (refuses secrets)
  omm debug [symptom...] .omm/debug/<stamp>.md + mode.json debug
  omm trace [target...]  Static outline of this plugin's hooks/commands
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
  for (const extra of extraHudLines(dir)) lines.push(extra);
  print(lines.join("\n"));
  return 0;
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
    if (roster.mission) print("mission: " + (roster.mission.length > 120 ? roster.mission.slice(0, 119) + "\u2026" : roster.mission));
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

function cmdAsk(args) {
  const question = args.join(" ").trim();
  if (!question) {
    print("usage: omm ask [question...]");
    return 1;
  }
  const catalog = loadSkillCatalog(join(ROOT, "skills"));
  const picked = pickSkill(question, catalog);
  const rec = {
    query: question,
    skill: picked.id,
    score: picked.score,
    reason: picked.reason,
    alternatives: picked.alternatives,
  };
  writeJsonPretty(join(resolveOmmDir(), "ask", "last.json"), rec);
  print(picked.id);
  print(picked.reason);
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
      return cmdHud();
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
    case "ralplan":
      return cmdRalplan(args.slice(1));
    case "interview":
    case "deep-interview":
      return cmdInterview(args.slice(1));
    case "ultragoal":
      return cmdUltragoal(args.slice(1));
    case "handoff":
      return cmdHandoff(args.slice(1));
    case "skillify":
      return cmdSkillify(args.slice(1), ROOT);
    case "verify":
      return cmdVerify(args.slice(1));
    case "autopilot":
      return cmdAutopilot(args.slice(1));
    case "execute":
      return cmdExecute(args.slice(1));
    case "remember":
      return cmdRemember(args.slice(1));
    case "debug":
      return cmdDebug(args.slice(1));
    case "trace":
      return cmdTrace(args.slice(1));
    default:
      print(`unknown command: ${cmd}`);
      print("try: omm --help");
      return 1;
  }
}

process.exit(main(process.argv));
