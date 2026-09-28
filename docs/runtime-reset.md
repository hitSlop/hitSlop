# Runtime reset (September 2026)

This records the pre-launch reset of the hitSlop runtime: why it happened, what
changed, why slops no longer have a `main.ts`, whether the SDK shape should change
before it is frozen, and what is left to do. The contracts themselves live in
[versioning](versioning.md) and [engineering contract](engineering-contract.md);
this document explains how we got there.

## Goal

After any hitSlop update, supported documents keep opening, editing and exporting
correctly, the runtime stays cheap to maintain, and nothing blocks real-time
collaboration with Loro later.

The previous design protected document *data* well: sealed runtime bytes, 104 replayed
fixtures, historical readers. What it protected badly was the *code* frozen inside
each slop.

## Why the old design was hard to keep compatible

Every built slop contained its own copy of the SDK's boot and lifecycle code
(`adapter.ts`, `view-lifecycle.ts`, `Slop.svelte`, the Svelte store). That frozen
copy:

- imported about 20 named exports from `/__runtime__/index.js`. Renaming or removing
  one would stop every older slop at module link time, with a blank window;
- called Swift bridge methods directly (`hostCall("config" | "ready" | "status" |
  "window.resize" | "runtimeRecovered")`, and a raw `webkit.messageHandlers`
  post in `Slop.svelte`). The private bridge was therefore frozen too;
- **defined `globalThis.__slop`**, which Swift calls to flush, prepare-close and close.
  Save-before-close ordering ran through per-slop frozen code;
- was split from runtime code by a filename allowlist in the build plugin
  (`hostedModules`). Nothing enforced that line.

The compatibility surface was therefore spread across runtime exports, the bridge,
the `__slop` protocol and a build heuristic. Much of the sealing machinery, and the
byte-preservation workarounds such as the `session-types.ts` aliases, the
`transaction` alias and the `observeText` alias, existed to police that spread.

The data layer also had gaps that would hurt collaboration:

- `importUpdates` **rejected** remote bytes whose merged state failed validation.
  Two valid peers could merge into an "invalid" state and then fork forever. This was
  reproduced: two peers each incrementing a counter by 1e308 could no longer merge.
- `change()` stages on a fork and imports the result. Loro reports those imports as
  remote, so **transactions emitted no local-update events**. That was fine for
  SQLite but wrong as a sync stream (also reproduced).
- Row `$id` was Loro's internal container ID, leaking engine details into apps,
  fixtures, CLI scripts and JSON import.
- There was no document identity, and no policy for history growth against the
  32 MiB storage limit.

Because the app had not launched and no documents from 1.0.x/npm 1.1.x needed to open,
we reset instead of layering compatibility on top.

## What changed

### 1. The app owns the page; slops supply one module

```
.slop (frozen)                                hitSlop.app (replaceable)
  assets/app.js ── default.mount(ctx, target) ─▶ runtimes/2/boot.js
  assets/app.css                                  opens document, builds ctx,
  state.schema.json, initial.json                 owns __slop, capture, errors
  state/… (data)                              ◀── private bridge ──▶ Swift
```

- Packages no longer contain `app.html`. Swift (`SchemeHandler`) and the CLI preview
  serve a fixed page that loads `/__runtime__/boot.js`.
- `boot.js` opens the document, builds `ctx`, imports `/assets/app.js` and calls
  `default.mount(ctx, target)`. It owns `__slop`, status, render errors, recovery,
  presentation and capture orchestration.
- `headless.js` reuses the same open path and never loads app code.
- Apps import **nothing** from `/__runtime__/`. The build rejects engine imports,
  `/__runtime__/*`, `webkit.messageHandlers`/`__slop`, and remote stylesheets, fonts
  or scripts needed at startup.
- The Swift↔JS bridge and `__slop` are now private to the app and change freely.

### 2. The compatibility promise is three contracts, each with a gate

| Contract | Definition | Gate |
|---|---|---|
| App module | `default.mount(ctx, target)`, asset URLs, theme variables, capture, package validation | Frozen consumer apps run in WebKit |
| `ctx` | [`abi.ts`](../packages/document/src/abi.ts): behavior, not just names; grows only | Frozen consumers `2-1` (hand-written JS) and `2-1-svelte` (real adapter) check it while mounting |
| Data | Descriptor format + frozen schema-key canonicalization, Loro layout, SQLite format 1 | Bun replay: exact expected state and `issues.json`, scenarios, historical readers, mixed-version collaboration |

Breaking `ctx.bind.text` in a disposable runtime makes both frozen consumers fail,
so the gate does its job. The key-name comparison the first plan proposed would not
have caught changes in behavior.

### 3. Data semantics ready for collaboration

