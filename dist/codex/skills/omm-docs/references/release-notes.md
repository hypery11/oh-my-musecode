# Release notes: full checklist, template, anti-patterns

Release notes are the CHANGELOG's Unreleased section promoted to a version
and rewritten for someone deciding whether and how to upgrade. Run the list
top to bottom before cutting a version. Every item is a `bash`, `search`, or
`read_file` check, not an opinion. The short form lives in SKILL.md step 7.

## Inputs

- [ ] `git log --oneline <last-tag>..HEAD` gone through in full; every commit
      mapped to a CHANGELOG line or consciously skipped (refactor, test-only, CI).
- [ ] `git diff <last-tag>...HEAD --stat` scanned for files that imply a
      visible change nobody wrote up: migrations, schema files, `--help`
      text, config examples, install scripts, lockfiles with major bumps.
- [ ] Version number matches the house scheme and the size of the change.
      A removed flag or a changed default is not a patch release.

## Content, in this order

- [ ] **Breaking** first, each with: what breaks, how to detect it, the exact
      edit that fixes it. Labelled the way the file already labels them.
- [ ] **Removed / deprecated**: old name, replacement, last version it works.
- [ ] **Added**: one line per capability, leading with what the reader can do.
- [ ] **Changed**: old value, new value, how to keep the old behaviour.
- [ ] **Fixed**: the symptom the reader saw, not the internal cause.
      "Crash on empty config" beats "Null check in loader".
- [ ] **Security**: present only if real; never buried in Fixed.
- [ ] Upgrade steps as a numbered list if any step is more than "install".
- [ ] Minimum runtime, platform, or dependency changes stated.

## Line quality

- [ ] Every line names a user-visible surface (flag, command, config key,
      API, file format, behaviour). A line naming a module or function is
      internal: cut it or rewrite it from the outside.
- [ ] No "various", "misc", "improvements", "cleanup", "refactor".
- [ ] No adjectives (powerful, seamless, robust, simple).
- [ ] Past tense or present tense as the file already uses; never mixed.
- [ ] Issue and PR references in the exact house format, all resolvable.
- [ ] Contributor credits only if the file already has them.

## Cross-checks

- [ ] Every name in the notes: `search` it in the code at HEAD. Zero hits
      means a rename or removal the notes got wrong.
- [ ] Every removal in the notes: `search` the docs for the old name; a
      reference doc that still teaches it is a doc bug: fix or report.
- [ ] README and reference docs describe the released behaviour, not the
      pre-release one. `git diff <last-tag>...HEAD -- '*.md'` gone through once.
- [ ] Every example in docs touched this cycle has been run at HEAD.
- [ ] Version string, date, and compare link (if the file has them) match
      the tag being cut.

## Template

Copy the house format if one exists. Otherwise:

```
## [X.Y.Z] - YYYY-MM-DD

### Breaking
- `<surface>`: <what breaks>. <detect it by ...>. <fix: exact edit>.

### Removed
- `<old>` (deprecated since A.B.C); use `<new>`.

### Added
- `<surface>` <does what, for whom> (#NNN)

### Changed
- `<surface>` default is <new>, was <old>; set <how> to keep <old> (#NNN)

### Fixed
- <symptom the reader saw> (#NNN)

### Upgrading
1. <step>
2. <step>
```

## Anti-patterns

- Release notes generated from the commit list with the prefixes stripped.
- A "Highlights" paragraph that repeats the Added section with adjectives.
- Breaking changes listed under Changed because the label felt harsh.
- Notes written before the last PR merged, then never re-read.
- Documenting a feature that is still behind a flag as if it shipped.
