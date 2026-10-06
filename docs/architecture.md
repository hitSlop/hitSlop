# Architecture

hitSlop documents are local files (`.slop`): one SQLite database that pairs an immutable
authored app with a structured document. One Rust core, `hitslop-core` on Loro, owns
document semantics, durable storage and the live owner: the file, writer lock, edit queue
and save scheduler, and it answers the page's document requests. Swift `DocumentOwner`
passes them through, answers the window's own, and delivers owner events to the page;
Loro bytes never reach Swift. The WebView renders immutable snapshots and holds no CRDT.

```text
page (WebKit) ── requests ──▶ Swift DocumentOwner façade ──▶ Rust owner (edit worker)
              ◀──────────── ordered publications ─────────────────────┤
                                                                     ▼
                                                        persistence worker ──▶ .slop

slop ── slop-engine ── Rust command router ──▶ same Rust owner
                         live: owner's socket; closed: acquire writer lock
                     └── hitslop-native: open windows / render saved-state exports and artwork
```

Descriptor kinds: text (merging); boolean, string, number, integer and enum (last
writer wins, checked on write); optional (of a scalar, text or an object); object;
list of object rows with `$id`; list of scalars by index; record of scalars or objects
by key; and counter. [Document types](reference/document-types.md) describes each
kind's snapshot, merge, write rules, handles and CLI paths.