- **Application `$id`**: a random 128-bit register written on insert. Projection never
  creates random IDs and never exposes container IDs. A valid stored ID belongs to the
  row with the lowest internal identity among those claiming it, so moving rows never
  transfers an ID. Rows with a missing, invalid or duplicate ID get a **derived ID**:
  `x-` plus a fixed hash of internal identity. It is identical on every peer and
  revision, never written, and pinned by a golden vector. Such rows render, stay
  addressable and editable, and are flagged. JSON import keeps supplied IDs, so an
  export → import round trip preserves references.
- **Preserve and flag**: any decodable state opens. Anomalies go to `issues`
  (`{path, kind, detail}`). Values that are representable keep their stored value; ones
  that are unusable read as a documented fallback, and edits beneath them are refused.
  A counter that merged past the finite range reads `null`, because no JSON number
  represents it; that needs overflowing merges and is flagged. Nothing is repaired on
  open. Incremental patching and full reads share one policy (records re-read through
  the same projection), checked by an invariant matrix.
  Undecodable bytes, missing dependencies and wrong schema keys still fail.
- **One outbound stream**: `onLocalUpdate` emits each accepted local commit exactly
  once, including transactions, CLI batches and JSON imports. Imported peer updates
  are persisted but never re-emitted. Subscribe events carry
  `origin: local | host | remote`.
- **Identities**: `$id` (row or tree node), `doc_id` (document, minted in SQLite and
  renewed by Duplicate) and the Loro peer ID (per session, never persisted) are kept
  separate.
- **Capacity (a safety net)**: edits update live state synchronously. Before saving a
  pending batch, the runtime measures its actual full snapshot against the 32 MiB
  limit. There are no per-edit estimates or reserved headroom. An oversized snapshot
  leaves work visible and unsaved, sets `save-failed` and `full`, and blocks closing
  and export. Editing and retry remain available; success clears the failure. The
  Mac app offers **Keep Open** and **Discard Unsaved Edits**. Explicit discard awaits
  reloading the latest durable state under the same writer lock, then remounts the
  view to clear drafts. Failed restoration keeps the unsaved work. CLI save failures
  are unknown outcomes, not proof an edit was rejected; never replay intent blindly.
- **History**: full history is kept. The workload in `scripts/v1/growth.ts` measured
  5,000 tasks of churn at about 2% of 32 MiB. That is a benchmark, not a bound on
  future history growth: either content or history can exhaust capacity. Compaction
  (`slop compact`, automatic at 256 updates or 4 MiB) folds the update log into a
  snapshot; it does not trim history. Readers accept shallow snapshots, but safe
  pruning still requires a sync-aware design for peer catch-up and shared history.
- **Engine placement**: Loro stays in the WebView. Authored UIs need a synchronous
  local replica, the schema layer exists once (in TypeScript), and loro-swift lags
  (1.13.x, experimental). The host is storage and transport. A schema-agnostic Swift
  peer can be added later for background sync; reads that never throw make that
  possible.

### 4. A clean baseline

- Contract 1, its 104 fixtures and the 1-1 byte-preservation rules are gone. The
  launch baseline is **contract 2 / revision 1**, SDK/CLI/schema **2.0.0**.
- The number 2 only avoids colliding with the old sealed 1/x records in git history.
  The history guard exempts contract 1 (`retiredContracts`).
- New fixtures are authored once by `scripts/v1/author-fixtures.ts`, which never
  overwrites a fixture:
  - `2-1`: every value kind, a checkpoint plus a log, a scenario and a collaboration
    script;
  - `2-1-saved-state`: theme, an attachment and several updates;
  - `2-1-issues`: merged anomalies with exact issues;
  - `2-1-svelte`: the frozen Svelte adapter.
- `release:check` now refuses to ship a bundled template without a sealed specimen
  (`check:sealed-templates`).

## Why every `main.ts` was deleted

All 51 examples had the same four lines:

```ts
import "./styles.css";
import { mountDocument } from "@hitslop/document/host";
import App from "./App.svelte";
await mountDocument(App);
```

`mountDocument` was the part that booted the runtime, talked to Swift and installed
`__slop`, and all of that is now the runtime's job. What remains is declaring which
component is the app. That is convention, so the build generates it:

```ts
import "./styles.css";
import App from "./App.svelte";
import { defineSlop } from "@hitslop/document/svelte";
export default defineSlop(App);
```

Deleting the file removes one piece of boilerplate that authors or agents could get
subtly wrong, such as a missing `await`, a wrong import or extra side effects. A
`main.ts` is still honored when present, for non-Svelte apps: it must
`export default { mount(ctx, target) }` (`SlopApp` from `@hitslop/document/abi`).
The test fixtures use this to prove the ABI works without Svelte.

