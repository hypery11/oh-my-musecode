#!/usr/bin/env python3
"""Offline tests for file-based omm CLI verbs (OMM_DIR / cwd)."""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CLI = ROOT / "bin" / "omm.mjs"


def fail(msg: str) -> None:
    print(f"FAIL  {msg}", file=sys.stderr)
    raise SystemExit(1)


def run_omm(args, env, cwd) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["node", str(CLI), *args],
        text=True,
        capture_output=True,
        check=False,
        cwd=str(cwd),
        env=env,
    )


def main() -> None:
    tmp = Path(tempfile.mkdtemp(prefix="omm-cli-"))
    omm = tmp / "state"
    decoy = tmp / "decoy"
    decoy.mkdir()
    env = {**os.environ, "OMM_DIR": str(omm)}
    apply_slug = "tmp-omm-cli-test"
    apply_dir = ROOT / "skills" / apply_slug
    try:
        r = run_omm(["team", "ship", "wiki", "cli"], env, decoy)
        if r.returncode != 0:
            fail(f"team init exit {r.returncode}: {r.stderr}")
        if not (omm / "team" / "mission.md").is_file():
            fail("team did not write mission.md under OMM_DIR")
        roster = json.loads((omm / "team" / "roster.json").read_text(encoding="utf-8"))
        if "architect" not in json.dumps(roster):
            fail("roster missing roles")
        r = run_omm(["team"], env, decoy)
        if r.returncode != 0 or "roster:" not in r.stdout:
            fail(f"team list: {r.stdout!r}")

        r = run_omm(["ask", "git", "rebase", "conflict"], env, decoy)
        if r.returncode != 0:
            fail(f"ask exit {r.returncode}: {r.stderr}")
        if "git-master" not in r.stdout:
            fail(f"ask expected git-master, got {r.stdout!r}")
        last = json.loads((omm / "ask" / "last.json").read_text(encoding="utf-8"))
        if last.get("skill") != "git-master":
            fail(f"ask last.json skill={last.get('skill')}")
        if last.get("query") != "git rebase conflict":
            fail(f"ask last.json query={last.get('query')}")
        if "alternatives" not in last or not isinstance(last["alternatives"], list):
            fail("ask last.json missing alternatives")
        if last.get("score") in (None, 0):
            fail(f"ask score should be >0, got {last.get('score')}")

        r = run_omm(["mission", "land", "phase-2"], env, decoy)
        if r.returncode != 0 or "queued 1" not in r.stdout:
            fail(f"mission append: {r.stdout!r}")
        r = run_omm(["mission"], env, decoy)
        if "pending" not in r.stdout:
            fail(f"mission list: {r.stdout!r}")
        r = run_omm(["mission", "done", "1"], env, decoy)
        if r.returncode != 0:
            fail("mission done failed")
        queue = json.loads((omm / "mission" / "queue.json").read_text(encoding="utf-8"))
        if queue[0].get("status") != "done":
            fail("mission not marked done")

        r = run_omm(["wiki", "write", "home", "hello wiki"], env, decoy)
        if r.returncode != 0:
            fail(f"wiki write: {r.stderr}")
        r = run_omm(["wiki", "list"], env, decoy)
        if "home" not in r.stdout:
            fail(f"wiki list: {r.stdout!r}")
        r = run_omm(["wiki", "show", "home"], env, decoy)
        if "hello wiki" not in r.stdout:
            fail(f"wiki show: {r.stdout!r}")

        r = run_omm(["wait", "0"], env, decoy)
        if r.returncode != 1 or "timeout" not in r.stdout:
            fail(f"wait empty: code={r.returncode} out={r.stdout!r}")
        log = omm / "team" / "log.jsonl"
        log.parent.mkdir(parents=True, exist_ok=True)
        log.write_text('{"event":"subagent-stop","id":"a"}\n', encoding="utf-8")
        r = run_omm(["wait", "0"], env, decoy)
        if r.returncode != 0 or "subagent-stop" not in r.stdout:
            fail(f"wait existing log: {r.stdout!r}")

        r = run_omm(["update"], env, decoy)
        if r.returncode != 0:
            fail(f"update exit {r.returncode}")
        if "muse plugins update" not in r.stdout or "muse plugins approve" not in r.stdout:
            fail(f"update help: {r.stdout!r}")

        r = run_omm(["ralplan", "ship", "ralph"], env, decoy)
        if r.returncode != 0:
            fail(f"ralplan exit {r.returncode}: {r.stderr}")
        mode = json.loads((omm / "mode.json").read_text(encoding="utf-8"))
        if mode.get("mode") != "ralplan" or mode.get("topic") != "ship ralph":
            fail(f"ralplan mode.json {mode}")
        if not (omm / "plan.md").is_file():
            fail("ralplan missing plan.md")
        ralph = json.loads((omm / "ralph.json").read_text(encoding="utf-8"))
        if ralph.get("active") is not False or ralph.get("iterations") != 0 or ralph.get("max") != 20:
            fail(f"ralplan ralph.json {ralph}")
        if ralph.get("goal") != "ship ralph":
            fail(f"ralplan goal {ralph.get('goal')}")
        (omm / "plan.md").write_text("# keep me\n", encoding="utf-8")
        r = run_omm(["ralplan", "again"], env, decoy)
        if (omm / "plan.md").read_text(encoding="utf-8") != "# keep me\n":
            fail("ralplan overwrote existing plan.md")

        r = run_omm(["interview", "auth", "flow"], env, decoy)
        if r.returncode != 0:
            fail(f"interview exit {r.returncode}: {r.stderr}")
        notes = list((omm / "interview").glob("*.md"))
        if len(notes) != 1:
            fail(f"interview notes {notes}")
        if "Hypotheses" in notes[0].read_text(encoding="utf-8"):
            fail("interview file should not use debug Hypotheses heading")
        if "Subject: auth flow" not in notes[0].read_text(encoding="utf-8"):
            fail("interview missing subject")
        if not (omm / "requirements.md").is_file():
            fail("interview missing requirements.md")
        r = run_omm(["deep-interview", "alias"], env, decoy)
        if r.returncode != 0:
            fail(f"deep-interview alias exit {r.returncode}")
        if len(list((omm / "interview").glob("*.md"))) != 2:
            fail("deep-interview should add a second stamp file")

        r = run_omm(["ultragoal", "north", "star"], env, decoy)
        if r.returncode != 0:
            fail(f"ultragoal exit {r.returncode}: {r.stderr}")
        mode = json.loads((omm / "mode.json").read_text(encoding="utf-8"))
        if mode.get("mode") != "ultragoal":
            fail(f"ultragoal mode {mode}")
        if not (omm / "ultragoal.md").is_file():
            fail("missing ultragoal.md")
        plan = json.loads((omm / "plan.json").read_text(encoding="utf-8"))
        if plan.get("milestone") != "milestone-1":
            fail(f"ultragoal plan milestone {plan}")
        steps = plan.get("steps") or []
        if not steps or any(s.get("status") != "pending" for s in steps):
            fail(f"ultragoal steps {steps}")

        r = run_omm(["handoff", "next", "chat"], env, decoy)
        if r.returncode != 0:
            fail(f"handoff exit {r.returncode}: {r.stderr}")
        handoff_path = (omm / "handoff.md").resolve()
        if str(handoff_path) not in r.stdout:
            fail(f"handoff should print path, got {r.stdout!r}")
        body = handoff_path.read_text(encoding="utf-8")
        if "mode" not in body.lower() or "plan" not in body.lower():
            fail("handoff.md missing mode/plan summary")

        r = run_omm(["skillify", "deploy-flow"], env, decoy)
        if r.returncode != 0:
            fail(f"skillify exit {r.returncode}: {r.stderr}")
        if not (omm / "skillify" / "deploy-flow.md").is_file():
            fail("skillify missing draft")
        if "suggested SKILL.md" not in r.stdout:
            fail(f"skillify should print skeleton: {r.stdout!r}")
        if (ROOT / "skills" / "deploy-flow").exists():
            fail("skillify without --apply mutated plugin skills")
        r = run_omm(["skillify", "--apply", apply_slug], env, decoy)
        if r.returncode != 0:
            fail(f"skillify --apply exit {r.returncode}: {r.stderr} {r.stdout}")
        if not (apply_dir / "SKILL.md").is_file():
            fail("skillify --apply did not write SKILL.md")
        shutil.rmtree(apply_dir, ignore_errors=True)

        r = run_omm(["verify", "hooks", "green"], env, decoy)
        if r.returncode != 0:
            fail(f"verify claim exit {r.returncode}")
        v = json.loads((omm / "verify.json").read_text(encoding="utf-8"))
        if v.get("claim") != "hooks green" or v.get("status") != "pending" or v.get("ok") is not False:
            fail(f"verify.json {v}")
        if not isinstance(v.get("checks"), list):
            fail("verify checks missing")
        if not (omm / "verify.md").is_file():
            fail("missing verify.md")
        r = run_omm(["verify"], env, decoy)
        if r.returncode != 0 or "pending" not in r.stdout:
            fail(f"verify print: {r.stdout!r}")
        r = run_omm(["verify", "pass", "pytest ok"], env, decoy)
        if r.returncode != 0 or "pass" not in r.stdout:
            fail(f"verify pass: {r.stdout!r}")
        v = json.loads((omm / "verify.json").read_text(encoding="utf-8"))
        if v.get("ok") is not True or v.get("status") != "pass" or v.get("evidence") != "pytest ok":
            fail(f"verify pass state {v}")
        r = run_omm(["verify", "fail", "flake"], env, decoy)
        if r.returncode != 0:
            fail("verify fail")
        v = json.loads((omm / "verify.json").read_text(encoding="utf-8"))
        if v.get("ok") is not False or v.get("status") != "fail":
            fail(f"verify fail state {v}")

        r = run_omm(["autopilot", "land", "it"], env, decoy)
        if r.returncode != 0:
            fail(f"autopilot exit {r.returncode}")
        mode = json.loads((omm / "mode.json").read_text(encoding="utf-8"))
        if mode.get("mode") != "autopilot":
            fail(f"autopilot mode {mode}")
        auto = json.loads((omm / "autopilot.json").read_text(encoding="utf-8"))
        if auto.get("active") is not True or auto.get("step") != 0 or auto.get("max") != 20:
            fail(f"autopilot.json {auto}")
        if not (omm / "plan.json").is_file():
            fail("autopilot should keep/create plan.json")

        # reset plan steps for execute
        (omm / "plan.json").write_text(
            json.dumps(
                {
                    "steps": [
                        {"id": "1", "title": "first", "status": "pending"},
                        {"id": "2", "title": "second", "status": "pending"},
                    ]
                },
                indent=2,
            )
            + "\n",
            encoding="utf-8",
        )
        r = run_omm(["execute"], env, decoy)
        if r.returncode != 0 or "in-progress" not in r.stdout:
            fail(f"execute: {r.stdout!r}")
        plan = json.loads((omm / "plan.json").read_text(encoding="utf-8"))
        if plan["steps"][0].get("status") != "in-progress":
            fail(f"execute did not mark first in-progress: {plan}")
        if not (omm / "progress.md").is_file():
            fail("execute missing progress.md")
        r = run_omm(["execute", "done"], env, decoy)
        if r.returncode != 0 or "done" not in r.stdout:
            fail(f"execute done: {r.stdout!r}")
        plan = json.loads((omm / "plan.json").read_text(encoding="utf-8"))
        if plan["steps"][0].get("status") != "done":
            fail(f"execute done state {plan}")
        r = run_omm(["execute", "second"], env, decoy)
        if r.returncode != 0:
            fail("execute named step")
        plan = json.loads((omm / "plan.json").read_text(encoding="utf-8"))
        if plan["steps"][1].get("status") != "in-progress":
            fail(f"execute second {plan}")

        r = run_omm(["remember", "prefer", "small", "diffs"], env, decoy)
        if r.returncode != 0:
            fail(f"remember exit {r.returncode}")
        mem = (omm / "memory.md").read_text(encoding="utf-8")
        if "prefer small diffs" not in mem:
            fail("memory.md missing note")
        jl = (omm / "memory.jsonl").read_text(encoding="utf-8").strip().splitlines()
        rec = json.loads(jl[-1])
        if rec.get("type") != "note" or rec.get("text") != "prefer small diffs" or "ts" not in rec:
            fail(f"memory.jsonl {rec}")
        r = run_omm(["remember", "leaked", "api_key", "value"], env, decoy)
        if r.returncode != 1 or "error" not in r.stdout.lower():
            fail(f"remember should refuse api_key: code={r.returncode} out={r.stdout!r}")
        r = run_omm(["remember", "password", "dump"], env, decoy)
        if r.returncode != 1:
            fail("remember should refuse password")
        r = run_omm(["remember", "secret", "token", "here"], env, decoy)
        if r.returncode != 1:
            fail("remember should refuse token/secret")

        r = run_omm(["debug", "hook", "crash"], env, decoy)
        if r.returncode != 0:
            fail(f"debug exit {r.returncode}")
        mode = json.loads((omm / "mode.json").read_text(encoding="utf-8"))
        if mode.get("mode") != "debug":
            fail(f"debug mode {mode}")
        dbg = list((omm / "debug").glob("*.md"))
        if len(dbg) != 1:
            fail(f"debug files {dbg}")
        dtext = dbg[0].read_text(encoding="utf-8")
        if "Symptom: hook crash" not in dtext or "## Hypotheses" not in dtext:
            fail(f"debug md {dtext!r}")

        r = run_omm(["trace", "ralph"], env, decoy)
        if r.returncode != 0:
            fail(f"trace exit {r.returncode}")
        tpath = omm / "trace" / "ralph.md"
        if not tpath.is_file():
            fail("trace missing ralph.md")
        ttext = tpath.read_text(encoding="utf-8")
        if "stop_chain" not in ttext and "stop-chain" not in ttext:
            fail("trace ralph should mention stop-chain")
        if "not a live tracer" not in ttext.lower():
            fail("trace should say it is static")

        r = run_omm(["hud"], env, decoy)
        if r.returncode != 0:
            fail(f"hud exit {r.returncode}")
        for needle in ("autopilot:", "interview:", "debug:", "handoff.md", "ultragoal.md"):
            if needle not in r.stdout:
                fail(f"hud missing {needle}: {r.stdout}")

        r = run_omm(["not-a-command"], env, decoy)
        if r.returncode != 1:
            fail("unknown command should exit 1")

        help_out = run_omm(["--help"], env, decoy).stdout
        verbs = (
            "team", "ask", "wait", "mission", "wiki", "update", "setup", "doctor", "hud",
            "ralplan", "interview", "deep-interview", "ultragoal", "handoff", "skillify",
            "verify", "autopilot", "execute", "remember", "debug", "trace",
        )
        for verb in verbs:
            if verb not in help_out:
                fail(f"HELP missing {verb}")

        print("ok  cli team/ask/wait/mission/wiki/update + ralplan/interview/verify/autopilot engines against OMM_DIR")
    finally:
        shutil.rmtree(apply_dir, ignore_errors=True)
        shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    main()
