# Shared Rust semantic-core spike

An isolated start on [LoroHostPlan](../../docs/LoroHostPlan.md), not a replacement
runtime. Read [NATIVE-OWNER.md](NATIVE-OWNER.md) for the integration milestone and
[REPORT.md](REPORT.md) for the earlier binding milestone.
No production target, engine pin, template, package format or sealed artifact changes.

## Reproduce (Apple silicon Mac)

Prerequisites: root Bun dependencies, Xcode, Rust/rustup. `rust-toolchain.toml`
pins Rust 1.96.1 and the WASM target. Cargo.lock pins Loro 1.16.2 and UniFFI 0.31.1.
Bootstrap the matching WASM binding tool once, from the repository root:

```sh
cargo install wasm-bindgen-cli --version 0.2.127 --locked --root spikes/hitslop-core/dist/tools
```

Then:

```sh
bun spikes/hitslop-core/generate.ts --check
cargo test --manifest-path spikes/hitslop-core/Cargo.toml -p hitslop-core-spike --release --locked
bun spikes/hitslop-core/build.ts
bunx tsc -p spikes/hitslop-core/tsconfig.json
bun spikes/hitslop-core/run.ts
bun spikes/hitslop-core/sensitivity.ts
bun spikes/hitslop-core/renderer-sensitivity.ts
# S3: one CLI edit path (forward or own), races, SIGKILLs and cold start
swift build --package-path spikes/hitslop-core -c release
bun spikes/hitslop-core/cli-races.ts
```

`build.ts` generates Swift bindings, creates a host-architecture XCFramework,
builds a SwiftPM command-line consumer, and generates a web-target WASM module
that Bun initializes from bytes. It does not sign/publish an application. Swift
needs access to its normal compiler caches. All build output stays under this
spike's ignored `target/`, `dist/` and `.build/` directories.

`run.ts` checks the same literal scenarios independently through Swift/UniFFI and
Bun/WASM, then exchanges real checkpoint/update bytes in both directions. It writes
hashes, artifact sizes and completed checks to `.hitslop/v1-evidence/hitslop-core/`.
These are correctness results, not latency or full application-size measurements.

`sensitivity.ts` replaces fork staging with shared live mutation in a **disposable
copy** and requires the atomic rejection test to detect changed state/version. It
does not touch the source under `core/`. It uses its own target cache so deliberately broken objects cannot pollute normal
builds. Its failed-test log is evidence of successful sensitivity.
`renderer-sensitivity.ts` similarly removes the release-gap pump in a disposable
binding and proves the renderer regression test fails for stalled input.

After editing `contract.ts`, regenerate with `bun spikes/hitslop-core/generate.ts`.
TypeBox owns the experimental intent envelope; never hand-edit `wire.generated.rs`.
The production schema generator and contracts are unchanged.

## Implemented boundary

One Rust `Document` accepts descriptor JSON, initial JSON, typed intent batches and
Loro bytes. Swift and WASM adapters only marshal coarse calls. The subset is
`text`, `boolean`, `object`, `list(object)`; descriptors use the existing format-1
shape. Unsupported vocabulary rejects explicitly.

Operations: boolean `set`, ID-addressed `insert`/`remove`/`move`, and UTF-16 `splice`.
Insertion can mint a random 128-bit application ID or accept one for subsequent
operations in the same batch. Initial fixture rows can carry explicit IDs. `at`
accepts `{before:id}` or `{after:id}`; omission means end. This experimental wire
is not the production CLI API. Ordinary splices require the current base. The
separate `text` call retains an authored branch and parent chain for delayed drafts;
`command_current` explicitly interprets offsets at owner-queue execution and refuses
a supplied base. Within a batch, operations see earlier staged edits.

Batches apply directly to the owner's Loro document; there is no per-edit fork.
Each intent is fully validated before its first mutation. If a later intent rejects
after earlier ones mutated, the owner is rebuilt at the pre-batch frontiers with
`fork_at` (O(document), only on that rejection path) and a fresh peer; the pending
operations were never exported. Imports check dependencies from the blob header
before Loro buffers anything. Native panic containment invalidates an uncertain
owner rather than continuing with it. WASM traps still require discarding/recreating
the WASM instance.

