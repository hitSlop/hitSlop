# Runtime reset (September 2026)

> Historical record, not a contract or task guide. Current rules live in the
> [engineering contract](../engineering-contract.md), [versioning](../versioning.md)
> and the [runtime reference](../reference/runtime.md).

hitSlop's runtime was reset twice before launch.

The **first reset** moved boot and lifecycle out of each slop and into the host. The
runtime now owns the page and mounts `assets/app.js` through `default.mount(ctx, target)`.
Apps reach it only through the `ctx` ABI. Rows carry application `$id` registers, and
merged anomalies are preserved and flagged instead of repaired. That produced
contract 2 (Mac 1.1.x, npm 2.0.0).

The **second reset** kept those boundaries and removed what still cost too much. Every
save exported a full snapshot just to measure it against capacity. Several runtime
archives were installed. The public SDK depended on Loro, and builds compared the
entire engine identity. The reset produced contract 3, SQLite format 2,
checkpoint-plus-update capacity and a durable minimum reader revision. Contracts 1
and 2, their fixtures and mixed-version collaboration were retired without
migration; packages from Mac ≤1.1.1 and npm ≤2.0.0 are refused. Git retains the
earlier design notes.

## Decisions and success criteria

The maintainer authorized a clean prelaunch break: assume no existing slops in the
wild need support, and remove obsolete compatibility handling instead of building a
migration path. Published npm versions, release tags and remote release assets were
not overwritten; the new baseline uses new identities and versions.

| Area | Decision |
|---|---|
| Installed engine | One host-supplied runtime; no bundled legacy engines |
| Launch identity | Runtime contract 3, revision 1; SDK/schema/CLI 3.0.0 |
| Document format label | Keep `hitslop-v1`; the runtime contract identifies this reset |
| Storage | SQLite format 2, with no migration from prelaunch formats |
| Postlaunch upgrades | New apps open existing documents; older apps may refuse newer writes |
| Slop updates | Ship new templates; do not update app code or schemas inside existing documents |
| Capacity | Limit stored checkpoint plus update payloads, not the hypothetical full snapshot |
| Authoring | Typed handles and immutable reads, with ordinary Svelte binding conveniences |
| Optimization | Prefer the simplest correct design; measure before adding specialized machinery |

Success meant:

- A routine autosave transfers and persists incremental updates without exporting
  the entire document.
- An acknowledged save survives process death and reopening. Checkpoint maintenance
  never destroys the last durable representation. Power-loss durability is claimed
  only after verifying macOS SQLite sync behavior.
- Runtime loading fails promptly and visibly when resources are missing, invalid,
  or incompatible, and failed startup does not leak ownership or WebViews.
- A compatible Loro/runtime update does not require rebuilding existing slops.
- Authors can build Svelte slops without depending on the engine implementation,
  private host bridge, or a specific Loro version.
- Release checks prove forward compatibility from the new launch baseline without
  shipping historical runtimes to users.

Two correctness decisions are worth their reasons. Counter-changing saves always
checkpoint: Loro 1.16.1 can replay batched counter deltas to a different value
(`1e16, -1e16, +1` reopened as 0; safe-integer deltas can also drift), and
fractional counters stay supported. The mutable-proxy authoring style (`count++`,
array `splice`) was rejected because it would need a second mutation model to keep
CRDT intent and row identity; Svelte function bindings over typed handles cover the
convenience instead.

## Open follow-ups

- **Fork-based `change()` staging.** O(document) per transaction (about 30 ms at
  40,000 rows), and it hides local operations from a Loro `UndoManager`. Replace it
  before undo; the observable `change()` contract (synchronous, all-or-nothing,
  pre-change reads) does not depend on it.
- **Power-cut durability.** SQLite runs with `synchronous=EXTRA` and `fullfsync=ON`;
  process-kill tests do not simulate physical power loss.
- **Shared WebKit data store.** Prewarm and each document use separate
  non-persistent stores, so the WASM compile may not be reused. Measure a shared
  store only with explicit same-origin (`slop://app`) storage isolation checks.
- **Release acceptance.** Offline installed-app acceptance, signed/notarized
  validation and publication follow [releasing](../guides/releasing.md).

Do not replace base64 transport, change SQLite journaling, pool document WebViews
or add another cache layer without a demonstrated bottleneck and its own
correctness review.

## Implementation evidence

The launch candidate uses contract 3 / runtime revision 1 / storage revision 1,
SQLite format 2, and SDK/schema/CLI 3.0.0. Installed consumers contain one engine.
The SDK tarball contains app-facing helpers and types; engine sources and the
Loro dependency remain internal to the host build.

A preserved pre-reset runtime and the candidate were exercised with 32
single-character text edits and explicit saves against in-memory storage. The
numbers isolate engine work and exclude WebKit, bridge transport, SQLite sync and
disk latency; the large strings were compressible, so this is not a near-capacity
workload.

