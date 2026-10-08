# Shared Rust core: first milestone

Historical first milestone: **working native/WASM foundation; S1 is not passed**. 2026-09-28.

The follow-up implementation and current limits are in [NATIVE-OWNER.md](NATIVE-OWNER.md);
its current status table summarizes every gate (latency, publication correctness,
S3 CLI routing, S4 counters; S2 IME still open).
The results and unimplemented items below describe the first milestone at the time it ran.

## Question and method

Can one hitSlop semantic implementation run behind coarse Swift/UniFFI and
Bun/WASM calls, preserving basic intent, atomicity and byte replay behavior?

Built a separate Rust workspace pinned to Loro 1.16.2, UniFFI 0.31.1,
wasm-bindgen 0.2.127 and Rust 1.96.1. It implements the checklist descriptor subset
without authored code, a JSON mirror or a production engine change. The SwiftPM
consumer links the generated native XCFramework and imports Foundation only.

Independent literal fixtures adapt the observable identity/atomicity obligations
from `packages/document/tests/handles.test.ts`, plus Unicode outcomes. Swift and
Bun each check those outcomes at their actual binding boundary. Neither derives
expected values from the other implementation's output. Rust additionally tests
malformed imports, one anomalous scalar case, independent-replica convergence and
host-minted ID persistence. These tests do not replace current conformance coverage.

## Results

- Native static/dynamic libraries and the WASM module build successfully.
- SwiftPM consumes the generated XCFramework under Swift 6 language mode.
- **12 scenarios pass through Swift and 12 through Bun/WASM**, including inserted
  row references within a batch, move identity, late rejection, duplicate IDs,
  UTF-16 emoji edits, surrogate-boundary rejection and explicit stale-base refusal.
- Accepted patches reconstruct the expected state and carry the current issues.
- Both bindings reopen checkpoints and checkpoint-plus-update sequences.
- Real **native → WASM and WASM → native byte replay passes** against literal values.
- **Six Rust owner tests pass.** A malformed import leaves the owner unchanged;
  an imported invalid boolean is preserved and flagged across reopen; reading does
  not change its version; separate replicas converge; minted IDs survive reopening.
- Atomicity sensitivity passes: bypassing staging in a disposable source copy makes
  `atomic_rejection` fail because an earlier edit and removal changed live state
  and version before a later operation rejected. Original source remains unchanged.
- Experimental TypeBox/Rust generation freshness and strict TypeScript checks pass.
- Repository `bun run check` passes. `bun run test` passes **154 tests**, **115
  compatibility replay cases** and **52 bundled-template open/reopen checks**.
  Production native/release suites were not rerun; no production native source,
  runtime or build integration changed.

The first binding run produced a 3,526,285-byte WASM file and a 5,300,800-byte Swift
executable. These are uncompressed standalone artifact sizes, **not app-size deltas**
or performance gates. There is no five-process latency matrix yet. Reproduction
commands are in [README.md](README.md); local hashes/results and the intentional
sensitivity failure log are in `.hitslop/v1-evidence/hitslop-core/`.

The first Swift build required access to normal compiler caches outside the command
sandbox. Re-running with that access completed. No production app signing or release
verification is claimed.

## Limits and next gate

Continue the spike. The coarse binding/toolchain is feasible on this Mac; the
experiment has **not** established full semantic parity or a performance advantage.

1. Replace top-level list invalidations and linear ID scans with identity-aware
   incremental patches/indexes. Expand patch/issue equivalence to randomized local
   and remote changes before putting this candidate into the placement matrix.
2. Implement the pending text draft/parent ancestry contract. Current-base splices
   reject stale bases; they do not solve the archived `abcXYZXYX` failure. Prove
   remote edits, selection and IME separately. No text-binding correctness claim.
3. Port descriptor coverage and exact preserve-and-flag/derived-ID behavior from
   existing fixtures. Current issue vocabulary and raw anomalous projections are
   preliminary, not contract-3 parity. Missing dependencies reject; durable pending
   import buffering is absent. Counter policy remains a separate investigation.
4. Add the owner lifecycle adapter, SQLite, writer forwarding/races, native save
   failures, renderer remount and process-death tests. The current headless runner
   is a binding consumer, not the production native document CLI.
5. Add the `rust-core` placement candidate, matching views/workloads/periodic saves,
   and run the revised comparison gates with per-process tails. Establish bounded
   import allocation and fuzz/panic behavior, signing and reproducible CI builds.

JSON/byte input sizes are capped, but decompressed memory, full-history growth and
durable capacity enforcement are not proved. Native unwind containment cannot catch
OOM/abort, and WASM trapping differs from native unwinding. Request deduplication,
automatic retries, durable receipts and patch gap recovery are not implemented.

## Effect on the plan

Retain the shared Rust semantic core as the candidate. Keep staged atomicity and
the existing native control. Do not expand this milestone into template removal,
ABI cutover, storage-format changes, or a compatibility reset. S1 and the production
decision stay open until the remaining gates above have evidence.
