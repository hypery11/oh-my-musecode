---
name: omm-observability
description: "Use when asked to add logging, metrics or tracing, or we cannot see what happened: structured events, stable keys, golden signals, trace context, no secrets or PII, sampled noise; Do not use when production is down right now (omm-incident)."
---

# Observability

Contract: instrumentation is done only when the final message pastes one real emitted
log line, metric sample or span from a run you executed with `bash`, plus the grep of
that output for the test secret returning nothing. "Added logging" without the line is
not done. State the question the data must answer before the code that emits it.
Reuse the repo's logger, metrics client and tracer; never add a second one.

## 0. Study what exists

- `search` for the logger, metrics client and tracer already in use (`getLogger`,
  `structlog`, `slog`, `zap`, `pino`, `tracing::`, `prometheus`, `statsd`,
  `opentelemetry`, `start_span`, `traceparent`). A second library is a finding.
- `read_file` one instrumented handler end to end: format (JSON, key=value), key
  names, levels, where request context is bound (middleware, consumer loop). Copy
  exactly. Collect the keys in use (`search`, regex, `trace_id|request_id|user_id`).
- State the question: "which step failed for order 4711 and why", "p95 of checkout by
  outcome". Then the query you would run (`event="charge" AND outcome!="ok"`). No
  query, no line.
- Per-language idioms, capture fixtures, header names: `references/stacks.md` beside
  this file (directory from the `locator:` line of the read_skill result).
- `write_todos`: one item per event, metric or span; verify last.

## 1. Pick the signal

| Question | Signal |
|---|---|
| what happened to THIS request, with which values | log event |
| how often, how slow, how broken, over time; alert on it | metric |
| where did the time go across services or threads | span |
| all three at one boundary | span + duration histogram + one log line at the end |

Never derive a metric from log volume.

## 2. Log events

- One line per meaningful event: a boundary crossed (request in, call out, job start),
  a decision (retry, fallback, cache miss), a state change, a failure, input rejected.
  Not function entry, not a loop iteration, not `starting`/`done` pairs: one line at
  the end with `outcome` and `duration_ms`.
- Constant message, variables in fields: `log.info("gateway.charge", outcome="declined",
  amount_cents=1200)`, never `f"charge {amount} declined"`; a formatted message is
  unqueryable.
- Stable keys: `search` for the key before inventing one; one name per concept
  everywhere. snake_case, unit in the name (`duration_ms`, `size_bytes`). Defaults when
  the repo has none:

  | concept | key |
  |---|---|
  | correlation | `trace_id`, `span_id`, `request_id` |
  | actor | `user_id`, `tenant_id` (ids only) |
  | domain object | `<noun>_id` (`order_id`) |
  | outcome | `outcome` (`ok`/`error`/...), `error_type`, `error_code` |
  | where | `service`, `route` (template, not URL), `method`, `status` |

- Correlation on every line from bound context (contextvars, `logger.bind`, `With`,
  MDC) set once at the entry point, never threaded by hand. None exists: add it first.
- Levels: ERROR = a human must act and nobody upstream will; WARN = degraded, handled;
  INFO = the event (an expected 4xx included); DEBUG = off in production. A retried
  failure that succeeds is one WARN, never ERROR per attempt.
- Errors: log once, at the layer that handles (returns, retries, converts); never
  log-and-rethrow at each layer. Fields `error_type`, `error`, the ids to find it.

## 3. Metrics: four golden signals per boundary

- Latency: histogram `duration_seconds{op, outcome}`. Traffic: `requests_total`.
  Errors: the `outcome` label on those, not a separate counter that drifts.
  Saturation: a gauge of what runs out (`pool_in_use`, `queue_depth`, `inflight`).
- Labels are bounded sets you can list: route template not URL, status class not
  code, `outcome` from an enum. Never `user_id`, `email`, `order_id`, a raw path or an
  error message: each distinct value is a new time series.
