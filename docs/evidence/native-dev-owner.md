# Native owner for `slop dev`

2026-10-06. Isolated spike on `spike/native-dev-owner`, based on `6557240e`.
The main checkout is unchanged. This is an implementation and comparison of a native
development preview, not the OPFS browser host in `plans/browser-host.md` or the
app-definition/package-format migration.

## Run it

From this worktree, after `bun run verify` builds the engine and page shell:

```sh
bun packages/hitslop/src/cli/cli.ts dev examples/slops/quick-checklist --native-owner
```

Omit `--native-owner` for the existing disposable WASM preview. Both retain Vite
component and stylesheet HMR. A refresh, definition change, or command change resets
the native preview. Each browser page owns a different temporary document.

## What runs

```text
Vite page + authored UI
  │ existing Vite HMR WebSocket, authenticated preview frames
  ▼
Bun development host (transport and process lifetime only)
  │ bounded newline JSON over stdin/stdout
  ▼
slop-engine --client-protocol N --preview-owner TEMP.slop
  ├─ the real Rust Owner, writer lock and persistence worker
  ├─ command::page for edits, undo/redo, flush and attachments
  └─ commands.run → existing restricted QuickJS child → intents → Owner
```

Metadata evaluation stages and packs a real template at startup and on definition or
command changes. The host creates a fresh document from that template for each page.
Vite still serves live UI modules; the packed UI exists to satisfy current full-file
acceptance, not as a second UI to execute. Native preview pages do not load WASM.

The existing CLI command coordinator is shared, with the same host time/random seed,
one stale-base retry, and no retry of an unknown outcome. Native preview commands use
the page edit origin so undo treats the command as one step. The callable drains
pending input and waits for its accepted publication before resolving. Each evaluation
gets a new restricted process and runtime; module globals cannot survive a call.

This deliberately uses today's package shape and TypeBox generation. The new preview
frame is generated from `src/schema/preview.ts`. No compatibility marker changes.
There are no Swift, UniFFI, SQLite-layout, Loro, or package-format changes.

## Lifetime and failure behavior

- Only the parent chooses a document path. Page requests cannot supply one. Vite
  checks the connection origin and a random per-server token before opening a session.
- Definition invalidation fences all old sessions before rebuilding. A failed build
  cannot leave an editable preview using stale commands.
- EOF replaces the owner's view token, waits for bounded evaluations, closes through
  the owner, and saves. The parent waits for exit before deleting the temporary file.
  Shutdown has a final timeout/kill fallback for this disposable data.
- Publications use a bounded queue; overflow asks the page to resynchronize. Requests
  and responses have size limits, with at most 64 pending transport requests and eight
  concurrent evaluator requests per process.
- A dead owner or transport failure shows a visible error and rejects pending calls.
  Editing requires an explicit reload; no command is silently replayed.
- Templates from earlier rebuilds and session files are removed. Writer-registry lock
  files are never removed or bypassed.

## Evidence

The browser tests in `packages/hitslop/tests/cli/dev.native.test.ts` compare HMR on both
hosts: accepted edits and row identity survive UI edits; broken UI and metadata recover;
definition changes reset; native pages fetch no WASM. The native-owner test covers
drain-before-command, a command that refuses page execution, fresh evaluator globals,
undo/redo, attachment import/read/flush, independent tabs, bounded failed execution,
command rebuilds, refresh during evaluation, owner death, and child/file cleanup.

Rust engine tests exercise the process boundary directly: the preview holds the real
writer lock, EOF saves before reopening, path-bearing frames are refused, and a wrong
client protocol is refused before trying to open a document.

Final verification:

- `bun run verify` passed (90.1 s on the final run). All 221 enabled Rust tests passed,
  with four existing skips; all 148 Bun tests passed. Hygiene, types and packed-package
  checks passed (four packed cases passed, one skipped). Contracts and landing had
  already passed and were unchanged, so the final run reused their verification stamps.
- `bun test packages/hitslop/tests/cli/dev.native.test.ts
  packages/hitslop/tests/cli/native-owner.native.test.ts
  packages/hitslop/tests/cli/commands.native.test.ts` passed: eight tests, 61 assertions.
  This includes both HMR hosts and the combined native command/lifecycle case.
