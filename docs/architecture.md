# Architecture

pop documents are local files (`.pop`): one SQLite database that pairs an immutable
authored app with a structured document. The Rust foundation separates Loro document
semantics and app acceptance in `pop-core`, durable storage in `pop-storage`, and the live
owner in `pop-runtime`. Swift `DocumentOwner` passes document requests through, answers
the window's own, and delivers owner events to the page;
Loro bytes never reach Swift. The WebView renders immutable snapshots and holds no CRDT.

```text
page (WebKit) ── requests ──▶ Swift DocumentOwner façade ──▶ Rust owner (edit worker)
              ◀──────────── ordered publications ─────────────────────┤
                                                                     ▼
                                                        persistence worker ──▶ .pop

pop ── pop-engine ── Rust command router ──▶ same Rust owner
                         live: owner's socket; closed: acquire writer lock
                     ├── closed export, a build's artwork ──▶ restricted evaluator renders
                     └── open ──▶ LaunchServices ──▶ dotpop.app
```

Descriptor kinds: text (merging); boolean, string, number, integer and enum (last
writer wins, checked on write); optional (of a scalar, text or an object); object;
list of object rows with `$id`; list of scalars by index; record of scalars or objects
by key; and counter. [Document types](reference/document-types.md) describes each
kind's snapshot, merge, write rules, handles and CLI paths.