Author imports did not change: `useDocument`, `<Slop>`, `bindText`, `bindValue`,
`capture` and `attachments` keep their names. They now forward to `ctx` instead of
importing runtime modules.

## Should we revisit the DSL / SDK shape?

**Yes, a few targeted changes, and before launch.** Everything reachable from
`ctx`, including the transaction scope passed to `change()` and every handle method,
becomes frozen for good once 2/1 is sealed, because the ABI can only grow. Pruning
is free now and impossible later.

Applied before sealing:

1. **The transaction scope is `{ fields, at }`.** The command-style methods
   (`tx.set(field, v)`, `tx.text(field).replace`, `tx.insert/remove/move/clear`) are gone
   from `change()` and `Document`. Templates used only `tx.fields` (103 uses) and
   `tx.at` (43). An internal executor still serves CLI batches, previews and JSON
   import.
2. **Anomalous rows keep `string` IDs honestly.** Derived IDs make every visible row
   keyed, addressable and editable, instead of `null` IDs that break list keys and
   `at()`.
3. **`bind.value` is generic** over the scalar value type, and **counters are typed `number | null`**
   (`null` only after merged increments overflow; Koi Pond and Side Quest handle it).
4. **Handle names stay as they are.** Renaming (`put`/`delete` versus `set`/`clear`,
   the two `move` shapes) would touch all 51 templates for little gain.

Worth keeping as is: descriptors as pure data (`defineDocument`/`s`), immutable
snapshots with stable identity, synchronous all-or-nothing `change`, `doc.at(snapshot)`,
previews, and Svelte as the only first-party adapter.

Not recommended now: loro-mirror style reconciliation, a Swift-side engine, schema
evolution, or a rich-text editor binding (`loro-prosemirror` would have to ship inside
the runtime; add it later as a `ctx` capability).

## Further work

**Before launch**

1. Seal: `bun scripts/v1/runtime-release.ts`, then `bun run fixtures:seal --write`, then
   the complete `release:check` ([releasing](guides/releasing.md)).
2. Issues are visible to agents in `slop get --snapshot`; the Mac app shows them nowhere
   yet. Consider a small indicator, which is not required for launch.

**Soon after**

- **Undo**: `change()` stages on a fork and imports it, which hides local operations
  from a Loro `UndoManager`. Replace the staging implementation behind the same
  `change` contract before adding an undo capability.
- **Bun-tier ABI runs**: the consumer self-tests run only in WebKit. A DOM shim such
  as happy-dom would give faster feedback in the everyday tier. WebKit stays the
  authority.
- **Runtime hardening**: bridge access is rejected at build time only. Hiding
  `webkit.messageHandlers` from app code at run time would be defense in depth, if
  WebKit allows it cleanly.
- **Large collections**: addressed edits use the projected snapshot for lookup,
  with a scan fallback for rows inserted earlier in the same transaction. Keep
  further indexing changes tied to measured workloads (see the performance check below).
- **Sync**: the outbound stream feeds only SQLite today. It emits at commit, before durability, and a capacity failure followed by Discard rolls those edits back, so a transport must forward only saved updates. Transport, enrollment
  (`doc_id` never authorizes sync on its own), durability and discard semantics, presence as an optional
  `ctx.capabilities` entry, and history pruning each need their own design. Pruning
  must stay readable by earlier revisions: readers already accept shallow
  checkpoints; writers do not produce them. Local discard does not retract operations
  already delivered to another peer; transport must account for that before it ships.
- **Clean-up**: local `generated/v1/runtime-releases/1-*` copies are unused now.

## Review round (27 September)

An independent review before sealing found, and this round fixed:

| Finding | Fix | Proof |
|---|---|---|
| Reaching capacity silently discarded a pending draft | Drafts commit before save-time measurement; oversized work stays visible, save and close fail, and explicit Discard restores durable state | Composition and discard regressions failed first; the storage and view lifecycle tests cover retained drafts and restoration |
| A record entry read differently before and after reopening | Records re-read through the full projection | Invariant matrix over nine anomaly positions; failed first |
| Null row IDs could break ordinary rendering | Derived stable IDs; ownership independent of position | Identity tests, golden vector |
| The native compatibility test edited `title` on every fixture | Conformance edits only on the conformance schema; templates get generic checks plus a render-never-writes equality | Passed with a temporary `codex-pet` specimen |
| The history guard missed `issues.json` and `collaboration.json` | Every file of a recorded fixture is protected | Extended history test |
| Collaboration exercised only alternating sync | Concurrent offline round plus an independent `expected` model | Fails when peers diverge and when both agree on a wrong result |

