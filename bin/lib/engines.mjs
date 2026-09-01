import { appendFileSync, existsSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import {
  countNonemptyLines,
  ensureDir,
  isPortableSlug,
  looksSecretText,
  readFileCapped,
  readJson,
  readJsonCapped,
  resolveOmmDir,
  safeStr,
  slugify,
  stripSecretKeys,
  truncate,
  utcStamp,
  writeJsonPretty,
} from "./fs.mjs";

function print(s) {
  process.stdout.write(String(s).endsWith("\n") ? s : s + "\n");
}

function modePath(dir) {
  return join(dir, "mode.json");
}

function writeMode(dir, extra) {
  writeJsonPretty(modePath(dir), extra);
}

function listDirFiles(dir) {
  try {
    return readdirSync(dir).filter((name) => {
      try { return statSync(join(dir, name)).isFile(); } catch { return false; }
    });
  } catch {
    return [];
  }
}

export function cmdRalplan(args) {
  const dir = resolveOmmDir();
  ensureDir(dir);
  const topic = args.join(" ").trim();
  writeMode(dir, { mode: "ralplan", topic });
  const planMd = join(dir, "plan.md");
  if (!existsSync(planMd)) {
    const title = topic || "untitled";
    writeFileSync(
      planMd,
      "# Plan\n\nTopic: " + title + "\n\n## Steps\n\n- [ ] Clarify goal\n- [ ] Name verify commands\n",
      "utf8",
    );
  }
  writeJsonPretty(join(dir, "ralph.json"), {
    active: false,
    goal: topic,
    iterations: 0,
    max: 20,
  });
  print("ralplan: mode.json + plan.md + ralph.json (active=false)");
  if (topic) print("topic: " + topic);
  return 0;
}


function uniqueStampPath(dir, ext = ".md") {
  const stamp = utcStamp();
  let path = join(dir, stamp + ext);
  let n = 0;
  while (existsSync(path)) {
    n += 1;
    path = join(dir, stamp + "-" + n + ext);
  }
  return { stamp: stamp + (n ? "-" + n : ""), path };
}

function writeInterview(args) {
  const dir = resolveOmmDir();
  const interviewDir = ensureDir(join(dir, "interview"));
  const subject = args.join(" ").trim();
  const { stamp, path: notePath } = uniqueStampPath(interviewDir);
  const body = [
    "# Interview",
    "",
    "Subject: " + (subject || "(unspecified)"),
    "Stamp: " + stamp,
    "",
    "## Questions",
    "",
    "- Goals:",
    "- Constraints:",
    "- Non-goals:",
    "- Success metrics:",
    "",
    "## Notes",
    "",
    "(CLI stub — in-session Muse fills the interview prose.)",
    "",
  ].join("\n");
  writeFileSync(notePath, body, "utf8");
  const reqPath = join(dir, "requirements.md");
  const req = [
    "# Requirements",
    "",
    "Subject: " + (subject || "(unspecified)"),
    "Source: " + notePath,
    "",
    "(Condensed brief — in-session Muse fills this after the interview.)",
    "",
  ].join("\n");
  writeFileSync(reqPath, req, "utf8");
  print(notePath);
  print(reqPath);
  return 0;
}

export function cmdInterview(args) {
  return writeInterview(args);
}

export function cmdUltragoal(args) {
  const dir = resolveOmmDir();
  ensureDir(dir);
  const goal = args.join(" ").trim();
  writeMode(dir, { mode: "ultragoal", goal });
  const md = join(dir, "ultragoal.md");
  writeFileSync(
    md,
    [
      "# Ultragoal",
      "",
      "Goal: " + (goal || "(unspecified)"),
      "",
      "## Vision",
      "",
      "(CLI stub — in-session Muse fills vision, milestones, metrics, anti-goals.)",
      "",
      "## Milestones",
      "",
      "1. milestone-1 (current)",
      "",
    ].join("\n"),
    "utf8",
  );
  writeJsonPretty(join(dir, "plan.json"), {
    title: goal || "ultragoal",
    milestone: "milestone-1",
    steps: [
      { id: "1", title: "Scope milestone-1", status: "pending" },
      { id: "2", title: "Name deliverables", status: "pending" },
      { id: "3", title: "Name verification", status: "pending" },
    ],
  });
  print("ultragoal: mode.json + ultragoal.md + plan.json (milestone-1)");
  if (goal) print("goal: " + goal);
  return 0;
}

function summarizeJsonFile(path, label) {
  const got = readJsonCapped(path);
  if (!got) return "";
  if (got.skipped) return label + ": (file too large, skipped)";
  if (got.value == null) return "";
  const cleaned = stripSecretKeys(got.value);
  return label + ": " + truncate(JSON.stringify(cleaned), 240);
}

export function cmdHandoff(args) {
  const dir = resolveOmmDir();
  ensureDir(dir);
  const focus = args.join(" ").trim();
  const lines = ["# Handoff", ""];
  if (focus) {
    lines.push("Focus: " + focus);
    lines.push("");
  }
  lines.push("Generated: " + new Date().toISOString());
  lines.push("");
  const modeLine = summarizeJsonFile(join(dir, "mode.json"), "mode");
  if (modeLine) lines.push(modeLine);
  const planLine = summarizeJsonFile(join(dir, "plan.json"), "plan");
  if (planLine) lines.push(planLine);
  const planMd = join(dir, "plan.md");
  if (existsSync(planMd)) {
    const n = countNonemptyLines(planMd);
    lines.push("plan.md: " + (n === "large" ? "large" : n + " lines"));
  }
  const verifyLine = summarizeJsonFile(join(dir, "verify.json"), "verify");
  if (verifyLine) lines.push(verifyLine);
  const ralphLine = summarizeJsonFile(join(dir, "ralph.json"), "ralph");
  if (ralphLine) lines.push(ralphLine);
  const autoLine = summarizeJsonFile(join(dir, "autopilot.json"), "autopilot");
  if (autoLine) lines.push(autoLine);
  const teamDir = join(dir, "team");
  if (existsSync(teamDir) && statSync(teamDir).isDirectory()) {
    const mission = join(teamDir, "mission.md");
    if (existsSync(mission)) {
      const got = readFileCapped(mission, 8192);
      if (got && got.text) {
        const line = got.text.split(/\r?\n/).find((l) => l.trim()) || "";
        lines.push("team mission: " + truncate(line.replace(/^#+\s*/, ""), 120));
      }
    }
    const roster = summarizeJsonFile(join(teamDir, "roster.json"), "team roster");
    if (roster) lines.push(roster);
    const logPath = join(teamDir, "log.jsonl");
    if (existsSync(logPath)) {
      const n = countNonemptyLines(logPath);
      lines.push("team log: " + (n === "large" ? "large" : n + " lines"));
    }
  }
  lines.push("");
  lines.push("No secrets included. Next session: read this file, then `/hud` or `omm hud`.");
  lines.push("");
  const path = join(dir, "handoff.md");
  writeFileSync(path, lines.join("\n"), "utf8");
  print(path);
  return 0;
}

function skillSkeleton(slug, name) {
  return [
    "---",
    "name: " + slug,
    "description: " + (name || slug) + " workflow for Muse sessions.",
    "---",
    "",
    "# " + (name || slug),
    "",
    "You are the **" + slug + "** role. Follow this workflow in Muse.",
    "",
    "## When to activate",
    "- Repeated workflow: " + (name || slug),
    "",
    "## How to work",
    "1. Restate the goal.",
    "2. Persist notes under `.omm/`.",
    "3. Use Muse-native tools only.",
    "",
    "## State",
    "- Notes: `.omm/skillify/" + slug + ".md`",
    "",
    "## Done when",
    "The workflow is documented and the next command is named.",
    "",
  ].join("\n");
}

export function cmdSkillify(args, root) {
  const apply = args.includes("--apply");
  const nameParts = args.filter((a) => a !== "--apply");
  const name = nameParts.join(" ").trim() || "workflow";
  const slug = slugify(name, "workflow");
  const dir = resolveOmmDir();
  const outDir = ensureDir(join(dir, "skillify"));
  const draftPath = join(outDir, slug + ".md");
  const skeleton = skillSkeleton(slug, name);
  writeFileSync(
    draftPath,
    "# Skillify: " + name + "\n\nSlug: " + slug + "\n\n" + skeleton,
    "utf8",
  );
  print("draft: " + draftPath);
  print("suggested SKILL.md:\n" + skeleton);
  if (!apply) {
    print("note: not writing plugin skills/ (pass --apply with a portable slug)");
    return 0;
  }
  if (!isPortableSlug(slug)) {
    print("error: slug is not portable [a-z0-9-]+ : " + slug);
    return 1;
  }
  const dest = join(root, "skills", slug, "SKILL.md");
  ensureDir(dirname(dest));
  writeFileSync(dest, skeleton, "utf8");
  print("wrote " + dest);
  print("remind: add capabilities.skills in plugin.json, then muse plugins validate");
  return 0;
}

export function cmdVerify(args) {
  const dir = resolveOmmDir();
  ensureDir(dir);
  const path = join(dir, "verify.json");
  const mdPath = join(dir, "verify.md");
  const sub = args[0];
  if (!args.length) {
    if (!existsSync(path)) {
      print("(no verify.json)");
      return 1;
    }
    print(readFileSync(path, "utf8").replace(/\n$/, ""));
    return 0;
  }
  if (sub === "pass" || sub === "fail") {
    const note = args.slice(1).join(" ").trim();
    const cur = readJson(path, { claim: "", status: "pending", ok: false, checks: [] });
    if (!cur || typeof cur !== "object") {
      print("error: verify.json is unreadable");
      return 1;
    }
    const ok = sub === "pass";
    cur.status = ok ? "pass" : "fail";
    cur.ok = ok;
    if (note) cur.evidence = note;
    if (!Array.isArray(cur.checks)) cur.checks = [];
    cur.checks.push({ status: cur.status, ok, note: note || "", ts: new Date().toISOString() });
    writeJsonPretty(path, cur);
    writeFileSync(
      mdPath,
      "# Verify\n\nClaim: " + (cur.claim || "") + "\nStatus: " + cur.status + "\nok: " + ok + "\n" + (note ? "\n" + note + "\n" : ""),
      "utf8",
    );
    print(cur.status);
    return 0;
  }
  const claim = args.join(" ").trim();
  const rec = { claim, status: "pending", ok: false, checks: [] };
  writeJsonPretty(path, rec);
  writeFileSync(mdPath, "# Verify\n\nClaim: " + claim + "\nStatus: pending\nok: false\n", "utf8");
  print("pending: " + claim);
  return 0;
}

function defaultPlan(title) {
  return {
    title: title || "plan",
    steps: [
      { id: "1", title: title || "first step", status: "pending" },
    ],
  };
}

export function cmdAutopilot(args) {
  const dir = resolveOmmDir();
  ensureDir(dir);
  const goal = args.join(" ").trim();
  writeMode(dir, { mode: "autopilot", goal });
  const planPath = join(dir, "plan.json");
  if (!existsSync(planPath)) {
    writeJsonPretty(planPath, defaultPlan(goal || "autopilot"));
  }
  writeJsonPretty(join(dir, "autopilot.json"), {
    active: true,
    step: 0,
    max: 20,
    goal,
  });
  print("autopilot: mode.json + autopilot.json (active=true, step=0, max=20)");
  if (goal) print("goal: " + goal);
  return 0;
}

function loadPlan(dir) {
  const path = join(dir, "plan.json");
  const plan = readJson(path, null);
  if (Array.isArray(plan)) {
    return { path, root: { steps: plan }, stepsKey: "steps", asArray: true };
  }
  if (plan && typeof plan === "object") {
    if (Array.isArray(plan.steps)) return { path, root: plan, stepsKey: "steps", asArray: false };
    if (Array.isArray(plan.items)) return { path, root: plan, stepsKey: "items", asArray: false };
    if (Array.isArray(plan.todos)) return { path, root: plan, stepsKey: "todos", asArray: false };
    return { path, root: plan, stepsKey: "steps", asArray: false };
  }
  return { path, root: { steps: [] }, stepsKey: "steps", asArray: false };
}

function normalizeStep(s, i) {
  if (typeof s === "string") return { id: String(i + 1), title: s, status: "pending" };
  if (s && typeof s === "object") {
    return {
      ...s,
      id: String(s.id || i + 1),
      title: s.title || s.name || s.text || "step",
      status: s.status || "pending",
    };
  }
  return { id: String(i + 1), title: "step", status: "pending" };
}

function savePlan(bundle) {
  const steps = bundle.root[bundle.stepsKey] || [];
  if (bundle.asArray) writeJsonPretty(bundle.path, steps);
  else {
    bundle.root[bundle.stepsKey] = steps;
    writeJsonPretty(bundle.path, bundle.root);
  }
}

function appendProgress(dir, line) {
  const p = join(dir, "progress.md");
  const stamp = new Date().toISOString();
  const prev = existsSync(p) ? readFileSync(p, "utf8") : "# Progress\n\n";
  const base = prev.endsWith("\n") ? prev : prev + "\n";
  writeFileSync(p, base + "- " + stamp + " " + line + "\n", "utf8");
}

export function cmdExecute(args) {
  const dir = resolveOmmDir();
  ensureDir(dir);
  const bundle = loadPlan(dir);
  if (!Array.isArray(bundle.root[bundle.stepsKey])) bundle.root[bundle.stepsKey] = [];
  let steps = bundle.root[bundle.stepsKey].map((s, i) => normalizeStep(s, i));
  bundle.root[bundle.stepsKey] = steps;

  if (args[0] === "done") {
    const idx = steps.findIndex((s) => String(s.status).toLowerCase() === "in-progress");
    if (idx < 0) {
      print("error: no in-progress step");
      return 1;
    }
    steps[idx].status = "done";
    savePlan(bundle);
    appendProgress(dir, "done " + steps[idx].id + " " + steps[idx].title);
    print("done " + steps[idx].id + ": " + steps[idx].title);
    return 0;
  }

  const task = args.join(" ").trim();
  let idx = -1;
  if (task) {
    idx = steps.findIndex((s) => {
      const id = String(s.id).toLowerCase();
      const title = String(s.title).toLowerCase();
      const q = task.toLowerCase();
      return id === q || title === q || title.includes(q);
    });
  }
  if (idx < 0) {
    idx = steps.findIndex((s) => {
      const st = String(s.status || "pending").toLowerCase();
      return st === "in-progress";
    });
  }
  if (idx < 0) {
    idx = steps.findIndex((s) => {
      const st = String(s.status || "pending").toLowerCase();
      return st === "pending" || st === "todo" || st === "open";
    });
  }
  if (idx < 0) {
    const rec = { id: String(steps.length + 1), title: task || "task", status: "in-progress" };
    steps.push(rec);
    idx = steps.length - 1;
  } else {
    steps[idx].status = "in-progress";
    if (task && !steps[idx].title) steps[idx].title = task;
  }
  savePlan(bundle);
  appendProgress(dir, "in-progress " + steps[idx].id + " " + steps[idx].title);
  print("in-progress " + steps[idx].id + ": " + steps[idx].title);
  return 0;
}

export function cmdRemember(args) {
  const text = args.join(" ").trim();
  if (!text) {
    print("usage: omm remember [note...]");
    return 1;
  }
  if (looksSecretText(text)) {
    print("error: refusing to store token/secret/password/api_key material");
    return 1;
  }
  const dir = resolveOmmDir();
  ensureDir(dir);
  const ts = new Date().toISOString();
  const md = join(dir, "memory.md");
  const prev = existsSync(md) ? readFileSync(md, "utf8") : "# Memory\n\n";
  const base = prev.endsWith("\n") ? prev : prev + "\n";
  writeFileSync(md, base + "- " + ts + " " + text + "\n", "utf8");
  const jl = join(dir, "memory.jsonl");
  appendFileSync(jl, JSON.stringify({ ts, type: "note", text }) + "\n", "utf8");
  print("remembered " + ts);
  return 0;
}

export function cmdDebug(args) {
  const dir = resolveOmmDir();
  const debugDir = ensureDir(join(dir, "debug"));
  const symptom = args.join(" ").trim();
  writeMode(dir, { mode: "debug", symptom });
  const { stamp, path } = uniqueStampPath(debugDir);
  writeFileSync(
    path,
    [
      "# Debug",
      "",
      "Symptom: " + (symptom || "(unspecified)"),
      "Stamp: " + stamp,
      "",
      "## Hypotheses",
      "",
      "",
    ].join("\n"),
    "utf8",
  );
  print(path);
  return 0;
}

const TRACE_OUTLINES = {
  ralph: [
    "## /ralph + Stop",
    "- command: commands/ralph.md (in-session; arms `.omm/ralph.json`)",
    "- hook: hooks/stop_chain.py on Stop — ralph first",
    "- files: `.omm/ralph.json` `{active,goal,iterations,max}`, `.omm/mode.json`",
    "- continue until `<promise>DONE</promise>` or budget",
  ],
  ralplan: [
    "## /ralplan + omm ralplan",
    "- CLI: `omm ralplan` writes mode.json, plan.md stub, ralph.json active=false",
    "- command: commands/ralplan.md (interview prose in-session)",
    "- hook: hooks/user_prompt.py may set mode ralplan",
    "- next: `/ralph` to arm the loop",
  ],
  autopilot: [
    "## /autopilot + omm autopilot",
    "- CLI: `omm autopilot` writes mode.json, plan.json if missing, autopilot.json",
    "- hook: hooks/stop_chain.py after ralph/ulw/boulder/todo",
    "- files: `.omm/autopilot.json` `{active,step,max}`",
  ],
  execute: [
    "## /execute + omm execute",
    "- CLI: next pending plan.json step → in-progress; `omm execute done`",
    "- command: commands/execute.md (code in-session)",
    "- files: `.omm/plan.json`, `.omm/progress.md`",
  ],
  verify: [
    "## /verify + omm verify",
    "- CLI: claim → verify.json pending; pass/fail updates ok/status/evidence",
    "- command: commands/verify.md (evidence prose in-session)",
    "- files: `.omm/verify.json`, `.omm/verify.md`",
  ],
  interview: [
    "## /deep-interview + omm interview",
    "- CLI aliases: `omm interview` and `omm deep-interview`",
    "- files: `.omm/interview/<utc-stamp>.md`, `.omm/requirements.md`",
    "- command: commands/deep-interview.md (questions in-session)",
  ],
  "deep-interview": [
    "## /deep-interview + omm interview",
    "- CLI aliases: `omm interview` and `omm deep-interview`",
    "- files: `.omm/interview/<utc-stamp>.md`, `.omm/requirements.md`",
    "- command: commands/deep-interview.md (questions in-session)",
  ],
  ultragoal: [
    "## /ultragoal + omm ultragoal",
    "- CLI: mode.json ultragoal, ultragoal.md, plan.json milestone-1 pending steps",
    "- command: commands/ultragoal.md (vision prose in-session)",
  ],
  handoff: [
    "## /handoff + omm handoff",
    "- CLI: summarizes mode/plan/verify/team into `.omm/handoff.md` (no secrets)",
    "- command: commands/handoff.md",
  ],
  skillify: [
    "## /skillify + omm skillify",
    "- CLI: `.omm/skillify/<slug>.md` draft + printed SKILL.md skeleton",
    "- `--apply` writes `skills/<slug>/SKILL.md` only for portable [a-z0-9-]+ slugs",
    "- command: commands/skillify.md",
  ],
  remember: [
    "## /remember + omm remember",
    "- CLI: append `.omm/memory.md` + `.omm/memory.jsonl` `{ts,type:note,text}`",
    "- refuses token/secret/password/api_key",
    "- hook: pre_compact.py also appends memory.md",
  ],
  debug: [
    "## /debug + omm debug",
    "- CLI: `.omm/debug/<stamp>.md` with symptom + empty Hypotheses; mode.json debug",
    "- command: commands/debug.md",
  ],
  trace: [
    "## /omm-trace + omm trace",
    "- CLI: static outline of this plugin's hooks/commands (not a live tracer)",
    "- files: `.omm/trace/<slug>.md`",
  ],
  team: [
    "## /team + omm team",
    "- CLI: `.omm/team/mission.md` + roster.json; list roster/log",
    "- hooks: subagent_start.py / subagent_stop.py append team/log.jsonl",
  ],
  ask: [
    "## /ask + omm ask",
    "- CLI: scores skills/*/SKILL.md YAML description + first heading + id",
    "- writes `.omm/ask/last.json` `{query,skill,score,reason,alternatives}`",
    "- no remote model",
  ],
  hud: [
    "## /hud + omm hud",
    "- CLI: text snapshot of `.omm/` (not a TUI, not a statusline)",
    "- files: mode, ralph, plan, verify, team, memory, autopilot, interview, debug, trace",
  ],
  wiki: [
    "## /wiki + omm wiki",
    "- CLI: list/show/write `.omm/wiki/*.md`",
  ],
  "stop-chain": [
    "## stop-chain hook",
    "- source: hooks/stop_chain.py (Stop)",
    "- order: ralph, ulw/ultrawork, boulder, todo (capped nudge), then autopilot",
    "- block JSON: `{decision:block,reason}` until DONE promise or max",
  ],
  "skill-gate": [
    "## skill-gate hook",
    "- source: hooks/skill_gate.py (PreToolUse)",
    "- opt-in `.omm/skill-gate.json`; fail-open intent-gate via `.omm/intent-gate.json`",
  ],
};

const HOOK_INDEX = [
  "session-start → hooks/session_start.py (SessionStart)",
  "prompt-keywords → hooks/user_prompt.py (UserPromptSubmit) writes mode.json",
  "skill-gate → hooks/skill_gate.py (PreToolUse)",
  "stop-chain → hooks/stop_chain.py (Stop) ralph, ulw, boulder, todo, autopilot",
  "subagent-start → hooks/subagent_start.py",
  "subagent-stop → hooks/subagent_stop.py",
  "pre-compact → hooks/pre_compact.py",
  "session-end → hooks/session_end.py",
];

export function cmdTrace(args) {
  const dir = resolveOmmDir();
  const traceDir = ensureDir(join(dir, "trace"));
  const target = args.join(" ").trim() || "plugin";
  const slug = slugify(target, "plugin");
  const key = String(target).toLowerCase().trim();
  const focused = TRACE_OUTLINES[key] || TRACE_OUTLINES[slug];
  const lines = [
    "# Trace: " + target,
    "",
    "Static outline of Oh My Muse Code hooks/commands (not a live tracer of user code).",
    "",
  ];
  if (focused) lines.push(...focused);
  else {
    lines.push("## Target");
    lines.push(target);
    lines.push("");
    lines.push("No named outline for this target. Plugin map:");
  }
  lines.push("");
  lines.push("## Hooks (8)");
  for (const h of HOOK_INDEX) lines.push("- " + h);
  lines.push("");
  lines.push("## Companion CLI verbs");
  lines.push("setup doctor hud team ask wait mission wiki update ralplan interview deep-interview ultragoal handoff skillify verify autopilot execute remember debug trace");
  lines.push("");
  const path = join(traceDir, slug + ".md");
  writeFileSync(path, lines.join("\n") + "\n", "utf8");
  print(path);
  return 0;
}

export function summarizePlan(plan) {
  if (Array.isArray(plan)) return plan.length + " steps";
  if (!plan || typeof plan !== "object") return truncate(plan, 80);
  const steps = plan.steps || plan.items || plan.todos;
  const n = Array.isArray(steps) ? steps.length : null;
  const title = plan.title || plan.name || plan.summary || plan.goal || "";
  const bits = [];
  if (title) bits.push(truncate(title, 72));
  if (n != null) bits.push(n + " steps");
  if (plan.status) bits.push(String(plan.status));
  if (plan.milestone) bits.push(String(plan.milestone));
  return bits.join(", ") || "present";
}

export function summarizeVerify(v) {
  if (Array.isArray(v)) return v.length + " results";
  if (!v || typeof v !== "object") return truncate(v, 80);
  const bits = [];
  if (v.status != null) bits.push(truncate(v.status, 40));
  if (typeof v.ok === "boolean") bits.push(v.ok ? "ok" : "not-ok");
  if (typeof v.passed === "boolean") bits.push(v.passed ? "passed" : "failed");
  if (v.claim) bits.push(truncate(v.claim, 72));
  if (v.summary) bits.push(truncate(v.summary, 72));
  return bits.join(", ") || "present";
}

export function summarizeTeam(teamDir) {
  const files = listDirFiles(teamDir);
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

export function summarizeMemory(dir) {
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

function countDirMd(dir) {
  try {
    return readdirSync(dir).filter((f) => f.endsWith(".md")).length;
  } catch {
    return 0;
  }
}

export function extraHudLines(dir) {
  const lines = [];
  const autoGot = readJsonCapped(join(dir, "autopilot.json"));
  if (autoGot && autoGot.skipped) lines.push("autopilot: (file too large, skipped)");
  else if (autoGot && autoGot.value && typeof autoGot.value === "object") {
    const a = autoGot.value;
    const active = a.active === true || a.active === "true";
    const step = a.step ?? "?";
    const mx = a.max ?? "?";
    const goal = safeStr("goal", a.goal, 80);
    let row = "autopilot: active=" + active + " step=" + step + "/" + mx;
    if (goal) row += " goal: " + goal;
    lines.push(row);
  }
  if (existsSync(join(dir, "ultragoal.md"))) lines.push("ultragoal.md: present");
  if (existsSync(join(dir, "handoff.md"))) lines.push("handoff.md: present");
  if (existsSync(join(dir, "requirements.md"))) lines.push("requirements.md: present");
  if (existsSync(join(dir, "progress.md"))) {
    const n = countNonemptyLines(join(dir, "progress.md"));
    lines.push("progress.md: " + (n === "large" ? "large" : n + " lines"));
  }
  const interviewN = countDirMd(join(dir, "interview"));
  if (interviewN) lines.push("interview: " + interviewN + " notes");
  const debugN = countDirMd(join(dir, "debug"));
  if (debugN) lines.push("debug: " + debugN + " files");
  const traceN = countDirMd(join(dir, "trace"));
  if (traceN) lines.push("trace: " + traceN + " files");
  const skillN = countDirMd(join(dir, "skillify"));
  if (skillN) lines.push("skillify: " + skillN + " drafts");
  return lines;
}
