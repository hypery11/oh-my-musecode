# Concurrency: per-language telltales, detectors, stack dumps, atomics

Companion to omm-concurrency steps 1, 3 and 4. Run everything with `bash`; find the
telltales with `search` (literal mode, `glob`-scoped to the language's files).

## 1. Telltales for shared mutable state (step 1)

| Language | Actors appear as | Shared state appears as |
|---|---|---|
| Go | `go func`, `go f(`, `errgroup`, `time.AfterFunc` | package-level `var`, a `map` written from a goroutine, struct fields behind a pointer handed to `go`, `sync.Mutex` fields (the lock names the state) |
| Rust | `thread::spawn`, `tokio::spawn`, `rayon`, `.await` | `static mut`, `Arc<Mutex<_>>`, `Arc<RwLock<_>>`, `RefCell`/`Cell` reached across an `await`, `unsafe impl Send/Sync` |
| Python | `threading.Thread`, `ThreadPoolExecutor`, `asyncio.create_task`, `gather`, `multiprocessing`, gunicorn/celery workers | module globals, class attributes, `lru_cache` on a method, mutable default args, `os.environ`, `os.chdir` |
| JS / TS | `Promise.all`, `setTimeout`, `worker_threads`, `postMessage`, request handlers | module-level `let`/`const` object, closure state read after an `await`, `process.env`, a `Map` cache |
| Java / Kotlin | `ExecutorService`, `CompletableFuture`, `@Async`, coroutines `launch` | `static` fields, `HashMap`/`ArrayList` shared, lazy singletons, `synchronized` (names the state) |
| C# | `Task.Run`, `async`/`await`, `Parallel.For` | `static` fields, `Dictionary` shared, `lock(obj)` (names the state) |
| C / C++ | `pthread_create`, `std::thread`, `std::async`, signal handlers | globals, `static` locals, anything reached from a signal handler |
| Any | second process, cron, queue consumer, retry, replica | file, temp path, DB row, cache key, lock file, port, cwd |

## 2. Race and deadlock detectors (step 3.1)

| Language | Command | Notes |
|---|---|---|
| Go | `go test -race ./...` (and `-count=20` for stress) | reports the two stacks and the variable; a clean run is not proof of absence, only of the interleavings that happened |
| Rust | `RUSTFLAGS="-Zsanitizer=thread" cargo +nightly test --target <triple>`; `loom` for model-checking a lock-free structure; `cargo miri test` for UB | `loom` needs the test written against `loom::sync`; worth it only for hand-rolled atomics |
| C / C++ | `-fsanitize=thread -g` on all objects, then run; `helgrind` (`valgrind --tool=helgrind`) | TSan and ASan do not combine in one build |
| Java | `jcstress` for a lock-free structure; `-XX:+UnlockDiagnosticVMOptions` no detector; run the test with `-Djdk.tracePinnedThreads` for virtual-thread pins | most Java races are proven by step 3.3 (injected delay), not by a detector |
| Python, JS | none | prove with stress (3.2) and injected delay (3.3); `python -X dev` and `PYTHONASYNCIODEBUG=1` flag un-awaited coroutines and slow callbacks |
| SQL | `SHOW ENGINE INNODB STATUS`, `pg_locks` joined to `pg_stat_activity` | a DB deadlock report names both statements; the fix is the same lock-order rule |

## 3. Stack dumps for a hang or deadlock (judgment calls)

| Runtime | Command |
|---|---|
| Go | `kill -QUIT <pid>` (dumps all goroutines to stderr), or `curl :6060/debug/pprof/goroutine?debug=2` when pprof is mounted |
| JVM | `jstack <pid>`; it prints "Found one Java-level deadlock" with both monitors |
| Python | `py-spy dump --pid <pid>`; `faulthandler.dump_traceback_later` in the test |
| Node | `kill -USR1 <pid>` then attach the inspector, or `node --inspect`; `--trace-warnings` for unhandled rejections |
| C / C++ / Rust | `gdb -p <pid>` then `thread apply all bt`; `lldb -p <pid>` then `bt all` |
| .NET | `dotnet-dump collect -p <pid>` then `clrstack -all` |

Scan the dump for two threads each blocked in a lock acquire; the frames above the
acquire name the lock each already holds. Compare acquisition order across those paths.

## 4. Atomics and single-owner primitives (step 4.3)

| Need | Prefer |
|---|---|
| run once | `sync.Once` (Go), `std::sync::OnceLock` (Rust), `functools.cache` on a zero-arg function (Python, GIL-protected), a static initializer or `Lazy` (JVM, .NET) |
| get-or-create in a map | `LoadOrStore` (Go `sync.Map`), `entry().or_insert_with` under one lock (Rust), `dict.setdefault` (Python, atomic under the GIL for one call), `computeIfAbsent` (Java), `GetOrAdd` (C#) |
| counter | `atomic.Int64` (Go), `AtomicUsize` (Rust), `AtomicLong` (Java), `Interlocked` (C#); Python: a lock, `+=` is not atomic |
| flag | one atomic word or `Event`; two fields that must agree: one lock or an immutable struct swapped atomically |
| across processes or hosts | DB `UNIQUE` constraint + `INSERT ... ON CONFLICT DO NOTHING RETURNING`, `SELECT ... FOR UPDATE [SKIP LOCKED]`, `pg_advisory_xact_lock`, Redis `SET NX PX` with a token you check before `DEL`, a lock file with `O_EXCL`; never an in-process mutex |
| idempotency | a caller-supplied key stored with the result in the same transaction; a dedupe table with `UNIQUE(key)`; provider idempotency headers for payments and mail |
| bounded queue | fixed-capacity channel or queue with a stated full policy: block the producer, drop with a metric, or reject with an error; never `make(chan T)` unbounded growth via a goroutine per item |
| timeout that cancels | `context.WithTimeout` (Go), `tokio::time::timeout` (Rust), `asyncio.wait_for` (Python), `AbortController` (JS), `CancellationToken` (C#); a timeout on the wait alone leaves the work running |

## 5. Injected-delay hooks that already exist (step 3.3)

Before adding an env-var sleep, `search` the suite for a seam: `freezegun`/`time-machine`
(Python clocks), `tokio::time::pause` plus `advance` (Rust), `jest.useFakeTimers` (JS),
`testing/synctest` or a `clock` interface (Go), `Failpoint`/`gofail` (Go, TiDB style),
`ChaosMonkey` beans (Spring). Using the suite's own seam keeps the proof test committable.
