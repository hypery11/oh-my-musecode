# Profilers and timing harnesses by language

Loaded on demand from omm-perf; the loop is in SKILL.md. Detect the toolchain with
`search` on manifest files (`Cargo.toml`, `go.mod`, `package.json`, `pyproject.toml`,
`*.csproj`, `Package.swift`, `Gemfile`, `pom.xml`). Prefer a harness the project
already has (`search` for `bench`, `criterion`, `pytest-benchmark`, `jmh`,
`BenchmarkDotNet`). Prefer a sampling profiler on the real workload over a tracing
one (tracing inflates small, frequently called functions). Always a release or
optimised build with symbols kept. Check a tool exists with `bash`
(`command -v <tool>`) before planning around it; missing: say so, install only with
the project's own package manager (`cargo install`, `npm i -D`, `uv add --dev`,
`go install`), never into the system interpreter, else fall back to the generic row.

## Generic CLI (any language)

- Timing loop, median of N (warm-up first, output to `/dev/null`):
  ```sh
  <cmd> >/dev/null 2>&1   # warm-up, discarded
  for i in $(seq 7); do /usr/bin/time -p <cmd> 2>&1 >/dev/null | awk '/^real/{print $2}'; done \
    | sort -n | awk '{a[NR]=$1} END {print "min", a[1], "median", a[int((NR+1)/2)], "max", a[NR]}'
  ```
- `hyperfine --warmup 1 -r 10 -N '<cmd>'` when installed (`-N` skips the shell).
- Peak RSS: macOS `/usr/bin/time -l <cmd> 2>&1 >/dev/null | awk '/maximum resident/{print $1}'`
  (bytes); Linux `/usr/bin/time -v <cmd> 2>&1 >/dev/null | awk -F: '/Maximum resident/{print $2}'`
  (KB). `time` in a shell is the builtin without `-v`/`-l`; use the absolute path.
- Sampling without instrumentation: Linux `perf record -g <cmd>; perf report --stdio | head -60`;
  macOS `xctrace record --template 'Time Profiler' --launch -- <cmd>` or `sample <pid> 10`
  for a running process.

## Per language

| Language | CPU time | Memory / allocation | Micro-benchmark (only for a proven hotspot) | Traps |
|---|---|---|---|---|
| Python | `py-spy record -o prof.svg -- python <entry>` (no code change; `--pid` for a running process; `--idle` shows I/O waits); `python -m cProfile -o out.prof <entry>` then `python -c "import pstats; pstats.Stats('out.prof').sort_stats('cumulative').print_stats(20)"` | `tracemalloc` snapshots around the suspect; `memray run <entry>` then `memray flamegraph`; `python -X importtime` for slow startup | `python -m timeit -s '<setup>' '<stmt>'`; `pytest --benchmark-only` | cProfile is tracing: call-heavy code looks worse than it is; same interpreter as production, no coverage plugin |
| Node / TS | `node --cpu-prof <entry>` then open the `.cpuprofile` in devtools; `node --prof x.js && node --prof-process isolate*.log \| head -80` for text | `node --heap-prof`; `process.memoryUsage().rss` at the peak; `--expose-gc` and `global.gc()` before reading heap | `performance.now()` around N iterations with the result consumed; `hyperfine 'node x.js'` | run compiled JS with `NODE_ENV=production`, not a TS loader; discard JIT warm-up; `console.time` around awaits measures the event loop |
| Go | `go test -bench '<Name>' -benchmem -count 10 -cpuprofile cpu.out ./pkg` then `go tool pprof -top cpu.out`; servers: `net/http/pprof` and `go tool pprof -top 'http://<host>/debug/pprof/profile?seconds=30'` | `-memprofile mem.out` then `pprof -sample_index=alloc_objects`; `GODEBUG=gctrace=1` | `go test -bench` before and after into files, then `benchstat old.txt new.txt` | compiler removes unused results: assign to a package-level sink; `-count 1` is one run; no `-race` while measuring |
| Rust | `cargo flamegraph --root -- <args>` (needs `perf` or `dtrace`); `samply record ./target/release/x`; `perf record -g` | `heaptrack ./target/release/x`; `valgrind --tool=dhat` | `criterion` with `std::hint::black_box(result)`; `cargo bench` | `cargo build --release` plus `[profile.release] debug = true` for symbols; `cargo run` without `--release` is a debug build, discard the number |
| JVM | JFR: `java -XX:StartFlightRecording=duration=60s,filename=r.jfr -jar app.jar` then `jfr print --events jdk.ExecutionSample r.jfr`; `asprof -d 30 -f p.html <pid>` (`-e wall` for off-CPU) | `jcmd <pid> GC.class_histogram \| head -30`; `-Xlog:gc*`; `jcmd <pid> GC.heap_dump h.hprof` for a leak | JMH with `Blackhole`, `@Warmup`, `@Fork(1)`; never a bare `System.nanoTime` loop | JIT warm-up is seconds, not iterations; GC pauses show as latency spikes, read GC logs first; same JVM flags as production |
| .NET | `dotnet-trace collect -- dotnet app.dll` (or `-p <pid>`); `dotnet-counters monitor -p <pid>` | `dotnet-gcdump collect -p <pid>`; `dotnet-counters` GC heap size | BenchmarkDotNet (`[Benchmark]`, `[MemoryDiagnoser]`) | `dotnet build -c Release`; run the DLL, not `dotnet run` on Debug; tiered JIT needs warm-up |
| C / C++ | `perf record -g ./x; perf report --no-children`; macOS `xctrace`; exact counts (slow): `valgrind --tool=callgrind` then `callgrind_annotate` | `valgrind --tool=massif; ms_print massif.out.*`; `heaptrack ./x`; leaks once in a separate `-fsanitize=address` build | Google Benchmark with `benchmark::DoNotOptimize(result)` | `-O2 -g -DNDEBUG`, never `-O0`, no sanitizers in the timed build; `-fno-omit-frame-pointer` for stacks |
| Swift | `xctrace record --template 'Time Profiler' --launch -- .build/release/x`; `sample <pid>` | `xctrace record --template 'Allocations'`; `leaks <pid>` | XCTest `measure { }` with `XCTClockMetric` / `XCTMemoryMetric` | `swift build -c release`; debug builds are 10x slower |
| Ruby | `stackprof --mode cpu --out s.dump -- ruby x.rb; stackprof s.dump --text`; `rbspy record -- ruby x.rb` | `memory_profiler` gem; `GC.stat[:total_allocated_objects]` before and after | `benchmark-ips` (`x.compare!`) | `RAILS_ENV=production`; `--yjit` only if production uses it; GC dominates allocation-heavy code |
| PHP | `php -d xdebug.mode=profile -d xdebug.output_dir=. x.php` (cachegrind file, read with `callgrind_annotate`) | `memory_get_peak_usage(true)` at the end of the request | `hyperfine 'php x.php'`; `phpbench` | opcache on, `xdebug.mode=off` unless profiling that run; OPcache is off in CLI by default |
| SQL | `EXPLAIN (ANALYZE, BUFFERS) <query>` (Postgres), `EXPLAIN ANALYZE` (MySQL 8) on production-sized data; find the query via `pg_stat_statements` ordered by `total_exec_time` or the slow query log | `Sort Method: external` and temp buffers in the plan mean `work_mem` spills | `\timing on` in psql, 5 runs, median | estimate off from actual rows by 10x means stale statistics: `ANALYZE <table>` before blaming the index; first run is the cold cache |
| Browser | `lighthouse <url> --only-categories=performance --output=json --quiet`; Performance panel recording; `performance.mark()`/`measure()` around the path | DevTools Memory: heap snapshot, allocation timeline | - | production build, cache disabled, CPU throttling on (4x); a dev-server bundle is not production size |
| Shell / process | `strace -c -f <cmd>` (Linux) or `dtruss -c <cmd>` (macOS) for syscall counts; `iostat`, `vmstat 1` during the run | `ps -o rss= -p <pid>` sampled in a loop | - | a process waiting on I/O shows low CPU; profile the wait, not the code |

