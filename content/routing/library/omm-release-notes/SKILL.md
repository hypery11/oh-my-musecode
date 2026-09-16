---
name: omm-release-notes
description: Draft release notes or a changelog entry from the commits since the last tag (what changed since, changelog entry for the release, version notes), grouped for users and linked to the commits; Do not use to cut the tag or publish the release, and not for a single commit's message (omm-commit-message).
metadata:
  triggers: release notes, changelog, changelog entry, what changed since, since the last tag, since the last release, version notes, release summary, prepare the release notes
---

# Release notes

Goal: a changelog entry a user of the software can act on, derived from the
real commit range, not from memory of the session. Output: the entry in the
repo's existing format, and the range it covers.

## 1. Fix the range

```
git describe --tags --abbrev=0            # last tag; none -> use the first commit
git log --oneline <tag>..HEAD             # the range
git log <tag>..HEAD --format='%h %s%n%b' # subjects + bodies (the WHY lives here)
```

Read the bodies, not just the subjects. Also `git diff --stat <tag>..HEAD` for
paths that reveal user-facing surface (CLI, config, public API, docs).

## 2. Match the repo's format

`read_file` the existing `CHANGELOG.md` / `CHANGES` / `docs/releases/*` head
and mirror it exactly: heading style, version and date line, section names
(Keep a Changelog `Added / Changed / Deprecated / Removed / Fixed / Security`,
or the repo's own), link style. No file yet: use Keep a Changelog headings.

## 3. Write for the reader, not the committer

- One line per user-visible change, imperative or past tense as the file
  already uses; name the feature, flag, command or API, never the file.
- Breaking changes first, under their own heading, each with the migration
  step.
- Fold internal-only commits (refactors, CI, formatting) into one line or
  drop them; a changelog is not `git log`.
- Reference the commit or PR the way the file already does (`(#123)`,
  short sha) so every line is traceable.
- Unreleased version: heading `## [Unreleased]` unless the user gave the
  version; do not invent a version number or a date.

## 4. Hand back

Show the entry, state the range (`<tag>..<sha>`, N commits), and list what
you deliberately left out. Edit the changelog file only when the user asked
for the file change (`edit_file`, inserting above the previous entry); never
tag, push or publish.
