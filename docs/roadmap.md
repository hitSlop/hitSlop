# Direction

Today hitSlop is a signed Mac app and matching npm authoring packages. People create local
documents from bundled or installed templates, edit them in the window or through the CLI
(their coding agent included), and export PNG and PDF.

## Where we're heading

AI made writing a small tool cheap. What's still missing is a place where those tools live,
keep their data and stay changeable. hitSlop is that place. Every slop gets merging
storage, saving and recovery, export and a Finder icon without its author doing anything,
and a person and their agent edit the same live document. The next steps make the climb
from using a slop to changing it gentle:

```text
use ──▶ tweak ──▶ ask ──▶ remix ──▶ author
 ✓        ✓       CLI only  missing    ✓
```

In order, with the reasoning in [ideas](ideas.md):

1. [Ask from the window](ideas.md#ask-from-the-window): the person's agent, launched
   from the toolbar, editing the open document.
2. [Attribution and "Undo that"](ideas.md#attribution-and-undo-that): every change says
   who made it. Edit ▸ Undo already reverts an agent's edits.
3. [`slop watch` and `slop mcp`](ideas.md#slop-watch-and-slop-mcp): agents follow edits and
   reach slops without a shell.
4. [Remix](ideas.md#remix) with [additive app upgrades](ideas.md#additive-app-upgrades).
   Both change the engineering contract and need a decision before work starts.

## Open now

- At launch, capture and freeze the first [compatibility corpus](testing.md#compatibility-corpus)
  entry; from then on every released document stays openable.
- Check Quick Look by hand on a document received by Mail and AirDrop. Quarantined copies,
  a file another process is writing and a crashed write (the document icon until the app
  recovers it) are checked.
- Decide whether Time Machine copying a whole document at the current attachment limits
  is acceptable, or lower the limits for launch.
- Confirm the platforms the CLI's engine ships for (Windows isn't planned).
- Check system IME composition and Edit ▸ Undo by hand (typing, ⌘Z inside a field, an
  agent edit between steps); no evidence file covers them.
- Shape Lab: the production-window shadow refresh, and the opening-only black strip, which
  was not reproduced and has no confirmed cause
  ([evidence](evidence/shape-lab-interaction-2026-09-30.md)).
- A long-lived memory study across windows, captures and close/reopen cycles. The window
  benchmarks record footprints only and make no leak claim.
- Text drafts that can't be saved are reported and dropped, in two cases. One case is a draft
  whose outcome is unknown when its field unmounts, which includes a draft that became
  unresolved earlier. The other is a draft sent from a version that automatic retention
  had to trim in a session exceeding its storage budget, to a field another edit changed meanwhile: the core refuses it (`stale_base`) and the field
  shows the saved text. The fix to plan is a recoverable draft with an explicit discard.
  Holding the close barrier instead made windows impossible to close.
- Each removed optional value or record entry that held a container, and each removed
  row holding one, leaves an empty mergeable container of about 19 bytes in trimmed
  documents, because Loro retains them by identity
  ([storage layout](reference/document-types.md#storage-layout)). Churning 1,000 such rows
  or unique record keys leaves about 37 KB. If that becomes material, ask Loro to drop
  inactive, empty mergeable containers from shallow snapshots; no layout change is needed.

## Next

- **Restore the archived examples.** Move each to `slop.ts` and the single file, check it
  in the app, and select the ones that ship.
- **Worker and worklet assets.** The shared page CSP now comes from `packages/schema`
  for native and browser pages. Explicit worker policy and build support for worker and
  worklet entry points remain: emit them as files in `assets` so `new Worker` and
  `addModule` load local URLs. Keep `blob:` and `data:` code refused and make failures
  understandable. WebKit does not isolate `slop:` pages, so there is no
  `SharedArrayBuffer`.

## Implemented foundation

- Rust owns edit admission, save scheduling, discard, close, command dispatch and the
  live socket. Swift delivers events and provides native UI and rendering.
- The engine creates and edits documents on macOS and Linux. Mac document commands
  prefer the engine shipped with the app; authoring validation uses the CLI's engine.
- Theme overrides share Loro storage, sequence, publications, undo and saving with data.
- Authoring has one generated Svelte entry. `Export.svelte` and `Icon.svelte` remain
  optional; captures use fresh saved-state pages, with a fresh App as the export fallback.

Implementation decisions, measurements and verification are recorded in the
[pre-launch simplification review](evidence/prelaunch-simplification-2026-10-04.md).

## Later

**Hosted templates.** A publishing service may accept validated immutable template
artifacts, expose a generated OpenAPI catalog, and download immutable cached masters.
Creating a document still makes a writable local copy, and bundled and cached masters
work offline. Plan a separate Cloudflare HTTP module with R2 artifacts and catalog
metadata, with TypeBox authoritative. Publisher identity, upload limits, package isolation
and checksums, immutable release identity and abuse controls are prerequisites. Any
worker kept in a local `deferred/` archive is unsupported scaffolding. The local app
needs no document server.

**Collaboration**, separate from hosted discovery. The owner can exchange Loro updates
with other replicas (it already imports and exports them), authenticate in Swift and
persist opaque updates remotely. Each replica keeps one writer and local SQLite storage.
Frontier version tokens and stateless text edits already work across replicas: a page's
text request names the history it saw, and the core merges it with whatever arrived
since. Values more than one replica can create (optional values, record entries) already
merge when created concurrently. Closing a large document trims all history, so sync will
need a retention policy compatible with offline replicas.
Remote edits would arrive as imports. Selective undo that preserves remote changes is
still needed: today a raw replica import clears the local undo/redo history.
Keep credentials outside authored code, and add a dedicated sync envelope rather than
overloading `apply`.
[Ideas](ideas.md#realtime-collaboration-on-durable-objects) sketches rooms on Cloudflare
Durable Objects, the SDK additions, and per-person `s.local` state.

Do not restore JSON room seeds, command/snapshot authority, guest snapshot reconciliation,
JavaScriptCore, data.json, or a second semantic validator. Convergence tests around
internal import/export do not constitute a shipped collaboration product.

## Deferred

Media import, account UI/Auth/App Check, a document history UI, schema evolution,
iCloud and other synced folders, and other native platforms. [Ideas](ideas.md) proposes
pulling additive schema changes and undoing an agent's change forward. Historical source
may be kept in the optional, Git-ignored `deferred/` archive; it is not in fresh clones.
Pre-launch documents are not migrated.
