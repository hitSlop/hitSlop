# Local Chrome browser hardening — 2026-10-08

Scope: correctness for the local `slop open --browser` beta. Hosted sharing,
Safari/mobile qualification, sustained-load measurements and launch UI polish remain
separate work. Package, runtime, storage and browser container markers remain at 1.

## Changes

- Validated imports receive a fresh document UUID in Rust. Reloading keeps that UUID;
  imported app/state/row identities/theme/attachments are preserved.
- Rust exports run the shared owner flush barrier and refuse failed saves before
  emitting pages. The host drains pending page drafts before requesting the export.
- Download opens Chrome's Save As picker during user activation, streams a unique
  temporary OPFS file to the chosen destination, and cleans it after completion or
  failure. Startup cleanup takes the copy lock before removing abandoned exports.
- Resource replies are capped at 1 MiB and Service Worker responses pull ranges with
  backpressure. Attachment verification hashes incrementally; verification state is
  retained per store and decoded app assets have a 32 MiB LRU cache.
- Rust validates browser request envelopes and generates both browser CSPs. Browser
  container format 1 is checked before opening or cleaning sahpool files. Unknown or
  unmarked pre-launch containers are refused without migration or deletion.
- Keep awake reports off, on or paused from actual lock state, including release and
  stale asynchronous grants.

## Regression evidence

Before the fixes, focused browser cases reproduced three failures: imported copies
retained the source UUID; direct Rust export omitted an accepted unsaved edit; and
Chrome's wake-lock release left the button showing on. Each passed after its fix.

The local host suite also checks bounded 10 MiB attachment streaming, digest and range
correctness, cancellation, Service Worker restart, Rust export refusal on an injected
OPFS write failure and recovery, Save As cancellation/write failure/repeated saves,
unknown-container refusal without file changes, and abandoned-export cleanup only
once the owning tab releases its lock. Tests use infrastructure fixtures.

## Hands-on Chrome check

Opened a fresh Hourglass copy through the CLI in desktop Chrome. Edited its title,
started the timer and toggled Keep awake on/off. Typed `Typed just before Save As`,
clicked Download .slop, and used the real macOS Save As dialog. The exported file was
placed at `/private/tmp/hitslop-hardening-download.slop`; the native engine reopened
it and returned that title with the timer state intact.

## Verification

- Focused Chrome host suite: 13 tests passed, 176 expectations. This includes the
  existing command, isolation, reload and compatibility round-trip cases.
- Generated-contract checks, TypeScript, SDK/shell (83 tests), CLI (96 tests) and all
  native/browser/evaluator Clippy configurations passed in the standard verifier.
- Standard Rust configuration: 335 passed, 5 skipped.
- Development-sync Rust configuration: 341 passed, 5 skipped. Nine slow launches
  completed; a sampled process was waiting in `_dyld_start`, before test code.
- Packed-package verification: 4 passed, 1 intentionally skipped.
- Swift integration: 208 tests passed across three shards.
- Full browser tier: 32 passed, 347 expectations, including all 13 local-host cases.
- Native CLI: 59 passed, 1 release-only host-death case skipped, 193 expectations.
  Compatibility replay, relocated helper PNG/PDF export and engine-crash recovery passed.
- `bun run verify` passed. `bun run verify --native` passed against the final sources.
  Raw runs: `.hitslop/evidence/runs/2026-10-09T01-59-47-944Z-81309` and
  `.hitslop/evidence/runs/2026-10-09T02-41-23-447Z-2517`.

## Limits

This pass does not establish near-limit import timing, a process-wide memory bound,
five-minute typing/save latency, cross-browser persistence or hosted origin viability.
The 32 MiB cache bounds decoded cached resources, not the entire WASM process. Large
attachment hashing still performs one full integrity scan on first access.

The optional interactive macOS fullscreen/continuous-typing session was not repeated
in this pass; existing Swift and native rendering coverage passed. No before/after
native save-latency benchmark was performed.
