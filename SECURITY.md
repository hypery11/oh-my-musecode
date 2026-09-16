# Security

Report vulnerabilities **privately** via GitHub Security Advisories on this repository. Do not open a public issue with secrets, tokens, `auth.json`, or hook stdin dumps that may contain prompts.

## What this project does
- A Muse Code plugin: markdown skills/commands plus hooks dispatched by the local `omm` binary.
- Hooks read Muse stdin JSON and may write `.omm/` in the workspace.
- There is **no** Meta API proxy and no cloud sidecar.

## What to include
- Muse version (`muse --version`)
- Plugin version
- Hook id and whether it was approved
- Redacted reproduce steps

Maintainer: [hypery11](https://github.com/hypery11)