## HTTP service (latency, throughput)

- Warm the process and its pools before the measured window; run the load generator on
  another machine when the numbers matter, and say where it ran.
- Load: `oha -z 30s -q 50 <url>`, `hey -z 30s -c 20 <url>`, `wrk -t4 -c50 -d30s <url>`,
  or `k6 run script.js` when the request mix matters. Report p50/p95/p99, requests per
  second, error rate, and the concurrency: p95 at 20 concurrent and at 200 concurrent
  are different metrics. A p99 from 50 requests is noise.
- Profile the server process with its language row while the load runs; watch RSS over
  the whole window (monotonic growth under steady load is a leak, not a working set).

## Reading a profile

- Sampling (perf, py-spy, JFR, async-profiler, samply, xctrace, rbspy) is cheap and
  truthful about wall time; under about 1,000 samples the tail is noise, profile longer
  or with a larger input. Instrumenting (cProfile, callgrind, xdebug, stackprof object
  mode) is exact on counts and inflates small hot functions. "Where does time go":
  sample. "How many calls": instrument.
- Inclusive (cumulative) time first: which subtree to enter. Self time second: which
  line. Call count third: whether the fix is "call less" or "make cheaper".
- A hot frame in the runtime or libc (malloc, memcpy, GC, regex, JSON, string
  formatting) is a symptom; the culprit is its caller in project code.
- Flat profile, no frame above 5-10%: the win is fewer items, fewer passes, fewer round
  trips, or the input is too small. Grow the input before concluding "nothing to fix".
- Wall time far above CPU time: I/O, locks, network, sleeps. Off-CPU views (`perf sched`,
  `py-spy --idle`, async-profiler `-e wall`) or count the calls at the boundary.
- Memory: peak is a working-set problem; steady growth under constant load is a leak.
  Allocation count drives GC time more than bytes do.

## Comparing two runs

- Same command, same input file, same machine, same build flags, back to back, with
  nothing else running. Alternate A B A B when the floor is close to the gain.
- `hyperfine 'A' 'B'` prints the ratio with its uncertainty; `benchstat` does the same
  for Go; `criterion` keeps the previous run for Rust. Trust a ratio only when its
  uncertainty band excludes 1.0.
- Report median and range (or p50/p95), never the mean alone.
