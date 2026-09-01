#!/usr/bin/env node
/**
 * Oh My Muse Code companion CLI (no runtime dependencies).
 * Real work: setup + doctor. Other subcommands are honest stubs.
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
  omm team|ask|hud|wait|mission|wiki|update
                         Planned stubs (plugin slash-commands are the live path)
  omm -h, --help         Show this help
  omm -V, --version      Print version

Environment:
  MUSE_EXPERIMENTAL_PLUGINS=1   required for Muse 1.0.1 plugin commands
  MUSE_NO_AUTO_UPDATE=1         recommended during validate
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
    case "team":
    case "ask":
    case "hud":
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