The same core compiles to WASM for `slop dev` and Bun tests. Authoring checks send the
bounded app input to the native engine's `validate-app` command, sharing the checks used
when packing and opening files.
The WASM build never edits a durable document. Authoring (`init`, `check`, `dev`, `build`)
needs no helper and runs on macOS and Linux: the build stages the compiled app and its
evaluated descriptor, initial values and theme, and the CLI's file engine
(`crates/slop-engine`, the same core built natively) packs the stage into a template,
checking it as the app opens it; text assets are stored compressed, since every document
carries its own copy of the app. Only `--artwork native` and `register` render artwork
with the app, from a draft of the template. The CLI and the app update separately: the
CLI names its command protocol to the engine, and the engine names it in every request
to a live owner; a side that does not serve it refuses and says which to update. Document
commands use the CLI's own `slop-engine` on every platform. The engine passes what needs
AppKit or WebKit (exports, windows, native artwork) to the app's rendering helper.
The app and helper link the same core; the engine ships only in the npm package.
Saved documents carry what they need to be read: the
file's `packageFormat` and `runtimeABI` requirements (`app` columns), the SQLite
application ID and storage version, and the document's layout (`meta.layout`). A build refuses a newer one with `requires_update`;
[compatibility](engineering-contract.md#compatibility) has the rules, and the
[compatibility corpus](testing.md#compatibility-corpus) replays every release's
documents.

Manifest acceptance is native-only Rust validation of the TypeBox-generated manifest
schema (newer `packageFormat` or `runtimeABI` requirements are refused first), followed
by the shared shape parser. SVG syntax uses `svgtypes::PathParser`; `kurbo` expands arcs
through a lazy iterator, with source-command and output-segment limits enforced by the
core. The authored path/viewBox/fillRule and the normalized native silhouette representation
stay the same. Swift decodes the validated manifest into its generated model
and decodes the window skin. The native engine validates the complete authoring app;
the manifest validator dependency is excluded from WASM. Packing validates descriptors, initial values
and theme defaults. Every open checks the file before reading a value: its application
ID, storage version and markers, then its exact tables, one `app` row, the sizes of
every value and asset, and the asset paths. Then the app is checked once (manifest and
window shape, descriptor and initial values, theme, `app.js` and skin), and the open keeps
what it found. An owner's store open is its document's one check, adding SQLite's quick
check; the session shows the document from it. Display-only opens (the catalog, Quick
Look) skip the quick check, and a host deciding how to open a file reads only its kind
from the header. A template opened as a document is refused with `is_template`. The core validates socket and page envelopes against the same generated
schemas (`Envelope`); Swift only serializes them for that check and maps accepted values.
Document payloads never need that: page batches and text edits, and CLI operations, cross
as JSON text that only the core parses, and state returns as the core's JSON text, spliced
into replies unparsed. All page methods use one `{ok, ...result}` or `{ok: false, code, error}` reply
contract. The core answers the document requests (open, edits, text, undo, flush,
attachments) and the window answers its own (config, readiness, resizing, errors).
WebKit correlates replies; native view tokens fence retired pages without page-carried
identity. Shared limits and codes live in TypeBox-free
`packages/hitslop/src/schema/constants.ts` and are generated into Rust and Swift.

## Layers

| Layer | Where | Owns |
|---|---|---|
| Core | `crates/hitslop-core` | Descriptors, validation, `$id` rows, atomic batches, the palette, publications, counters, text merges, frontier version tokens, window-shape geometry (`shape`, Loro-free) |
| File | `crates/hitslop-core/src/file/` (feature `storage`, native only) | The `.slop` file's layout, every statement on its tables (every write in `rows`) and the checks every open runs; pack, create, copy; where documents may live; the app's assets and artwork |
| Storage | `crates/hitslop-core/src/{store,registry}.rs` | Saved Loro state, including theme overrides, on the platform SQLite, attachments (each committed in its own transaction before an edit references it), append-or-checkpoint choice, size limits, identity checks; the writer lock and discovery in the registry |
| Engine | `crates/slop-engine` | Authoring validation and packing; document creation, inspection, and requests on macOS and Linux |
| Commands and socket | `crates/hitslop-core/src/{command,socket}.rs` | Typed command dispatch, writer admission, live-owner routing, handshake, framing and deadlines |
| Adapters | `crates/hitslop-core-{ffi,wasm}` | Records and typed errors (`Rejected`, `Invalidated`, and the storage failures); no semantics |
| Owner | `crates/hitslop-core/src/owner.rs` | Serial edit and persistence workers, save scheduling, view tokens, discard and close, and the page's document requests (`command.rs`); Swift is a typed façade that keeps what the owner's events tell it |
| Session | `HitSlopDocument/DocumentSession.swift` | WebView, the `hitslop` message handler, the push queue, native export callback and the lifetime of the Rust socket server; the window is its `DocumentSessionDelegate` |
| Window | `HitSlopHost/SlopWindow.swift` | How a document looks: shape, toolbar, pin level, theme panel, page-failure overlay, the save-failure sheet (from the owner's save status); its document operations go to the app |
| Quick Look | `apps/apple/App/QuickLook{Thumbnail,Preview}` | Finder, Mail and share-sheet thumbnails and previews from the file's artwork, read through the core in a sandbox |
| App | `HitSlopFeatures` (`AppModel`, `CommandQueue`, `CatalogModel`), `HitSlopCatalog/SlopApplicationCoordinator.swift` | Opening, one document operation at a time per document (a close or a save recovery requested meanwhile runs next), quit, the catalog, and alerts for failures that are not save failures |
| Author SDK | `packages/hitslop/src/sdk` | Descriptors, public types, errors, Svelte adapter; no host runtime |
| Page shell | `packages/hitslop/src/shell` (served at `/__shell__/`) | Store, handles, text binding, write queue, barrier, attachments, theme application |
| Contracts | `packages/hitslop/src/schema` (TypeBox) | Manifest, core wire, page protocol, socket; `bun run schema:generate` emits Rust and Swift |

## An edit

1. **Page.** A handle write (`set`, `insert`, `remove`, `move`, `increment`) or a
   `change(tx => …)` collector becomes one batch. Batches go through one FIFO queue, so an
   `insert` followed by a `move` cannot reorder. A scalar `set`, `clear` or assigned
   `value` also shows at once as a local preview over the snapshot; acceptance settles
   it and a refusal reverts it. Assigned values commit after 150 ms without another
   assignment, or at the next barrier.
2. **Host.** The page posts `apply {batch}`, the batch as JSON text. The native session checks
   the sending WebView, frame and origin and hands the request, with its view token, to
   the core's page dispatcher, which checks the envelope and the token before calling
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
DOM text and their confirmed version, which stays valid because versions name owner
history.

## Text

The page and agents change text the same way: a `set` in a batch that names a `base`
version means "this field was `from` at `base` and is now `value`". A text binding keeps
the user's text in the field and sends at most one such batch at a time,
`{base, intents: [{type: "set", path, value, from, selection}]}`. An agent passes the
version it read (`--base`); its set has no `from`, so the core reads the field's text at
`base`.

- The core computes the edit script on a throwaway document (its diff mutates while it
  runs, so it never touches the owner) and checks that the script reproduces the value.
- **Fast path:** the owner's field still equals `from`, so the script applies directly.
- **Merge path:** the field changed since `base`. The script is applied on a branch at
  `base` (a state-only copy, which trimmed documents allow) and merged with Loro; the
  caret is mapped through cursors. A field whose container `base` never saw (a row
  removed and inserted again) is `path_not_found`.
- A set carrying `selection` is the page's edit and its batch's only intent. The reply
  names `authored`, the version right after this edit on its own branch, and the merged
  selection. If the user kept typing, the next batch goes from the sent text at
  `authored`.
- During IME composition nothing is sent. Close and export commit a composition.
  Retargeting or unmounting a binding sends its unsent text first.
- Every token is checked against the document's history before Loro sees it: a
  malformed one is `invalid_version` and a foreign or trimmed one `stale_base`, never a
  panic.

A text set without a base (a text handle's `set(value)`, or the CLI without `--base`)
replaces the whole field as it is when the owner applies it, through the same script.

Merging works at three levels:
- **Characters.** Loro gives each character an identity. A concurrent insert lands next
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

Edit ▸ Undo and Redo revert changes made since the document opened: the person's, and
an agent's made through the CLI or socket. A document opens with nothing to undo. The
core keeps up to 100 steps per open document, each holding Loro frontiers before and
after the edit. Undo and redo restore either version as a new change with Loro's
`revert_to`, which inverts the operations between the two versions (the palette with the
data) and applies all of them or none. A restored row keeps its `$id`. There are no JSON
snapshots or persistent undo records.

- **Steps.** Each page batch is a step. A typing run is one step: consecutive text edits
  to one field, each starting at the caret the last one left. Consecutive agent batches
  are one step, so one undo reverts what the agent just did. Consecutive window changes
  to one palette color (a picker drag) are one step. Any other change, a
  concurrent page text merge or undo/redo itself ends a run. The concurrent text edit
  is its own step, even though the text implementation imports a temporary branch.
- **Refusals.** A batch or JSON replacement refused after a partial mutation rebuilds
  the owner at its pre-call version. Its history references survive replay, so undo,
  redo and the current run remain available. No-op edits also preserve history.
- **Result.** An undo is a new change: it publishes, autosaves and travels like any
  other. Nothing to undo publishes nothing.
- **Window.** The document window's `NSUndoManager` (`DocumentUndoManager`) reports
  whether undo and redo are available and sends them to the page, which first sends
  unsent text, queued writes and previews (`__slop.undo`, `doc.undo()`), then asks the
  owner. WebKit's own text-editing undo still registers there, as AppKit's grouping
  requires, but is never performed. Without a live page the owner undoes directly.
- **Browser.** In `slop dev`, a text field's own undo (`beforeinput` `historyUndo`) is
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
  or discard for a full document.
- `flush` resolves when the saved sequence covers every edit accepted before the call.
  `close` refuses new edits, flushes, trims history (below), writes the artwork its
  window captured ([close](#close-export-and-capture)), reclaims attachments nothing
  references ([attachments](#themes-and-attachments)), then releases the lock.
  `discard` waits for the write in flight and reloads saved bytes; work queued for the
  discarded state (a waiting flush, a page that was replaced) is refused with
  `owner_replaced`.
- Save status flows one way, owner to window: the edited mark and the failure sheet. The
  page learns durability only through `flush`, which resolves once saved and rejects
  when the save fails.

Storage is `document(id)`, `checkpoint(bytes)` and `updates(seq, bytes)`. Creating a
document atomically writes its identity and initial checkpoint before publishing the file. Saved updates without a checkpoint are
refused and preserved for recovery. A checkpoint replaces the log at 256 updates or
4 MiB; the limits are 4,096 updates and 32 MiB (`StorageLimits`).

History is trimmed when nothing is editing. After its final save, a session that edited
a document larger than 4 MiB writes one more checkpoint (`Store::close_job`) that keeps
no history: undo covers the open session only, so nothing reads it later. While open, a checkpoint over 16 MiB keeps the session's
history when that fits and none otherwise, so a concurrent text edit can still branch
from where the session opened. There is no public live-compaction command. Only the checkpoint may
start history late. Rollback rebuilds from where history starts; a version before it
is `stale_base`, and a concurrent text edit never branches from before the latest cut,
so no saved update depends on trimmed history. Files
use full auto-vacuum: every commit returns the pages it freed (the log a checkpoint
replaces, the artwork a close replaces), so a file holds no dead space. The store links the platform SQLite, the one library every other in-process user
loads, and the core is the only code that opens a `.slop` file.

## The file, its lock and copies

A template holds an app; a document holds an app and its saved state. Creating a
document copies a template and adds its identity and initial Loro checkpoint in one
transaction; nothing is ever unpacked. The core decides where a document may live, for
the app and the engine alike: opening one for writing needs a `.slop` name outside iCloud
Drive, and creating or copying one also refuses the installed and bundled templates
(`file::document_location`, `file::document_destination`). Swift adds only what
Foundation alone can tell, a folder iCloud syncs such as Desktop and Documents. The writer
lock is an `flock` on a registry file outside the document (`~/.hitslop/live`, named by
the file's device and inode), never on the database: closing any second descriptor on a
SQLite file drops SQLite's own locks. The lock holder publishes discovery beside it and
removes a crashed owner's. A rename stops the writer (`Moved`): SQLite names its journal
after the path, and Apple's SQLite never writes again through a connection whose file
was renamed. When the file is back where it was opened, the store reconnects and saves.

Duplicate and Share a Copy flush what the page accepted, render artwork from the saved
state, then copy from the owner's own connection with SQLite's online backup, so saves
queue behind the copy. The copy is made a document of its own before it is published
(`Store::copy_clean`): its saved state becomes a checkpoint with no history, it keeps
only the attachments that state references, and it carries the rendered artwork, or none
if rendering failed, never the original's. Nothing deleted before the copy is in it, and
writers zero deleted content (`secure_delete=FAST`) on every platform. The copy is
published without replacing anything. A capture's source is a plain backup, rendered
once and deleted. A window writes the file's
artwork as it closes ([close](#close-export-and-capture)). Finder, Mail and the share sheet
show it through the app's Quick Look extensions, which read the file's artwork read-only;
a file without artwork shows the `.slop` document icon.

## Close, export and capture

The page barrier sends unsent text, waits for queued writes and attachment imports, then
flushes; it never joins a flush that already passed its drain point. Inputs stay
enabled, so focus survives a cancelled barrier. Export, preview and icon rendering use
fresh read-only pages from saved state. For an open document, the core first backs up
SQLite after the page drain and save; the temporary copy owns its app, attachments,
data and theme independently of the editor's lifetime. No read transaction spans
WebKit rendering. A dedicated `Export.svelte` hides the editor before layout; without
one, the fresh App renders its default local UI state. Capture never changes the open
editor's focus, selection, scroll, frame or selected tab.

If the session changed the document, or the file has no preview, close captures its
preview and optional icon from that saved source. The owner writes successful artwork
through its writer connection and releases the lock. Finder's custom icon reflects the
saved artwork. A failed capture keeps old artwork and does not stop close; a failed save
keeps the window and ownership for retry. An attachment import stores the blob, then submits its reference through a
collector admitted past an active barrier. A collector that throws, or an edit the core
refuses, leaves the blob unreferenced until the document closes.

## CLI

`slop` is the user-facing CLI. Its private engine accepts one TypeBox `EngineRequest`
on bounded standard input and returns one method-specific `EngineReply`. Authoring,
catalog, inspection, named commands and owner operations share this JSON boundary.
`packages/hitslop/src/schema/engine.ts` generates the Rust dispatch types and validators.
The engine forwards open, screenshot and export JSON to `hitslop-native`; its generated
`NativeRequest` subset rejects document edits before starting AppKit. Exports still use
the Rust command router, so the live owner or a closed document's renderer handles them.

Both executables check the frozen first-position `--client-protocol N` before other
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
owner-accepted state. For a closed document, the engine takes the writer lock and runs
the same owner in-process, without WebKit or authored code. Closed exports use a
read-only saved-state renderer. Socket work runs off the main actor; edit payloads
remain JSON text until the core parses them.

Generic edits print `{ids}` after saving and are never replayed automatically. The socket has
one edit method, `batch`, for data and palette alike; CLI `apply` wraps one operation.
Socket `get` returns `{schema, defaults, version, value, theme}` after flushing: the app's
descriptor and declared colors, and the document's version, value and effective colors.
The page's state adds the publication `sequence`, which orders its stream and means
nothing to an agent. The CLI prints `value` by default and the whole object with
`get --snapshot`. Engine selection and protocol negotiation
are detailed in the [CLI reference](guides/cli.md#helper-discovery-and-identity).

## Themes and attachments

A theme is a palette: the app's theme defaults (`slop.ts`'s `theme`, stored in its `app` row)
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

A theme file is `{template, values}`: the manifest slug and full effective palette.
Import checks the whole file, refuses another template or undeclared color, and replaces
overrides; omitted colors return to defaults. Export reads the palette then flushes, so
a failed save fails export. The panel follows the document sequence; the page applies
its initial palette from owner state and later palettes from ordered publications.
There is no separate host-to-page theme delivery loop.

Attachments are content-addressed immutable blobs in the file's `attachments` table,
stored on the owner's persistence queue through its own connection, before the edit
that references one is accepted. An agent's blobs arrive in the `batch` that references
them (`slop apply --attach`), so no close falls between a blob and its reference. A blob
is referenced while its ID appears in a string, text or map key of the state
(`Document::attachment_references`). A close after a session that saved an edit or stored
a blob deletes the rest, after the final save; undo covers the open session only, so
history keeps nothing alive.

## Tests and performance

Tests live at the boundary that owns the behavior; see [testing](testing.md).

Performance evidence is in [`evidence/`](evidence/). At 1,000 rows a window opens in under
a second and a checkbox is accepted in about 12 to 14 ms (p95); see
`release-window-measurements-2026-09-30.json`. Publication cost from owner commit to page
at 1,000 and 5,000 rows is in `codebase-pass-phase2-2026-10-01.json`, edit latency at
5,000 rows in `edit-latency-2026-10-01.json`, and core keystroke cost at 10,000 and
100,000 characters (core only, not a system IME) in `long-text-2026-09-30.json`. A theme
panel color drag reaches the page within a frame at 1,000 rows (`theme-drag-2026-10-02.json`).