| Initial text | Snapshot exports, before → after | Median save, before → after | p95 save, before → after |
|---|---|---|---|
| 1 MiB | 32 → 0 | 0.99 → 0.36 ms | 3.22 → 2.15 ms |
| 8 MiB | 32 → 0 | 6.36 → 2.20 ms | 15.70 → 3.23 ms |

Raw evidence: `.hitslop/v1-evidence/runtime-reset-v2/performance.json`. These
support the structural snapshot claim, not an end-to-end latency guarantee.

The ordinary-counter regression failed before the fix (expected 1, reopened 0).
The native missing-entry regression also failed before cleanup (retained WebView
and busy writer lease). Missing entry JavaScript, missing imported JavaScript,
missing WASM and corrupt WASM now report promptly and release failed-open resources
in both visible and headless sessions.

The native window diagnostic completed all six cells (100/1,000 fully rendered
rows × 1/10/20 windows), each with 100 durable title edits and 1,000 state reads.
Native durable-edit p95 ranged from 40–92 ms at 100 rows and 188–311 ms at 1,000
rows. Host plus identified WebContent footprint ranged from 174 MiB (one 100-row
window) to 6,280 MiB (twenty 1,000-row windows); the host alone was 36–47 MiB after
closing. This is a warm-machine stress diagnostic, not a controlled comparison.
Raw results: `.hitslop/v1-evidence/runtime-reset-v2/native-windows.json`.

First visible Quick Checklist opens measured 539 ms without explicit prewarm and
500 ms with it; later opens ranged from 249–500 ms. The sample is small and does not
prove a reliable prewarm advantage. Evidence:
`.hitslop/v1-evidence/runtime-reset-v2/startup.json`.

### Final validation

`bun run release:check` passed all 18 stages on the uncommitted working tree based
on `35bdf0ca9ea00d7562c8906cf3776b3a7806b1e8`: source/generated checks, 145 Bun
tests, the Swift suite, eight native CLI tests, 106 native render/reopen packages,
storage and host-process crash recovery, installed npm consumers, landing-site
checks, and the packaged Debug Mac app with matching runtime bytes. Both 57-case
compatibility replays produced identical results. The runtime seal is contract
3/revision 1, SHA-256
`48cb39e98f7d066f2a74f4df9dc91c6fe152b5ed168c2551aa8e9b851c9e37d7`. All 51
bundled templates have launch specimens.

The first gate attempt correctly rejected 14 stale contract-2 builds under example
`dist` directories. They were preserved in
`.hitslop/runtime-reset-v2-prelaunch-builds/`; no writable documents were migrated
or removed. Gate reports are retained in `.hitslop/v1-evidence/runtime-reset-v2/`.
This was local release-candidate validation, not publication.

### Counter follow-up

The storage owner covers safe-integer replay and a required counter checkpoint that
exceeds capacity after successful incremental text saves: exact reopen values,
retained live edits, unchanged durable bytes, failed close, and successful discard.
Removing only the counter checkpoint guard in a disposable copy made both cases
fail. Evidence: `.hitslop/v1-evidence/runtime-reset-v2/counter-followup-{sensitivity,check,test}.log`.

### Bridge transport spike (binary vs. base64)

A proposal to replace base64-JSON bridge transport with raw binary was spiked in
an isolated WebKit harness; a companion proposal for a native `LoroDoc` mirror was
rejected untested as a second document engine. On this WebKit, a `Uint8Array` sent
through `postMessage` arrives as an index-keyed dictionary, an `ArrayBuffer` arrives
empty, and raw `Data` replies fail to serialize. Only `fetch()` against a
`WKURLSchemeHandler` transfers bytes exactly, which would be a much larger change.

With native `Uint8Array.toBase64`/`fromBase64`, binary would save about 2.5 ms on
writes and 8 ms on reads at 8 MiB, against tens of milliseconds end to end. The real
cost was the pre-Safari-18.2 `atob` fallback (46–84 ms at 8 MiB), so the deployment
target was raised to macOS 15.2 instead. No bridge transport changed. Evidence:
`.hitslop/v1-evidence/binary-transport-spike/*.json`.

A single 9.5 s edit+flush on a 1,000-row document did not reproduce: repeated
headless edits took 3–7 ms and visible edits 156–194 ms, consistent with the
window diagnostic above. Evidence:
`.hitslop/v1-evidence/large-list-edit-timing/{headless,visible}.json`.

## Approved third reset: native Rust ownership (in progress)

The maintainer approved runtime contract 4 / ABI 2, without contract-3 migration,
and moving every active slop except Quick Checklist into `archive/slops/`. Source
provenance is retained there; all sealed contract-3 artifacts and release records
remain immutable. Archived slops are restored as supported descriptors return.

The promoted Rust core is shared by native UniFFI and browser/test WASM bindings.
Native ownership uses SQLite format 2, storage reader revision 2. The new owner's
admission gate refuses old contracts before creating state. The production runtime
selector now selects contract 4. ABI-2 renderer/SDK and live/closed native CLI routes are active in development. Signing, release packaging and the installed trial remain pending; this is not a release record.
