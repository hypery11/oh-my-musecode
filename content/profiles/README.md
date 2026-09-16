# omm settings profiles

Each file is a SLICE of `$XDG_CONFIG_HOME/muse/settings.json` (else `~/.config/muse/settings.json`)
containing only typed keys from `docs/host-data/settings-keys.json`. `omm profile use <name>` merges
the slice as a targeted, validated, atomic patch and records every key's prior value in the ledger
(R9/R10); it never writes the whole file, because Muse rewrites `settings.json` from a typed struct
and destroys unknown keys. Switching profiles restores the previous profile's keys first.

Validation: `muse config validate --plane defaults --file <tmp>` on
`{"schema_version":1,"settings":<slice>}` for every key EXCEPT `permissions`, which the enterprise
validator does not know (`unknown_member` whatever its content). `permissions` is validated by the
runtime oracle: load the slice in a sandbox `HOME`, run `muse exec --provider echo hi`, and read
`runtime.session.permission_profile_committed` (`source.kind = user_named`, `id = omm-*`) from
`session.jsonl`. A profile that fails to resolve prints `Permission profile '<id>' is unavailable`
and Muse falls back to `:ask-me` - one bad profile in the map disables every named profile, so omm
only ever adds `omm-*` profile ids and never edits the user's own. Profile objects carry only the
documented `PermissionProfileDefinitionInputV1` members (`approval`, `reviewer`, `filesystem`,
`network`, `extends`; `docs/host-data/settings-keys.json` item 22, parent `deny_unknown_fields`) - no
`description` or other prose member, since one rejected member would void the whole map. What each
profile is for lives in this file: strict = managed sandbox, proxy-only network, prompt on every
unmatched action, human reviewer; ci = managed sandbox, restricted network, no prompts, no reviewer,
headless runs only.

## Key effects

| key | effect (measured on 1.0.1-R2006.1) |
|---|---|
| `permissions.profiles.omm-strict` | `extends ":ask-me"` (managed filesystem sandbox, proxy-only network); `approval: prompt_unmatched` prompts for every action no rule matches; `reviewer: human` turns the automated approval judge off. |
| `permissions.profiles.omm-ci` | `extends ":ask-me"`, `approval: allow_all`, `reviewer: none`: no prompt ever reaches a TTY. `network.mode: restricted` is required with `allow_all` (`proxy_only` + `allow_all` is "contradictory authority" and voids the profile); set it to `enabled` when the CI job needs direct network. `:unrestricted` cannot be a parent (`invalid parent profile id`). |
| `permissions.default_profile` | The profile a session commits when no `--permission-profile` is given. `--permission-profile` is mutually exclusive with `--yolo`, `--approval-mode`, `--disable-sandbox`, `--disable-approval`, `--sandbox-network`. |
| `run.context_slimming.skill_catalog_descriptions` | `first_sentence` cuts every catalog `<description>` at its first sentence: 23,838 -> 11,376 B (-52%) on 40 user skills + built-ins; the first sentence becomes the whole trigger surface. `full` (strict) keeps negative-trigger clauses visible. |
| `run.context_slimming.full_skill_description_ids` | Exact display ids kept at full length. The default is `["bundled:git"]` and a user value REPLACES it, so every profile re-lists it (740 -> 30 B otherwise). |
| `run.context_slimming.excluded_tool_names` | `["workflow"]` removes the tool AND the two workflow context blocks: -20,952 B of run context, -15,766 B of tool JSON on the wire. `[]` is the explicit no-op (default, strict). Never `["bash"]` alone (run fails: `bash_input` requires `bash`); `write_todos` is not removable. |
| `run.context_slimming.session_identity_enabled` | `false` drops the order-240 `session_identity` block (776-786 B). |
| `run.context_usage_message_enabled` | `false` suppresses the context-usage message (ci). |
| `reasoning_effort` | Default `high`; `medium` in fast. `--effort`/`MUSE_MODEL` overrides win. Ignored by the echo provider. |
| `max_consecutive_stop_hook_continuations` | Cap on `Stop`-hook `decision: block` continuations per turn; default 8 (9 assistant turns). `1` in ci so the omm-stop nudge can never loop a headless job. |
| `telemetry.enabled` | `false`: no telemetry upload (ci). |
| `feature_config.enabled` | `false`: no server-pushed feature config (kill lists, gates) fetched (ci). |
| `local_session_messaging.enabled` | `false`: no cross-session peer messaging tools (ci). |
| `notifications.condition` | `off`: no OSC9/bell notifications (ci; harmless under `exec`). |

Not expressible as a typed key: turning off `session.jsonl` (`MUSE_NO_SESSION_LOG` is not read by
the binary; the log is always written under `$XDG_DATA_HOME/muse/sessions/`). Point `XDG_DATA_HOME`
at a job-scoped temp dir in CI instead. `tui.*` keys are omitted from ci because the `exec` lane
never reads them; `tui.color_depth` and `tui.theme` are `field_not_activated` for the validator
anyway (omm sets the theme through `omm theme`, not a profile).

Echo-safe: no profile sets `provider` or `model`, so every profile loads under `--provider echo`
(the doctor/cost probe lane) exactly as under the meta provider.