- Record success and failure at the same site (`finally`, `defer`, middleware) so
  denominators match. Durations go in histograms; never compute an average yourself.
  Base units in the name (`_seconds`, `_bytes`, `_total`).

## 4. Traces

- One span per boundary crossing: inbound request, outbound HTTP/gRPC, db query, queue
  publish and consume, cache miss. Not per function.
- Propagate: inject on every outbound call (W3C `traceparent`, or what the repo's peers
  already send), extract on every inbound one; queue messages carry it in headers.
- Span attributes reuse the log keys; `trace_id` lands in every log line from the
  active span, not by hand. Span status error only where the log is ERROR.

## 5. Redaction and sampling

- Never in a field: passwords, tokens, API keys, `Authorization` and `Cookie` headers,
  session ids, full card or account numbers, raw bodies or user input. PII (email,
  name, phone, IP): only what the repo already logs. `user_id` and `card_last4`, not
  the identity.
- Never dump an object: `search` for `dump(`, `repr(`, `%r`, `JSON.stringify(req`,
  `{:?}` on a request or config type; pick fields. Redact in one place (a logger
  processor with a denylist of key names), not by hand at each call.
- Sample by trace, never by line. Keep every error, WARN and slow request; sample the
  rest above a few hundred events per second. Batch loops: one summary line with
  counts. Health checks: out of logs.

## 6. Verify

- Run the path with `bash` (an existing test, a local request, a script) so the new
  line, sample or span is emitted. Paste one real line.
- Check it: parses (`| jq .` for JSON); the step 0 keys present and non-empty.
  Metrics: `curl -s localhost:<port>/metrics | grep <name>`. Then `grep` the captured
  output for the test secret, token and card number: nothing.
- Add a test on the structured fields through a capture fixture (`caplog`, an in-memory
  exporter, a test registry), never on the message string.
- Final message: per addition, the question it answers and the pasted line.

## Judgment calls

- The repo logs with `print`/`console.log`: instrument the boundary you were asked for
  with a structured logger; migrating the rest is a separate task.
- No tracer installed: `request_id` bound at the entry and forwarded as a header gives
  most of the value. A tracing SDK is a dependency decision: propose.
- An INFO line at 10k requests per second is a bill: retention and budget unknown,
  ask first. Debug lines "just for now": at `debug`, removal a todo in the message.
- Renaming an existing line or key breaks saved queries: `search` `alerts/`,
  `*.rules.yml`, `dashboards/` first, then propose.

## Refuse

- Variables interpolated into the message; two logging libraries; `print` as logging.
- Unbounded label values; an error message as a label.
- Log-and-rethrow at each layer; ERROR on a handled retry.
- Whole request, response, headers or config in a field; `# TODO redact` on a secret.
- "Added logging" without the pasted line and the empty secret grep.

## Micro-example (Python)

"We cannot see why some payments fail." Question: which gateway outcome, per attempt,
findable by order. Existing: `structlog` with `request_id` bound in middleware,
`prometheus_client`, a tracer. `search` for `charge(` finds `logger.error(f"charge
failed for {card.number}: {e}")` before a re-raise: card number logged, variable
message, ERROR before the retry loop has decided, no metric. `edit_file`:

```python
with tracer.start_as_current_span("gateway.charge"):
    t0, outcome = time.monotonic(), "ok"
    try:
        return gateway.charge(card, amount)
    except GatewayError as e:
        outcome = e.code or "error"          # bounded: declined, timeout, error
        raise
    finally:
        s = time.monotonic() - t0
        CHARGE_SECONDS.labels(outcome=outcome).observe(s)
        log.info("gateway.charge", order_id=order.id, outcome=outcome,
                 amount_cents=amount, card_last4=card.last4, duration_ms=round(s * 1000))
```

ERROR moves to the caller that gives up after the last retry. `pytest
tests/test_charge.py -k declined -s` emits
`{"event":"gateway.charge","order_id":4711,"outcome":"declined","card_last4":"4242",
"duration_ms":38,"trace_id":"4bf9..."}`; `grep 4242424242424242` on the captured
output: nothing.
