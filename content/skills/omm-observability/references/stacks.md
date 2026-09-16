# Observability stacks, per language

Companion to `omm-observability`. Find what the repo already uses with `search` before
choosing anything here; these are the idiomatic defaults when nothing exists yet.

## Structured logger, metrics client, tracer, test capture

| Language | Logger (structured) | Metrics | Tracer | Capture in tests | Context binding |
|---|---|---|---|---|---|
| Python | `structlog`; stdlib `logging` + a JSON formatter | `prometheus_client` | `opentelemetry-sdk` | `caplog` (stdlib), `structlog.testing.capture_logs()` | `structlog.contextvars.bind_contextvars(request_id=...)` in middleware |
| Node / TS | `pino` | `prom-client` | `@opentelemetry/sdk-node` | `pino(opts, sink)` writing to an in-memory stream | `AsyncLocalStorage`; `pino` child logger per request |
| Go | `log/slog` (`NewJSONHandler`) | `prometheus/client_golang` | `go.opentelemetry.io/otel` | `slog.New(slog.NewJSONHandler(&buf, nil))` | `context.Context`; `slog.With` per request |
| Rust | `tracing` + `tracing-subscriber` (`.json()`) | `metrics` or `prometheus` crate | `tracing-opentelemetry` | `tracing_test`, or a subscriber writing to a `Vec<u8>` | `#[instrument(fields(...))]`; span fields inherit |
| Java / Kotlin | SLF4J + Logback JSON encoder | Micrometer | OpenTelemetry Java agent or SDK | Logback `ListAppender` | MDC (`MDC.put("request_id", ...)`) |
| .NET | `ILogger` message templates (`{OrderId}`, never `$"..."`) | `System.Diagnostics.Metrics` | `ActivitySource` | a test `ILoggerProvider` collecting entries | `ILogger.BeginScope`, `Activity.Current` |
| Ruby | `semantic_logger`, or `Logger` with a JSON formatter | `prometheus-client` | `opentelemetry-sdk` | `StringIO` as log device | `SemanticLogger.tagged`; Rails `CurrentAttributes` |

Test the fields, not the string: `assert rec["outcome"] == "declined"`, never
`assert "declined" in rec.message`.

## Propagation headers

- HTTP and gRPC: W3C `traceparent` (`00-<trace_id 32 hex>-<span_id 16 hex>-<flags>`) and
  `tracestate`. Legacy peers may send `X-Request-ID`, `X-B3-TraceId`; accept what the
  peers already send, emit both only during a migration.
- Queues: message headers or attributes (Kafka headers, SQS message attributes, AMQP
  headers, Pub/Sub attributes); the consumer extracts and starts a child or linked span.
- Cron and batch: no parent; start a new trace, add `job_run_id`, link to the enqueuing
  trace when the message carried one.
- Never propagate through the body or through a log field; the SDK's `inject`/`extract`
  is the only path.

## Metric naming and buckets

- `<namespace>_<subsystem>_<name>_<unit>`; counters end `_total`; base units
  (`_seconds`, `_bytes`, `_ratio`), never `_ms` or `_kb`.
- Histogram buckets bracket the target: a 200 ms p95 target wants buckets around
  `[.025 .05 .1 .2 .4 .8 1.6 3.2]`, not the default `10`.
- Cardinality check before merging: `count(count by (__name__)({__name__=~"<prefix>.*"}))`
  against a staging scrape; a label with more than a few hundred values is unbounded.

## Sampling knobs

- Traces: head sampling with `ParentBased(TraceIdRatioBased(r))` so children follow the
  root's decision; tail sampling (keep errors and slow traces) lives in the collector,
  not in code.
- Logs: sample by `trace_id` hash, never per line. `zerolog` and `slog` wrappers sample
  natively; `pino` and `structlog` sample in a processor or at the shipper.
- Repeated warnings: a rate limiter keyed by message (`once per minute per key`), with
  `suppressed_count` on the next emitted line.

## Redaction patterns to `search` for

`password`, `passwd`, `secret`, `token`, `api_key`, `apikey`, `authorization`,
`cookie`, `set-cookie`, `ssn`, `card_number`, `pan`, `cvv`, `iban`; plus whole-object
dumps: `dump(`, `repr(`, `%r`, `!r}`, `JSON.stringify(req`, `{:?}`, `.to_json` on
request, user, config or settings types. Fix by field selection at the call site or a
processor that drops known keys; a `# TODO redact` is a finding.