The pure `pop-core` semantics compile to WASM for SDK tests. The browser feature adds
`pop-runtime` and `pop-storage` for durable browser copies.
`pop open --browser file.pop` asks the native owner for a clean snapshot, then imports
it into an independent OPFS copy. A dedicated worker runs the shared Rust owner, Loro
and SQLite; named command execution is unavailable in browser copies.
The trusted host stays on `127.0.0.1:41238`; authored apps run in per-copy
`<id>.localhost:41238` frames. Each copy has a Web Lock, its own SQLite VFS pool
and a Service Worker resource channel bound to that frame client. Each successful import renews the document UUID; reload preserves it. Downloads drain
pending text, pass a Rust save barrier, and stream the current database through Chrome’s Save As dialog, including theme and attachments. Unique temporary exports are deleted
when writing finishes or abandoned-copy cleanup acquires the lock. Resource delivery
uses bounded chunks, incremental attachment verification and a bounded decoded cache. Browser storage is local to the profile and is not a backup.
Safari and mobile browser support are deferred. Hosted sharing uses the live
collaboration service described under [Sync](#sync).

`pop dev` uses a temporary `.pop`
document and a native owner per page; Vite supplies modules and HMR. UI edits retain that
owner, while a changed declaration invalidates it and creates a fresh preview. Linux
preview qualification is deferred.

Here "native owner" means the compiled Rust engine, not a native window. The browser
renders the development page; Vite forwards its WebSocket messages as newline JSON to
`pop-engine --preview-owner` over pipes. The Rust transport never blocks an owner
callback on its output. A full output queue or a stalled reader fails the disposable
session, fences the page and attempts a bounded close; the browser must reload rather
than replay an edit with an unknown outcome.

The typed TypeScript CLI invokes `pop-engine` as a separate process, once per request,
and gets structured JSON back. On macOS and Linux, a closed document's export and the artwork a
build packs render inside the engine, in its restricted evaluator, with no app involved;
an open document's export routes through the Rust command router to its live owner.
`open` is the one request that starts dotpop.app: the engine asks LaunchServices to open
the document in it. `open` refuses a template, a link and any file that is not a regular file;
a linked parent folder is fine. The restricted command evaluator's short execution budget is
separate from capture budgets.

Authors put metadata in `pop.ts`, shape and initial values in `schema.ts`, and the editor
in `App.svelte`. Required `Preview.svelte` and `Icon.svelte`, optional `Export.svelte`, `Widget.svelte` and agent-only `commands.ts` have
fixed root roles. The UI and definition graphs share resolution: the page imports the
schema and components, while the definition imports metadata, schema and commands.
A private capture graph compiles synchronous Svelte SSR for every Preview.svelte,
Icon.svelte, declared Export.svelte and Widget.svelte role. A capture's CSS is exactly what its own
Preview/Export/Icon/Widget role imports, in import order, including component CSS.
Captures no longer inherit the editor's stylesheets; `pop dev` still mounts static
views inside the editor page. Takumi uses tolerant CSS
parsing, so unsupported declarations do not refuse a whole shared stylesheet.
The graphs do not rewrite command bodies or substitute component tokens. The restricted
definition evaluator returns a declaration for `BuildInput`. Rust checks that input and the explicit resource inventory before
packing. Persisted and exchanged JSON readers ignore unknown members, while executing
requests, descriptor/arguments grammars and authoring input remain strict. Opening,
reading and saving limits are versioned by their owning marker and pinned by the corpus
ratchet; acceptance vectors pin positive decoding behavior. Imported images, fonts and CSS URLs become content-addressed media resources;
unimported files and `public/` are never copied.

The immutable `app` row contains catalog columns, compatibility markers and one
`definition_json`: the window, ordered theme, original document descriptor, views and
command descriptors. `assets` contains `ui.js`, optional `ui.css`, private `commands.js`, `widget.js`, `icon.js`, `preview.js` and `export.js`,
and media. Pack inserts assets before the app row, whose presence seals them. Attachments
and artwork have their own mutable lifecycles. Initial values seed the history's first snapshot; the authored schema module also includes
those creation defaults. There is no JSON mirror of saved document state.

Markers are checked before interpreting fields or comparing the layout. The format-1
acceptance module is Rust serde plus explicit checks, including the descriptor validator
and streamed PNG decoding. Skins are RGBA PNGs at exactly 1× or 2× the logical window.
SVG paths use `svgtypes` and `kurbo` with bounded commands and segments. Open-time acceptance
checks resource inventory, encodings and size bounds, the private command program, window
skin and saved state. UI resources decode when loaded. `pack` verifies media content
addresses and the app row seals them, so serving never re-hashes app media. Damaged ordinary resources
fail their load rather than refusing the document. The owner keeps its accepted app. Catalog and
Quick Look summaries read only bounded scalar columns and artwork; a summary is not a
certificate that the whole file is valid. Neither display path migrates the file.

Rust owns wire types, errors, limits and app metadata. ts-rs generates TypeScript;
UniFFI carries typed app definitions, summaries, window geometry and host actions to
Swift. Swift never decodes the app definition, theme or page-message JSON. Rust decodes a page
request once, applies its lifecycle fence, answers document requests, or returns a
native host action. The development preview supplies an already typed request through
`page_typed`, which checks the same bounds and owner view fence without a JSON round trip.
Document JSON remains opaque text through the bridge. TypeBox,
quicktype, handwritten contract generators and production JSON Schema validators are gone.

Agent commands through `pop call` send a name and arguments to the owner. Rust
validates the argument descriptor, evaluates the private program in a fresh restricted
child, validates its intents and applies one batch. Evaluation runs off the serial owner.
A definite stale base retries once with the original clock and seed; an unknown outcome
never retries. Completion waits for publication and persistence. Pages use handles and
`doc.change()` for edits; they contain no command stubs or implementations. The page
context and runner prelude have explicit ABI-1 arms.

Attachments load through same-origin URLs, with range requests, sniffed passive media
types, `nosniff` and a sandbox policy. The resource reader verifies the complete attachment
hash on first touch. Compressed app assets share a bounded decoded cache. Rust plans range status and headers for native,
browser and preview resource adapters, which retain their transport and origin checks. Script sources are restricted to shell and app-asset paths;
attachment and command-program paths cannot become script sources.

The CLI's engine forwards to a live owner or acquires the writer lock. Its command
protocol is exact, with no old CLI adapters. Owner socket operations carry `{protocol, request}` around the engine
request type and return the same typed successes and failures. Protocol refusal precedes
payload interpretation; hello preflight and transport limits still apply. Stored `packageFormat`, `runtimeABI`, SQLite storage version and Loro layout
remain independent; [compatibility](engineering-contract.md#compatibility) and the
[corpus](testing.md#compatibility-corpus) define the released-file guarantee.

## Foundation redesign

The Rust foundation redesign has passed local integration qualification. The crate
boundaries below are implemented; the checklist in
[Direction](roadmap.md#rust-foundation-redesign) records completion and validation.

`pop-contracts` contains passive wire, error and limit types plus app-presentation,
media-registry and evaluator types; it has no Loro, SQLite or renderer. `pop-core` owns
pure Loro semantics, app acceptance and sync and has no storage/runtime features by
default. Its optional `acceptance` feature enables tooling codecs and `ts` enables type
generation. `pop-storage` owns SQLite files, file acceptance, registry, attachments and
atomic publication. `pop-capture` owns saved-source capture and widgets. `pop-runtime`
owns owner, command, socket and persistence orchestration. `pop-renderer` owns Takumi and
fonts without JavaScript or owner code. `pop-library` owns local library indexing and cloud-document
coordination over runtime/storage; the native cancellable-I/O sharing client uses runtime.
The engine and FFI compose these crates; WASM SDK uses pure core, while its browser feature
adds runtime and storage. Loro remains pinned; there is one owner/edit path and no
persistent JSON mirror or second document engine.

`BUILD_ID` in core identifies the semantic fingerprint. Runtime's foundation fingerprint
covers core, storage, capture, runner, renderer, contracts, sharing protocol and fonts;
engine and FFI report that runtime foundation ID.

The redesign permits breaking internal APIs. New documents use layout 2, while published
layout-1 documents retain their representation and acceptance through the same engine's
layout adapter. Historical release assets, expectations, preludes and hash pins remain
immutable. Mechanical extractions do not raise format markers.

Layout-2 concurrent creation uses generation-aware mergeable slots: creation from the same
observed absence shares a payload; clear advances the generation; stale edits cannot
revive cleared content. Creation defaults are a shared baseline rather than writes
that overwrite another peer's explicit values. Saved-state acceptance and projection
share one layout-specific path.

Routine checkpoints preserve CRDT identity and session undo. Deep compaction is a
separate safe-close or explicit operation: it reconstructs accepted visible state into
a fresh checkpoint epoch and may reclaim old generations and unreferenced attachments.
A shallow Loro snapshot alone does not guarantee that hidden mergeable data disappears.
An outdated replica with unconfirmed work must preserve a recovery document before
refreshing. Replicated epoch replacement requires a conditional publication; it must
not make closing depend on network availability. There is no indefinite history archive,
device registry or permanent acknowledgement protocol. Independent recovery documents
preserve uncertain local work; their files remain discoverable after interrupted adoption.

Firestore discovers cloud documents across a person's devices. Persistent Document
DOs carry private sync and collaboration through one authority; R2 holds immutable
payloads. Account identity and quotas belong to the control plane; permission checks
and resource limits remain server-enforced. The transport carries opaque Loro bytes.

Named `commands.ts` commands remain an optional Mac CLI/agent automation interface.
Ordinary page editing uses handles; UI and commands can share plain transaction
functions. Process and in-process capture providers coexist explicitly; iPhone command
calls, SDK command calls and browser command workers are deferred until a concrete
invocation surface is implemented. In-process iPhone capture remains supported.

Separate Rust `StateContractId` and exact `AppRevisionId` identities prepare future explicit
bundle updates. Private-library compatibility uses the state contract; the relay still
requires the exact app digest through its existing wire protocol. Documents continue
pinning an immutable accepted app; background
updates, mutable partial bundles and schema migration are outside this work.

### Owner admission, callbacks and epoch installation

The serial owner admits at most 128 ordinary requests retaining up to 56 MiB, with eight
separate control slots and a shared 64 MiB ceiling. Flush, Discard, Close, epoch adoption
and native Handoff use control admission. Copy and epoch-preparation requests share a
two-producer limit. Host dispatches use ordinary admission; derived snapshot/export
buffers retain byte leases until their work settles. Exhaustion returns typed `busy`
before mutation. Control payloads can also exceed the shared ceiling. These operational
budgets account for retained work, not total Loro memory or process RSS.

Callbacks settle once, including requests queued behind shutdown. Their admission permits
are released before invocation; a listener, completion or sync-sink panic does not kill
the owner or leave later requests stranded. Definite Busy/Locked/refusal results keep the
owner retryable. An uncertain canonical write fences further writes until its outcome is
reconciled; a failure creating the recovery copy before canonical installation does not
invalidate the original owner.

`PrepareEpoch` binds a candidate to an exact source version and, for a fresh relay epoch,
required confirmed heads. Preparation does not replace the live document. `AdoptEpoch`
installs an accepted candidate under the existing writer lease, with a new-UUID recovery
first when the live version differs from the last confirmed authority version. During
installation, immutable `State` reads still return the last accepted snapshot; other
requests return temporary `busy`, and old sync traffic is fenced. Success emits one
ordered full publication when history changes. No-op adoption retains undo and its blobs.
The detailed publication, receipt and recovery rules are in the
[sharing lifecycle specification](reference/sharing-lifecycle.md).

## Layers

| Layer | Where | Owns |
|---|---|---|
| Contracts | `crates/pop-contracts`, `crates/pop-sharing-protocol` | Passive wire/error/limit and evaluator contracts; sharing wire contract |
| Core | `crates/pop-core` | Pure Loro semantics, app acceptance and sync; no storage or runtime feature by default |
| Storage | `crates/pop-storage` | SQLite `.pop` layout, file acceptance, registry/writer lock, attachments, save policy and atomic publication |
| Capture | `crates/pop-capture` | Saved-source capture and widgets, using storage, core and runner |
| Runtime | `crates/pop-runtime` | Owner, command/socket routing, persistence worker and save orchestration |
| Engine | `crates/pop-engine` | Authoring validation and packing, including artwork rendering on macOS and Linux; document creation, inspection and requests; closed PNG/PDF export on both platforms and macOS app opening through `/usr/bin/open -a` |
| Runner | `crates/pop-runner` | Restricted child evaluation of build declarations, stored commands and native image capture; ABI-specific preludes, sandbox and execution limits |
| Renderer | `crates/pop-renderer` | Takumi raster/PDF output and fonts; no JavaScript or owner |
| Library | `crates/pop-library` | Local library indexing and cloud-document coordination |
| Sharing client | `crates/pop-sharing-client` | Native cancellable I/O over runtime |
| Adapters | `crates/pop-core-ffi`, `crates/pop-core-wasm` | Compose crates for native FFI and WASM SDK/browser adapters |
| Session | `PopDocument/DocumentSession.swift` | WebView, the `pop` message handler, the push queue, native export callback and the lifetime of the Rust socket server; the window is its `DocumentSessionDelegate` |
| Window | `PopHost/PopWindow.swift`, `PopDocumentAttention.swift`, `PageFailureNotice.swift` | How a document looks: shape, toolbar, pin level, theme panel, failure screen (Reopen, Copy details, Get help) and the save-failure sheet (from the owner's save status); its document operations go to the app |
| Quick Look | `apps/apple/App/QuickLook{Thumbnail,Preview}` | Finder, Mail and share-sheet thumbnails and previews from the file's artwork, read through the core in a sandbox |
| App | `PopFeatures` (`AppModel`, `CommandQueue`, `CatalogModel`), `PopCatalog/PopApplicationCoordinator.swift` | Opening, one document operation at a time per document (a close or a save recovery requested meanwhile runs next), quit, the catalog, and alerts for failures that are not save failures |
| Author SDK | `packages/pop/src/sdk` | Descriptors, public types, errors, Svelte adapter; no host runtime |
| Page shell | `packages/pop/src/shell` (served at `/__shell__/`) | Store, handles, text binding, write queue, barrier, attachments, theme application |
| Contracts | `crates/pop-contracts` and `packages/pop/src/schema` | Rust owns shared wire types and limits; ts-rs exports TypeScript and UniFFI carries native types |

The window's failure screen (Reopen, Copy details, Get help) and the save-failure sheet
are presented by `PopDocumentAttention.swift` and `PageFailureNotice.swift`. Native
host-to-page calls (publish, reload, flush, undo/redo, prepareClose, cancelClose and every
capture's acquisition) have a 15 s deadline. On expiry the session takes the renderer-death
path with its own unresponsive reason and ends the page's process: the screen says the pop
stopped responding, accepted edits are kept, and the save sheet stays. Reopen recovers the
interface.

## An edit

1. **Page.** A handle write (`set`, `insert`, `remove`, `move`, `increment`) or a
   `change(tx => …)` collector becomes one batch. Batches go through one FIFO queue, so an
   `insert` followed by a `move` cannot reorder. A scalar `set`, `clear` or assigned
   `value` also shows at once as a local preview over the snapshot; acceptance settles
   it and a refusal reverts it. Assigned values commit after 150 ms without another
   assignment, or at the next barrier.
2. **Host.** The page posts `apply {batch}`, with a nested batch object. The native session checks
   the sending WebView, frame and origin and supplies its own lifecycle view token
   alongside the page's request to the core's page dispatcher, which checks the envelope and the token before calling
   `apply_batch`. The reply is `{sequence, ids}`. A batch that changes nothing publishes nothing and leaves the
   document clean.
3. **Push.** The core's publication, `{previous, sequence, version, ops, theme?}`, is
   appended to the session's push queue on the owner queue, so pushes keep owner order.
   One drain at a time delivers everything buffered through a single awaited
   generated host `publish` command. Swift never parses publications. An edit to an
   existing text field publishes a `text` op with its hunks (retain, insert, delete, in
   code points of the previous text), not the whole field. `theme` carries the
   effective palette only when that palette changed; a theme-only publication still
   advances the sequence.
4. **Store.** The page applies publications in sequence order, ignores any at or below
   its sequence, and copies only the objects on the changed paths; unchanged rows keep
   their identity. The write's promise resolves once the store reaches the reply's
   sequence, so the snapshot has updated when `await` returns.

One owner applies every edit, and every accepted edit keeps the stored state matching
the descriptor, so a snapshot needs no repair. Opening checks the saved state against the
app (descriptor and palette) and refuses a mismatch as `invalid_bytes` without writing.

A gap (`previous` above the store's sequence), an apply failure, or the host's queue-overflow
`resync` marker makes the page call `open` again and replace its state. The host keeps
that marker as the bounded push queue's recovery path. Text still in a field survives: bindings keep their
DOM text and the text they last confirmed, which the owner merges from.

## Text

The page and agents change text the same way: a `set` with `from` means "this field was
`from` when I last saw it and is now `value`". A text binding keeps the user's text in the
field and sends at most one such batch at a time,
`{intents: [{type: "set", path, value, from, selection}]}`. An agent passes the text it
read as `from`. No version names the starting point, so no edit is ever too old to merge.

- The core computes the edit script on a throwaway document (its diff mutates while it
  runs, so it never touches the owner) and checks that the script reproduces the value.
- **Fast path:** the owner's field still equals `from`, so the script applies directly.
- **Merge path:** the field changed since. The core diffs `from → current` (what changed
  concurrently) and `from → value` (the writer's edit), rebases the writer's script over
  the concurrent one and applies it to the live text as the session's own peer. At one
  place, the concurrent insertion comes first and the writer's after it, so a caret at the
  end of its own typing stays there. Both diffs work on strings: when both sides delete
  one of several identical adjacent characters, one of them may survive. Merges between
  devices are Loro's own and exact.
- A set carrying `selection` is the page's edit and its batch's only intent. The reply
  carries the merged selection. If the user kept typing, the next batch goes from the
  text this one sent.
- During IME composition nothing is sent. Close and export commit a composition.
  Retargeting or unmounting a binding sends its unsent text first. If that send has an
  uncertain outcome that cannot be resolved, the page retains the draft and field path
  in memory without replaying the edit. On Mac, close and quit offer Copy and Close,
  Discard and Close, or Cancel before destroying that page. Copy includes full text;
  diagnostics contain only a summary. Cancelling restores editing and retains the draft.
  Copies from multiple windows during one quit accumulate on the clipboard. Recovery
  remains page-local and does not survive renderer termination.

A text set without `from` (a text handle's `set(value)`, or the CLI without it) replaces
the whole field as it is when the owner applies it, through the same script.

Merging works at three levels:
- **Characters.** Between devices, Loro gives each character an identity. A concurrent insert lands next
  to the character it was typed beside, and inserts at one position are ordered by peer,
  so two people's typing never interleaves. Coarser units are worse: words duplicate when
  two people fix different letters of one word, lines conflict across paragraphs, and a
  whole field is last-writer-wins.
- **Typing.** The script keeps the common prefix up to the caret and the common suffix,
  so a keystroke is one splice and no diff runs.
- **Replacements.** Where a span is both deleted and inserted (a paste over a selection,
  autocorrect, a rewrite), Loro's character diff runs over the changed window. Its matches
  are then cleaned up as diff-match-patch's semantic cleanup does: a kept run between
  edits that is no longer than the larger edit on each side becomes part of the
  replacement, so a rewritten word does not keep stray letters that a concurrent
  keystroke could anchor to. Loro's refined diff stays off: it prices every gap the same
  whatever the field's length, so it would turn two small edits in a short field into
  one replacement.

## Undo

Edit ▸ Undo and Redo revert changes made since the document opened: the person's, the
window's, and an agent's made through the CLI or socket. All of them are the open
session's own Loro peer, and Loro's `UndoManager` undoes that peer's steps: an undo
reverses only the step's own operations (the palette with the data) and is rebased over
changes made since by anyone else, which it keeps. A document opens with nothing to
undo; the core keeps up to 100 steps per open document. A restored row keeps its `$id`.
There are no JSON snapshots or persistent undo records. The step limit is not a memory
budget: a grouped step can contain many edits, and dropping an undo step does not remove
its operations from the live Loro document.

- **Steps.** Each page batch is a step. A typing run is one step: consecutive text edits
  to one field, each starting at the caret the last one left. Consecutive agent batches
  are one step, so one undo reverts what the agent just did. Consecutive window changes
  to one palette color (a picker drag) are one step. Runs are `UndoManager` groups. Any
  other change, a merged page text edit, or undo/redo itself ends a run.
- **Refusals.** A batch that could fail after its first change (more than one intent, or
  a JSON replacement) runs first on a copy of the current state, so a refusal never
  touches the live document: undo, redo and the current run remain. If a live batch is
  refused anyway, the owner rebuilds at its pre-call version under the same peer and
  its undo starts over. No-op edits also preserve history.
- **Result.** An undo is a new change: it publishes, autosaves and travels like any
  other. Nothing to undo publishes nothing.
- **Window.** The document window's `NSUndoManager` (`DocumentUndoManager`) reports
  whether undo and redo are available and sends them to the page, which first sends
  unsent text, queued writes and previews (`__pop.undo`, `doc.undo()`), then asks the
  owner. WebKit's own text-editing undo still registers there, as AppKit's grouping
  requires, but is never performed. Without a live page the owner undoes directly.
- **Browser.** In `pop dev`, a text field's own undo (`beforeinput` `historyUndo`) is
  replaced by the document's.

## Saving

- Autosave waits 150 ms after an edit, and at most 1 s after the first unsaved one.
- The owner queue asks the store for a save job, which exports only what it writes: the
  updates since the last save, or a checkpoint. The persistence worker runs SQLite reads and writes; save-job construction stays on
  the edit worker. At most one write is in flight, and edits during it
  coalesce into the next one. The job's bytes stay in Rust.
- A failed or unacknowledged write retains the last confirmed saved version. The store
  re-reads its stored sizes after the failure, so the retry chooses append or checkpoint
  again. Overlapping Loro updates are safe to import; only a confirmed write advances the
  saved version. An unacknowledged commit can therefore show a save failure until retry
  succeeds.
- A write that fails keeps ownership and all edits. Failures are typed
  (`full`, `busy`, `moved`, `invalidated`, `io`) and reach the window, which offers retry,
  or discard for a full document that is not shared.
- `flush` resolves when the saved sequence covers every edit accepted before the call.
  `close` refuses new edits, flushes, trims history (below; never a shared document's),
  writes the artwork its
  window captured ([close](#close-export-and-capture)), reclaims unreferenced attachments
  in unshared documents only ([attachments](#themes-and-attachments)), then releases the lock.
  `discard` waits for the write in flight and reloads saved bytes; work queued for the
  discarded state (a waiting flush, a page that was replaced) is refused with
  `owner_replaced`.
- Save status flows one way, owner to window: the edited mark and the failure sheet. The
  page learns durability only through `flush`, which resolves once saved and rejects
  when the save fails. A renderer failure, including a page call that exceeds its deadline,
  keeps the native save-recovery sheet above the failure screen; only a save or an explicit
  discard clears it. Retry saves through the retained owner without reopening the page.

The SQLite schema has seven tables:

| Table | Contents | Written by |
|---|---|---|
| `app` | Sealed catalog row, compatibility markers and app definition | Pack, once |
| `assets` | Sealed app resources | Pack, before the app row seals them |
| `artwork` | Preview and icon PNGs | Pack; refreshed at close or supplied for a clean copy |
| `document` | Identity singleton: UUID | Document creation and independent copies |
| `history` | Loro snapshot, then updates: the saved state | Pack seeds it; saves, checkpoints and close-time trims update it; clean copies replace it with current state |
| `share` | Singleton room and relay endpoint of a shared document | The live sync path; empty when unshared, cleared on clean copies |
| `attachments` | Host-owned immutable content-addressed blobs | The owner's attachment store; unshared documents reclaim unreferenced blobs |

The schema check is exact. A new table or column requires a storage version bump and a
lazy migration under the writer lock. An older build refuses that file with
`requires_update` (“update pop”) before comparing its tables. Optional additions
that need no bump live in open surfaces: definition JSON members, Loro containers and
artwork rows. [App identity](engineering-contract.md#app-identity) uses a fixed encoding
of the sealed app and assets.

A replicated document's ordinary saves retain history and attachments that other replicas
can reference. Reclaiming them requires a confirmed authority checkpoint; membership
alone never authorizes a history replacement. A clean independent copy drops the share. Creating a
document validates the template's complete saved state in the source read transaction,
then atomically writes a fresh UUID and initial snapshot before publishing the file.
The UUID names the logical document, not its writer or its inode-based lease. Open,
save, rename, capture snapshots and internal backups preserve it; explicit Duplicate
and independent editable copies receive a new UUID. Templates have no UUID. The core
keeps this identity outside authored state; it is never a Loro peer ID.
A history that does not start with a snapshot is refused and preserved for recovery.
Saving prefers a checkpoint (one snapshot replacing the saved log) once the log reaches
256 saved updates or 4 MiB of update bytes. A regular snapshot retains history; a shallow
snapshot discards history before its starting version. The limits are 4,096 saved updates
and 32 MiB of saved Loro checkpoint and update bytes, not the whole `.pop` file: the app,
attachments and artwork are separate. Shared storage limits are defined in
[`wire/limits.rs`](../crates/pop-contracts/src/wire/limits.rs) and projected into host types.

Unshared documents need no lifetime edit history: undo covers the open session only.
After its final save, a session that saved an edit or stored an attachment to an unshared
document with more than 4 MiB of saved Loro data attempts one more checkpoint
(`Store::close_job`) that keeps only current state, when that would shrink the saved data.
This pruning is best-effort housekeeping: failure does not prevent closing after the
final save succeeded. Layout 2 may rebuild a fresh epoch; released layout-1 files keep
their layout and history identity through a shallow checkpoint. A session that only
reads does not prune.

While open, an unshared document's checkpoint keeps the whole history when it fits the
32 MiB limit and only current state otherwise. If neither a checkpoint nor an append fits,
saving fails and ownership is retained for retry. The live document keeps its own history
either way, so the session's undo still works. Replicated history replacement is explicit:
the owner prepares a candidate without changing its live state, the authority publishes it
conditionally, and only then does the owner install the accepted checkpoint. Changed or
uncertain local work gets an independent recovery first. A changed history clears undo;
an idempotent installation of the current version preserves undo and the blobs it needs.
There is no public live-compaction command. Only the checkpoint may start history late. Files
use full auto-vacuum: every commit returns the pages it freed (the log a checkpoint
replaces, the artwork a close replaces), so a file holds no dead space. Storage links the
platform SQLite, the same library other in-process consumers load, and the Rust storage
layer is the only code that opens a `.pop` file.

Live commits record timestamps and the messages `page`, `agent`, `command:{name}`,
`window`, `undo`, `redo` and `create`. Template
seeding uses deterministic operations without wall-clock timestamps. These messages
describe origins; they are neither unique request IDs nor a durable audit log.

## The file, its lock and copies

A template holds an app; a document holds an app and its saved state. Creating a
document copies a template and adds its identity and initial Loro checkpoint in one
transaction; nothing is ever unpacked. The core decides where a document may live, for
the app and the engine alike: opening one for writing needs a `.pop` name outside iCloud
Drive, and creating or copying one also refuses the installed and bundled templates
(`file::document_location`, `file::document_destination`). The regular-file, no-link-leaf
rule has one owner in the core (`file::existing` / `registry::regular`), used by every
document request, `open` included. Swift adds only what
Foundation alone can tell, a folder iCloud syncs such as Desktop and Documents. The writer
lock is an `flock` on a registry file outside the document (`~/.pop/live`, named by
the file's device and inode), never on the database: closing any second descriptor on a
SQLite file drops SQLite's own locks. The lock holder publishes discovery beside it and
removes a crashed owner's. A rename stops the writer (`Moved`): SQLite names its journal
after the path, and Apple's SQLite never writes again through a connection whose file
was renamed. When the file is back where it was opened, the store reconnects and saves.

Native save panels default to the last usable local folder, else Desktop when it is
local, else `~/pop`. Opening a document that lives in iCloud offers "Move to ~/pop
and Open": the core copies it through coordinated access into a fresh name with the same
identity and retained history, validates the copy, and only then moves the original to the
Trash. If cleanup fails, both files are kept and the app says where the local one is.
Devices share a person's own documents through the [Apple library](#synced-library),
not through synced folders.

The native Share popover offers PDF, PNG image and editable pop-file copies before
opening the macOS sharing picker. Opening the chooser creates no output; selecting a
format runs through the document command queue. PDF and PNG use the saved-state export
renderer. **Live collaboration** in that popover creates an edit link through the
service described under [Sync](#sync).

Duplicate and Share Pop File drain page edits and acquire a temporary clean copy of
accepted live state through the owner, even if saving the source history returns Full.
Both artwork and the new document come from that source, so
edits accepted while rendering cannot make the preview disagree with the copy. After
rendering, a temporary native owner takes the source's lock and performs the clean copy;
no authored code runs in that owner. The copy is made a document of its own before it is published
(`Store::copy_clean`): its live snapshot becomes a checkpoint with no history, it keeps
only the attachments that state references, and it carries the rendered artwork, or none
if rendering failed, never the original's. Nothing deleted before the copy is in it, and
writers zero deleted content (`secure_delete=FAST`) on every platform. The copy is
published without replacing anything. A capture's source is a plain backup, rendered
once and deleted. Share staging lives until the sharing picker is cancelled or the
selected service finishes, independently of the originating window. A window writes the file's
artwork as it closes ([close](#close-export-and-capture)). Finder, Mail and the share sheet
show it through the app's Quick Look extensions, which read the file's artwork read-only;
a file without artwork shows the `.pop` document icon.

## Close, export and capture

The page close barrier sends unsent text and waits for queued writes and attachment
imports; the native session then flushes the owner for ordinary close. Preparation
distinguishes drained edits from durable edits. Handoff drains without requiring the
full source to save, resolves retained text drafts, and publishes a verified clean
copy before releasing the original. A failed save or cancelled recovery unblocks the
page and keeps ownership. A page flush never joins a flush that already passed its
drain point. Inputs stay enabled, so focus survives a cancelled barrier. Export, preview and icon rendering use
saved state independently of the editor. For an open document, the core first backs up
SQLite after the page drain (within the page-call deadline) and save; a page that does
not answer counts as a dead renderer. The temporary copy owns its app, attachments,
data and theme independently of the editor's lifetime. No read transaction spans
rendering. Icons, preview artwork, widgets, PNG exports and vector PDFs use synchronous Svelte SSR
and Takumi in the restricted Rust child; there is no renderer choice, fallback or `pop.ts`
override. The interactive editor continues to use WebKit.
`Preview.svelte` and `Icon.svelte` are required when authoring; `Export.svelte` is optional.
Preview supplies a compact overview for Quick Look and the catalog. Export supplies complete
PNG/PDF output; without it, image/PDF share/save actions are hidden and CLI export is refused.
Stored-file acceptance remains separate.
Missing programs fail capture with a rebuild diagnostic. Rust owns output formats
through one `CaptureOutput` type; Swift consumes an already saved source, leaving
acquisition and publication at their existing boundaries.
Native callers reuse a Rust `CaptureRuntime` for evaluator configuration. A Rust batch
loads one saved document and lazily prepares its snapshot, theme and image inventory once
for its requested outputs, reporting each result independently. One failed output does not
discard the others. Close requests artwork and eligible widgets in the same batch.
Each output starts two restricted children: Markup evaluates synchronous QuickJS SSR;
Raster receives frames, fonts and selected images and never executes JavaScript.
The iOS preview uses the same Rust capture pipeline in-process on a serial background
queue for icon, preview and widget PNGs. QuickJS retains its heap/stack limits and cooperative
deadline; native raster work has no child-process crash isolation. Authored commands
remain unavailable on iOS. The Mac renderer continues to use restricted children.
A typed job envelope supplies separate markup, widget, PNG and PDF budgets.
Parent and child share render replies and typed failure serialization; the parent
validates reply combinations. Authored requests carry serialized JSON with image IDs
and media types, never image bytes. The core scans frame HTML and CSS for attachment
URLs, verifies only those blobs and enforces a 24 MiB image budget per output. Only
inventoried PNG/JPEG/GIF/WebP images enter Raster, with no network or filesystem
access. Imports, reads and writes are unavailable to capture components. The stored
program contract is [runtimeABI 1](reference/runtime.md#stored-capture-program-abi-runtimeabi-1).
PNG and PDF share layout, fonts and resources. PDF is one content-sized vector page,
with selectable text and uniform scaling to 14,400 points per side through upstream viewport DPR.
Takumi and its experimental PDF crate use one pinned Git revision; no local source fork
or spike supplies production code. Distribution notices live in
`crates/pop-renderer/renderer-licenses`. Capture never changes the open
editor's focus, selection, scroll, frame or selected tab.
Source acquisition preserves speech, media access, file selection, downloads and resize
eligibility. Its native queue still coordinates close, discard, reload and reopen.
Close preparation, page replacement and renderer loss cancel pending transcription.

If the session changed the document, or the file has no preview, close captures its
preview and optional icon from that saved source. The owner writes successful artwork
through its writer connection and releases the lock. Finder's custom icon reflects the
saved artwork. A failed capture keeps old artwork and does not stop close; a failed save
keeps the window and ownership for retry. After a ready page's close barrier saves its
edits, its window hides and the catalog returns if no other document is being presented
or opened. Artwork and final close continue with the same owner and writer lock. The
catalog initially shows existing artwork and refreshes when new artwork is written.
Opening that document meanwhile queues one reopen after ownership is released; a failed
close restores the retained window. Quit waits for these closes and cancels queued reopens.
Local Instruments signposts in `click.dotpop` / `DocumentClose` separate the barrier,
browser handoff, snapshot, rendering, owner close and artwork announcement, without
recording document content or paths.

An attachment import stores the blob, then submits its reference through a
collector admitted past an active barrier. A collector that throws, or an edit the core
refuses, leaves the blob unreferenced. Unshared documents reclaim it on close; shared
and Synced Library documents retain it for other replicas.

## macOS image widgets

An optional embedded `Widget.svelte` renders small, medium and large PNGs from one
saved snapshot, with family and viewport props and the document's effective theme.
It never falls back to an export or preview. A private synchronous Svelte bundle runs in
QuickJS, then Takumi renders all three canvases in a separate restricted child. The owner
supplies one saved source; authored execution and rasterization never run in the owner
or WidgetKit extension. The renderer uses portable default fonts and color emoji;
packaged @font-face fonts are registered with their authored family aliases per batch.
It does not discover system fonts or fetch remote resources. Each batch retains the
3-second process watchdog and CPU limit per stage, a 256 MiB QuickJS heap and an 8 MiB raster reply cap.
Linux additionally limits address space to 2 GiB; on macOS the parent samples resident
memory and terminates a widget child above 512 MiB. This is a polling threshold, not a
hard allocation ceiling.

Successful saves coalesce for one second before capturing; opening a document with
missing cache also schedules capture. Closing an eligible document after edits or with
missing cached images publishes the three images into a disposable App Group cache.
The running app watches cached source files, including saves made by a closed CLI
owner. Events coalesce before comparing a core-produced freshness fingerprint
with the version that produced the image. It combines the saved Loro version, the
installed evaluator's bytes (including Takumi and bundled fonts), provider identity, and widget canvases
and scale. Startup and reopen perform the same check, even without a refresh interval.
A renderer or capture-ABI change refreshes every cached image once; a core-side change
that alters pixels bumps the provider tag. Artwork-only writes do not refresh images.
The evaluator digest is computed off the main thread and cached against file identity,
size and modification/change times. An evaluator changed during capture cannot publish.
The fingerprint travels with the rendered snapshot, so a newer save cannot be marked
fresh by an older capture. Watching starts with a version recheck to cover saves made
while the first capture was running.
The owner still controls the close barrier and document artwork. A widget capture or
cache failure keeps the previous generation and does not become a save failure. Older
cache tokens are simply stale: the previous images stay visible until replacement
succeeds. This disposable cache policy does not change the document format.

One configurable WidgetKit extension offers the three sizes. Its AppIntents `Document`
parameter lists successfully captured documents. The extension reads presentation
records and PNGs only. The app and extension compile the shared intent source directly
so each bundle owns resolvable AppIntents metadata; the app passes active document IDs
to the publisher. Swift owns the disposable cache format and image bounds; the
extension does not link the document core. Rust owns stored declarations, capture
viewports and scheduling bounds, carried to the host through UniFFI. Immutable generation folders precede an
atomic record replacement; a late older capture cannot replace newer images. The cache
retains the current and previous generation. Readers own image bytes and retry an
expired pointer. App-side maintenance prunes confirmed missing/replaced files while
retaining cached images when their source is temporarily unavailable.

Each choice has a host-minted ID and bookmark, with local file identity checked on
resolution. Renames and same-volume moves refresh the choice's title, source location and bookmark
without replacing its images or capture time, and request a WidgetKit reload even when
the content fingerprint is unchanged. A cross-volume move drops the choice by design.
Copies get different choices even when their content matches. A
`dotpop://widget/<id>` URL resolves the source and routes through the existing document
coordinator, which focuses an open document. Unavailable files report a native error.

`widget.refreshMinutes` requests approximate capture intervals of 15–1440 minutes.
The running Mac app queries active WidgetKit configurations, deduplicates document IDs,
and serializes due captures using a tolerant background activity. Close captures bypass
that queue and do not rewrite document artwork when only widget images are missing. It skips missed
intervals after sleep. Open documents use the owner save/copy barrier; closed ones use
the read-only snapshot path. Quitting stops captures and file watchers and leaves cached images. WidgetKit
controls visible reload timing. The `pop dev` composition selector previews all three families with HMR. iOS
Home Screen widgets use the shared image cache and canvases, keyed by library document id. The phone
publishes an atomic library index with stored icons for the picker and renders only placed ids,
on saves, close, library version changes and foreground freshness checks. Taps open the library
document by id. Account changes clear the phone cache; Mac bookmarks and file identity stay local
to macOS. `PhoneAppDelegate` registers a `BGAppRefreshTask`. A background lease
refreshes downloaded library documents and placed widgets, with
expiration cancelling work. The app requests a refresh no earlier than 30 minutes later;
iOS decides whether and when it runs, and WidgetKit controls visible reloads. See the
historical [platform spike evidence](evidence/widget-images-2026-10-11.md).

## CLI

`pop` is the user-facing CLI. Its private engine accepts one Rust-owned `EngineRequest`
on bounded standard input and returns one method-specific `EngineReply`. Authoring,
catalog, inspection, named commands and owner operations share this JSON boundary.
`crates/pop-contracts/src/wire/engine.rs` defines the serde types and explicit checks;
ts-rs exports the CLI's TypeScript. The engine renders for itself. `export` of a closed
document on macOS or Linux draws a read-only snapshot of its saved state in the
engine's restricted evaluator, without the app or a lock. It refuses a template and an
existing output of any kind (`rejected`, reason `exists`) before rendering, stages the
result beside the destination, publishes it without replacing anything, and discards one
that finishes after the 30-second command deadline. When the CLI asks, as it does on
macOS and Linux, `pack` renders every declared view from the staged template: Preview,
Icon, Export as PNG and PDF, and all Widget families, even when preview/icon artwork
is supplied. It stores only the Preview and Icon artwork a project does not supply;
a failed view render publishes nothing. Only `open` starts
dotpop.app (macOS only). `open` refuses a template, a link and any file that is not a regular
file; a linked parent folder is fine. LaunchServices (`open -a`) opens
the document in the app that contains the running engine, else the one `POP_APP`
names, else `/Applications` or `~/Applications`.

The engine checks the frozen first-position `--client-protocol N` before other
arguments or stdin: mismatches exit 2 with one stderr line. Identity queries remain
flags, and the engine's exact standalone `--evaluate-command` starts its restricted
child. All handled requests return JSON and exit 0, including classified failures.
Missing, malformed and wrong-method replies leave an unknown outcome; clients never
retry them automatically. App payloads stay opaque until the core checks their format
requirements. The evaluator's Linux syscall allowlist uses `seccompiler`: an architecture
mismatch kills the process, denied calls return `EPERM`, and installation sets
`PR_SET_NO_NEW_PRIVS`.

An open document routes through its owner's socket, which lives as long as the owner,
not the page. Commands never blur a field being typed in; a live `get` returns
owner-accepted state. For a closed document, the engine takes the writer lock (an export
takes none) and runs the same owner in-process, without WebKit or authored code. Socket
work runs off the main actor; edit payloads remain JSON text until the core parses them.

Generic edits print `{ids}` after saving and are never replayed automatically. The socket has
one edit method, `batch`, for data and palette alike; CLI `apply` wraps one operation.
Socket `get` returns `{schema, defaults, version, value, theme}` after flushing: the app's
descriptor and declared colors, and the document's version, value and effective colors.
The page's state adds the publication `sequence`, which orders its stream and means
nothing to an agent. The CLI prints `value` by default and the whole object with
`get --snapshot`. Engine selection and protocol negotiation
are detailed in the [CLI reference](guides/cli.md#engine-discovery-and-identity).

## Themes and attachments

A theme is a palette: the app's theme defaults (`pop.ts`'s `theme`, stored in its `app` row)
declare the colors a person may change, as lowercase `#rrggbb` or `#rrggbbaa` (one
spelling per color), and fonts and derived values stay in the app's CSS. The overrides
live in Loro's root `theme` map, separate from the authored data tree. JSON replacement
and import replace data only, so they preserve the palette. `theme.rs` holds the shared
validation rules: declared tokens, canonical color spelling, 256 tokens and a 64 KiB
effective palette. Opening refuses stored overrides no write could make.

Palette changes are batch intents beside the data ones: `setTheme {values, replace?}`
sets colors (`null` returns one to the template's; `replace` returns every unlisted
one) and `importTheme {file}` replaces the palette with a theme file. The window's theme
panel and agents send them; the page cannot. They advance the same sequence, publish
effective values when changed, enter the undo history and are saved by the same jobs as
data, and a batch may change data and palette together. The window's consecutive
changes to one color are one undo step, so a picker drag undoes at once.

A theme file is `{template, values}`: the app's slug and full effective palette.
Import checks the whole file, refuses another template or undeclared color, and replaces
overrides; omitted colors return to defaults. Export reads the palette then flushes, so
a failed save fails export. The panel follows the document sequence; the page applies
its initial palette from owner state and later palettes from ordered publications.
There is no separate host-to-page theme delivery loop.

Attachments are content-addressed immutable blobs in the file's `attachments` table,
stored on the owner's persistence queue through its own connection, before the edit
that references one is accepted. An agent's blobs arrive in the `batch` that references
them (`pop apply --attach`), so no close falls between a blob and its reference. A blob
is referenced while its ID appears in a string, text or map key of the state
(`Document::attachment_references`). A close after a session that saved an edit or stored
a blob deletes the rest, after the final save; undo covers the open session only, so
history keeps nothing alive.

## Sync

### Synced Library

The [persistent cloud document specification](cloud-documents.md) defines the replacement
for CloudKit and temporary sharing rooms. Firestore is account-scoped discovery; a
Document Durable Object keyed by UUID is the single cloud authority for permissions
and accepted opaque Loro changes. R2 holds immutable snapshots and resources.

A local document requires no account. Enabling sync preserves its UUID and owner and
stages a durable initial upload. Sign-in retrieves metadata; opening downloads the
file. Rust owns validation, local installation, merge/recovery and persistence. Swift
provides Firebase authentication and typed Firestore listener events. Account changes
fence callbacks, flush owners and retain downloaded files while hiding account entries.

The [Apple catalog](apple-catalog.md) uses a device-local Inbox flag and cached nullable
sharing state. Receiving a remote reference does not make it a recent document until
it is opened here. These fields live only in the local library index; they do not
change the `.pop` format or grant cloud access.

### Relay

Private sync and collaboration use the same persistent cloud document. Sharing enables
or revokes an invitation; it never transfers byte authority. Signed-in guests discover
joined documents across devices while the invitation generation is active. Anonymous
guests open links without an account. Revocation fences guest sockets and membership,
while owner sync continues and downloaded guest copies remain usable offline.

Fresh epoch publication remains conditional on the current seed hash and frontier.
An outdated client preserves unconfirmed work in an independent recovery document
before refreshing. Current snapshots and bounded updates replace indefinite archives.
Clients publish their own Firestore references after cloud acknowledgement, with a
small durable local retry queue. Stale references are cleaned up when access is checked.
The Worker never connects to Firestore; the account registry supports discovery repair.


### Hosted browser isolation

The trusted browser host/storage is `share.dotpop.click`; authored apps run on isolated
UUID subdomains of `dotslop.com`. Service Worker channels deliver local resources;
host-scoped, partitioned HttpOnly cookies authorize hosted resources. SQLite uses OPFS
in the trusted worker; IndexedDB stores its local-copy catalog. Authored pages receive
the SDK bridge, never invitation credentials. Files are verified and installed before
updates apply; a failed transfer cannot acknowledge catch-up. Browser copies remain
editable offline and exportable after access is revoked.

### Native authentication

Mac and iPhone builds enable live collaboration without a launch flag. Mac Release
entitlements include the sharing Keychain group and the `share.dotpop.click` associated
domain. Developer ID apps cannot use native Sign in with Apple, so Mac sign-in uses
`ASWebAuthenticationSession` with Services ID `com.hitslop.app.signin`, grouped under
primary App ID `com.hitslop` alongside the iPhone app. iPhone retains native
`ASAuthorizationController`; both paths sign in to the same Firebase account. The Mac
requests no scopes and uses a fragment response at `https://share.dotpop.click/auth/apple`.
A static page forwards the unchanged fragment to `dotpop://auth/apple`; tokens never
reach the host. Only the active session consumes it, checks its random state and supplies
the ID token and raw nonce to Firebase. The callback page has a script-hash CSP,
`no-store` and `no-referrer`.

The sharing host retains its `/join` universal-link route, Apple callback and AASA
response. Links identify a document UUID and invitation capability. The browser
resolves the document service directly and does not require Firestore to render it.

## Tests and performance

Tests live at the boundary that owns the behavior; see [testing](testing.md).

Performance reports in [`evidence/`](evidence/) identify their producing build and
measurement conditions. Older reports are historical baselines; they do not establish
current startup, edit latency or memory behavior. Use the diagnostics in
[testing](testing.md#native-macos) to measure the candidate being reviewed.

## Fullscreen windows

The optional `window.fullscreenable` declaration is Rust-owned and defaults false for
both standard and skin windows. Native fullscreen keeps the existing owner and WebView.
Rust projects whether the authored composition must fit (fixed-size, explicit shape,
aspect lock or skin); other standard windows reflow to the viewport. Fitted content
retains its mask and coordinate system inside an opaque black fullscreen surface.
The host restores desktop frame, constraints and level on exit. Fullscreen state is
transient host state; captures keep their independent saved-state layout. Browser
fullscreen follows the manifest flag. Supported browser scope is described above;
remaining platform and sharing work is tracked in the [roadmap](roadmap.md#open-now).
