## Summary

## Checklist
- [ ] Original prose (no paste from oh-my-claudecode / oh-my-codex / oh-my-grok / oh-my-openagent)
- [ ] Skill paths are files (`skills/<id>/SKILL.md`); command ids do not collide with skill ids
- [ ] Each hook id uses a unique source file; `command` is an argv array; no `matcher`
- [ ] No `capabilities.tools` / `agents` / `outputStyles` / `settings` / `apps`
- [ ] Did not log into Meta / dump `auth.json` / add an API proxy
- [ ] `python3 -m py_compile hooks/*.py` and `node --check bin/omm.mjs` pass
- [ ] Docs stay honest about TEMPLATE vs STUB vs SHIPPED
