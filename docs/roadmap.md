# Direction

Today hitSlop is a signed Mac app and one matching npm package. People create local
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

- Native `window.fullscreenable` and the local Chrome browser beta are implemented.
  The same Rust owner/store powers independent OPFS copies, bounded resource reads,
  command workers and Save As downloads back to native. Identity, export barriers,
  failed writes and recovery are covered at their owning boundaries. Browser launch
  polish, sustained-edit/near-limit performance qualification, Safari/mobile and hosted
  delivery remain deferred. `slop dev` keeps its native owner.
  [Hardening evidence](evidence/browser-host-hardening-2026-10-08.md) records the scope
  and verification limits.
- Finish the launch foundation qualification: complete-value replacement, template
  acceptance, command declaration checks, the SDK/ABI cleanup, dedicated captures and
  document UUID copy rules. [Implementation evidence](evidence/launch-foundation-2026-10-07.md)
  distinguishes implemented work from checks still pending.
- At launch, capture and freeze the first [compatibility corpus](testing.md#compatibility-corpus)
  entry; from then on every released document stays openable.
- Give that entry boundary documents: limits at their maximums and every descriptor kind.
  The dev corpus already covers every window kind (1× and 2× skins, glass, transparent,
  path shapes), app media (font, image, audio) and page commands. Opening checks only
  what format 1 accepts, so tightening authoring rules never rejects a saved document.
- Storage version 2, whenever it comes, ships with: a backup of each file before its first
  migration (a clean copy, `file/copy.rs`); a test that a frozen version-1 file migrated by
  a write equals a newly created version-2 file in layout and value; and a test that an
  interrupted migration leaves the version-1 file intact.
- Freezing that entry also freezes runtime ABI 1's command prelude: capture records its
  digest, and `scripts/build/runner.ts` refuses to change it afterwards.
- Check Quick Look by hand on a document received by Mail and AirDrop. Quarantined copies,
  a file another process is writing and a crashed write (the document icon until the app
  recovers it) are checked.
- Decide whether Time Machine copying a whole document at the current attachment limits
  is acceptable, or lower the limits for launch.
- Exercise the release engines on darwin-arm64, linux-x64 and linux-arm64 (Windows isn't planned).
- Loro is pinned to a git commit of its main branch (`c00c9fa`), because the fixes undo
  relies on are not on crates.io. Return to an exact crates.io version once one ships
  them, with the corpus passing.
- Check system IME composition and Edit ▸ Undo by hand (typing, ⌘Z inside a field, an
  agent edit between steps); no evidence file covers them.
- Shape Lab: the production-window shadow refresh, and the opening-only black strip, which
  was not reproduced and has no confirmed cause
  ([evidence](evidence/shape-lab-interaction-2026-09-30.md)).
- A long-lived memory study across windows, captures and close/reopen cycles. The window
  benchmarks record footprints only and make no leak claim.
- Recover unresolved text after its field unmounts. A draft whose outcome is unknown is
  reported and dropped when its control unmounts. Recovering it needs a UI outside the
  control; indefinitely holding close after unmount previously made windows impossible
  to close.

## Next

- **Restore the archived examples.** Move each to `slop.ts` and the single file, check it
  in the app, and select the ones that ship.
- **Worker and worklet assets.** The shared page CSP is defined in Rust for native and
  browser pages. Explicit worker policy and build support for worker and
  worklet entry points remain: emit them as files in `assets` so `new Worker` and
  `addModule` load local URLs. Keep `blob:` and `data:` code refused and make failures
  understandable. WebKit does not isolate `slop:` pages, so there is no
  `SharedArrayBuffer`.

## Implemented foundation

- Rust owns edit admission, save scheduling, discard, close, command dispatch and the
  live socket. Swift delivers events and provides native UI and rendering.
- The CLI's engine creates, edits and validates documents on macOS and Linux. The Mac
  app links its core and supplies the rendering helper independently.
- The CLI, SDK, contracts, templates and skills ship as one `hitslop` npm package with
  the Mac app's release version.
- Rust types own every wire and the app definition; ts-rs generates TypeScript and UniFFI
  carries types to Swift. Command arguments are `s.*` descriptors the owner checks.
- Named commands run in the owner's restricted evaluator for page buttons and agents
  alike; the page bundle carries no command bodies.
- Theme overrides share Loro storage, sequence, publications, undo and saving with data.
- Sync's core and its proof: one Loro peer per session, three-way text merges without
  `base`, Loro's `UndoManager`, a merge-closed layout, the `history`/`share` storage tables,
  and the sync session with its owner hooks ([architecture](architecture.md#sync)). The
  engine that ships syncs through a Durable Object byte relay on Cloudflare's runtime
  ([evidence](evidence/sync-spikes-2026-10.md)). The production relay and clients are
  [later](#later).
- `slop.ts` declares the app: `view`, optional `export` and `icon` views, commands and
  artwork. Captures use fresh saved-state pages, with a fresh `view` as the export
  fallback. `slop dev` runs the native owner on a disposable copy per preview page.

A historical summary of the ownership decisions and integration findings is retained in
the [pre-launch simplification review](evidence/prelaunch-simplification-2026-10-04.md).

## Later

**Hosted templates.** A publishing service may accept validated immutable template
artifacts, expose a generated OpenAPI catalog, and download immutable cached masters.
Creating a document still makes a writable local copy, and bundled and cached masters
work offline. Plan a separate Cloudflare HTTP module with R2 artifacts and catalog
metadata, with Rust acceptance authoritative. Publisher identity, upload limits, package isolation
and checksums, immutable release identity and abuse controls are prerequisites. Any
worker kept in a local `deferred/` archive is unsupported scaffolding. The local app
needs no document server.

**Collaboration**, separate from hosted discovery: each device edits its own `.slop`, and
copies exchange Loro updates through a Durable Object byte relay. The core is in place and
proven against the relay on Cloudflare's runtime; [architecture](architecture.md#sync)
describes the design and its accepted limits, and
[ideas](ideas.md#realtime-collaboration-on-durable-objects) holds the open proposals
(presence, per-person `s.local` state).

Still to build: the production relay (room creation with write and read keys, R2 for the
seed and attachments, rate limits, quotas, server compaction); the Mac client (a socket
connection with backoff, Share and Join, the room key in the Keychain, a sync status line);
attachment sync; a way to save a full shared document's live state as a new, unshared copy;
CLI `share` and `join`; the browser host's connection; and hosted-latency qualification.

Do not restore JSON room seeds, a second command validator, guest snapshot reconciliation,
JavaScriptCore, data.json, or a second semantic validator. Convergence tests around
internal import/export do not constitute a shipped collaboration product.

## Deferred

Account UI/Auth/App Check, a document history UI, schema evolution,
iCloud and other synced folders, and other native platforms. [Ideas](ideas.md) proposes
pulling additive schema changes and undoing an agent's change forward. Historical source
may be kept in the optional, Git-ignored `deferred/` archive; it is not in fresh clones.
Pre-launch documents are not migrated.