The initial capacity check forked on every edit. Its replacement used an estimate,
but review reproduced an oversized full snapshot passing that estimate and a failed
composition being erased by a status notification. The final implementation measures
the full snapshot at save time, with no per-edit capacity work or reserve. Discard
now restores the latest durable bytes rather than merely clearing previews.

## Performance check (27 September)

Codex's save-time measurement exports the full snapshot on every save. Measured cost is
small: about 5 ms at 1 MiB and about 50 ms at 24 MiB, on a 200 ms autosave debounce, so no
estimate gate was added. The same benchmark exposed a larger regression from the derived-ID
change: every addressed row edit rescanned the list's effective IDs (128 ms per edit at
40,000 rows). Resolution now looks the row up in the projected snapshot and verifies it
against the edited engine, falling back to the scan only for rows the snapshot does not know
(for example, inserted earlier in the same transaction). Row edits dropped to 2.5 ms, and
`change()` from 445 ms to 30 ms, at 40,000 rows. The rest of `change()` is fork-based
staging, which is also the thing to replace before undo. `bun scripts/v1/growth.ts` reports
these timings.

## Status (27 September)

The host-owned runtime, app-facing `ctx`, application IDs, preserve-and-flag reads,
save-time capacity checks with explicit Discard, and row-lookup changes have passed
the document and native owner tests. Current rebuild evidence includes:

- All 51 bundled templates declare contract 2/revision 1; `check:built` passes.
- All 55 bundled and conformance packages passed headless open, authored rendering,
  PNG/PDF export, and unchanged-state reopen in the full render sweep.
- The Debug app's host and helper runtime bytes match the candidate.
- The native/host crash matrix passed, including acknowledged-write recovery.
- CI's `tests/**` native filter covers both `tests/abi/**` and `tests/compatibility/**`.
- Refreshed npm 2.0.0 tarballs passed the outside-checkout native workflow with Node
  absent; the relocated helper passed editing and PNG/PDF export without Bun or a
  checkout runtime fallback.
- Type/Svelte checks and the public documentation check/build passed after the
  entry-point, package-layout and theme documentation cleanup.
- Browser preview edits reset on refresh. In the Debug app, edits survived close
  and reopen, Duplicate assigned a distinct `doc_id`, independent edits stayed
  separate, and both documents survived application quit and relaunch.
- Manual QA found Duplicate returning a non-directory URL, unlike Open/Recents.
  Returning the validated package URL fixes the resulting busy-writer error when
  focusing the live duplicate; the regression failed before the fix and passed
  afterward. The rebuilt app also passed the same Recents interaction.
- PNG and PDF exports were visually inspected. A suspected progress-bar mismatch
  was ruled out by pixel inspection: both PNGs show two-thirds completion, matching
  the PDF and saved state. The 17 core Swift tests and rebuilt app's packaged
  template/runtime/create/reopen/export checks passed; the everyday suite passed
  142 tests. Logs and exports are retained in `.hitslop/v1-evidence/launch-qa/`.

The capacity contract is decided: oversized work remains live and editable, saves
and close/export fail visibly, and Retry or explicit Discard provides recovery.
CLI capacity failures use `failed`, because an edit may already be accepted in memory.

Mac **1.1.1/build 28** shipped from commit
`2e940fcb8551536454669ce58da9ff80c622316e`, tagged
[`macos-v1.1.1`](https://github.com/hitSlop/hitslop/releases/tag/macos-v1.1.1).
The clean local release gate, CI, and tagged release gate passed. Downloaded ZIP and
DMG checksums, signatures, Gatekeeper acceptance, installed-helper editing/export,
and the Sparkle update from signed 1.0.8 to 1.1.1 passed. Evidence is retained in
`.hitslop/v1-evidence/release-1.1.1/` and the release workflow artifact.

Contract 2, revision 1 and all 51 template specimens are the immutable launch
baseline. The failed `macos-v1.1.0` tag remains unchanged and has no release assets.
The follow-up fixed template-seal matching to exclude regenerated artwork while
preserving complete historical specimen seals, and pinned the templates' icon
dependency so clean CI builds reproduce the sealed authored content.

The matching schema, document and CLI npm packages are published at **2.0.0**.
Each registry integrity hash matches its tested GitHub Release tarball, and all
three `latest` tags resolve to 2.0.0. Fresh registry consumers passed SDK/type
checks, init/install/check/dev/build/register, theme overrides, PNG/PDF export,
and persisted edits using the signed helper without Node. Default and versioned
`bunx` and an isolated global `slop` installation passed without SDK overrides.

Actual network-disconnected launch was **not run**: the maintainer could not
perform it. Bundled-resource and native-helper checks passed without changing
the machine's network settings; they do not substitute for that manual check.