- Rust process checks include attachment bytes surviving close and reopen. Formatting,
  clippy and generated-contract drift checks passed through the verification runner.
- The complete Swift/native-app tiers were not run: no Swift, FFI or rendering-helper
  implementation changed. The browser integration tests above use real WebKit and the
  rebuilt Rust engine.

Linux verification is deferred at the user's request. Docker Desktop could not start
on this machine: its log reports a backend settings/bootstrap failure and exit 9.
That is an environment failure, not evidence for or against Linux support.

## Measured comparison

Apple M1, 16 GiB RAM, macOS 26.6.2, Bun 1.4.2, release Rust engine and Playwright
WebKit. No build or test suite ran beside the benchmark. These are local samples,
not a guarantee across hardware. The raw samples are in
[`native-dev-owner-benchmark.json`](native-dev-owner-benchmark.json).

| Fixture / owner | Command p50 / p95 | Component HMR p50 / p95 | Definition reset p50 / p95 |
| --- | ---: | ---: | ---: |
| Quick Checklist / WASM | 6 / 9 ms | 101 / 185 ms | 341 / 349 ms |
| Quick Checklist / native | 41 / 45 ms | 82 / 184 ms | 1,727 / 1,845 ms |
| PNG skin / WASM | 4 / 9 ms | 30 / 182 ms | 265 / 318 ms |
| PNG skin / native | 44 / 47 ms | 89 / 188 ms | 798 / 1,756 ms |

Twenty sequential awaited command calls per case; five HMR edits and five definition
resets. The command p95 includes the first call. Native first calls were 45 and 48 ms.
This clears the proposed 50 ms p95 gate on this Mac, with modest headroom. There is no
evidence here requiring a warm evaluator process; keep a fresh runtime either way.

Server start plus first page mount was 1.52 s WASM versus 1.84 s native for Quick
Checklist, and 0.46 s versus 0.87 s for the skin fixture. Those are single startup
observations, not percentile measurements. The native owners used 11,328 and 10,928
KiB RSS after the command samples. This is additional owner memory, not total browser
or process-tree memory. One owner process stays alive per page; one evaluator child
exists only while its command runs.

The material cost is definition rebuilding: this spike compiles and packs an entire
current-format template, even though Vite serves the live UI. That cost should be
addressed by the agreed app-definition/build work, not by introducing another resolver
or another validation path. Component/CSS HMR does not need that rebuild.

## Decision

Recommend adopting the native-owner direction for `slop dev`. It exercises the real
writer, save policy, attachments and restricted commands during authoring, while
preserving Vite HMR. The observed command cost fits the target and the persistent
process cost is small enough for ordinary development.

This is enough evidence to choose the architecture, not to merge the spike unchanged.
Before making it the default: finish the command-stub/owner integration alongside the
packaging design, remove the provisional callback argument, and run the deferred Linux
verification. Keep the pure browser/OPFS host as a separate product decision. No frozen
format should be made to carry this development-only experiment.

## Limits of this spike

- It is development-only, on a loopback Vite server. It does not implement a durable
  browser copy, OPFS, browser downloads, or `slop open --browser`.
- The command coordinator is still in `slop-engine`; moving its policy into the core
  owner is a separate production change. No second evaluator was introduced.
- Authored command bodies still exist in the current UI bundle. The native preview
  routes calls to the runner and ignores the callback; this is not yet the planned
  build-time command-stub transform and must not freeze as a new public ABI.
- The temporary owner has no published live-discovery socket. Its real lock refuses a
  second CLI writer. Supporting CLI calls into a preview would require the normal
  discovery/route path, not opening the file beside the owner.
- The browser still reads attachments with today's byte transport. Attachment URLs
  belong to the packaging migration and were not smuggled into this experiment.
- The installed Mac client, full package redesign and browser-host plan remain separate.

## Reproduce the comparison

```sh
bun scripts/dev/bench-native-dev.ts
```

This runs Quick Checklist and a 320×320 PNG-skinned fixture on both hosts. It records
listen/startup time, first mount, 20 awaited command calls, five component HMR edits,
five definition resets, and native-owner resident memory. The output defaults to
`/private/tmp/native-dev-benchmark.json`. Run without a concurrent build/test workload.
Command timing includes acceptance and snapshot publication, not just evaluator time.