Publications are built from Loro's synchronous `subscribe_root` events: each event
names its container and a typed diff. Map updates become field `set`s, text events a
`set` of that text, and movable-list deltas become `deleteRow`/`insertRow`/`moveRow`
against a persistent per-list index (order plus `$id` lookup) that the same events
keep current. Only changed fields and inserted rows are materialized; neither the
before nor the after document is. Containers created by the change are published
whole by the event that introduced them. Validation is incremental while the
document has no issues; a document with issues rescans on publish. Lists with
missing, invalid or duplicate row IDs fall back to an exact `set` of that list.
The renderer preserves unchanged objects and resynchronizes publication gaps.
Snapshots remain a full recomputation and serve as the tests' oracle.

Text drafts apply directly to the owner while the owner is still exactly at the
draft's last accepted version (the normal typing case). Only when other edits
intervene is the authored branch materialized with `fork_at` and merged.

Draft receipts deduplicate the latest request per active draft, including equivalent
JSON encodings. Released drafts are tombstoned; unknown/old requests never replay
silently. A session permits 64 active drafts and 4096 retired IDs, then requires a
flush/reopen. A new owner session requires reconciliation. These are experimental
bounds, not durable receipts or a production retry guarantee.

The conformance executable runs without WebKit or authored code. The separate
`rust-placement` executable mounts the shared Svelte view in WKWebView and persists
opaque bytes with the shared placement Store in fresh disposable directories.
Neither opens user `.slop` packages. Production locks and validation remain unchanged.
The two Swift packages link different native engines; the harness source is shared
through a source symlink. `spikes/placement-support` contains engine-independent
SQLite storage and JSON utilities.

## Native integration and measurement

From the repository root, with a normal command PATH:

```sh
bun spikes/engine-placement/prepare.ts
bun spikes/hitslop-core/build.ts
swift test --package-path spikes/hitslop-core -c release
swift test --package-path spikes/engine-placement -c release
bun test spikes/hitslop-core/renderer.test.ts
bun spikes/engine-placement/run.ts --rust --integration-only
bun spikes/engine-placement/run.ts --rust --matrix
```

The native runners open temporary windows. The matrix freezes both executables and
browser assets, runs the WebView correctness gate first, then rotates matched WASM,
Swift native and Rust native in separate processes. It records five trials for each
1k/5k × 1/5/10-window cell, plus one exploratory run per candidate at each
40k × 1/5/10-window stress cell. Failed cells retain their errors and count as
failures; they never disappear from the report. Resume an interrupted frozen run
with `--rust --matrix --resume <evidence-directory>`; executable and renderer hashes
are verified before continuing. Unindexed attempts are archived and their SQLite
working directories are recreated. `--redo <comma-separated-result-filenames>`
explicitly archives and repeats selected cells while preserving the frozen inputs. Do not run builds
or CPU-heavy tests concurrently with measurements. A system Japanese/Chinese IME
must be tested manually; synthetic composition and AppKit keyboard results do not
satisfy that gate. Evidence lives under `.hitslop/v1-evidence/engine-placement/`.

The manual gate still needs an interactive harness mode; the automated runner
exits after its scenarios. That follow-up must enable a Japanese or Chinese input
source and record composed text, caret, concurrent edit timing and flush/reopen
result. No system input settings are changed by these scripts.


Record the finished matrix with:

```sh
bun spikes/hitslop-core/summarize-native.ts <evidence-directory> --record
```

Per-operation native cost at 1k/5k/40k rows (the shape, not just the number, is the
gate: field and text edits must stay flat):

```sh
cargo run --manifest-path spikes/hitslop-core/Cargo.toml --release -p hitslop-core-spike --example cost_attribution -- spikes/engine-placement/dist
```

The native-only publication probe reads the same frozen fixture bytes and excludes
FFI/bridge/renderer cost. Its measured construction cost is a lower bound on the
complete publication budget:

```sh
cargo run --manifest-path spikes/hitslop-core/Cargo.toml --release -p hitslop-core-spike --example publication_cost -- <evidence-directory>/assets
```

The final integration evidence is checked in under `evidence/`; raw frozen binaries,
assets, SQLite stores and complete run records remain in `.hitslop/v1-evidence/`.
