# Architecture

hitSlop documents are local files (`.slop`): one SQLite database that pairs an immutable
authored app with a structured document. One Rust core, `hitslop-core` on Loro, owns
document semantics and durable storage: the file, the writer lock and the save policy. The Swift
`DocumentOwner` holds the live core, schedules saves and delivers to the page; Loro bytes
never reach Swift. The WebView renders immutable snapshots and holds no CRDT.

```text
            page (WebKit)                               host (Swift)                  core (Rust)
 Svelte app ─ ctx ─ shell store/handles ── open/apply/text/flush ──▶ DocumentOwner ── UniFFI ──▶ hitslop-core
            ◀────────── ordered pushes (__slop.publish) ────────┘   │ owner queue                (Loro)
                                                                        ▼
 CLI (slop / hitslop-native) ── socket (live) or writer lock (closed) ─┘ persistence queue ──▶ the .slop file
```

Descriptor kinds: text (merging); boolean, string, number, integer and enum (last
writer wins, checked on write); optional (of a scalar, text or an object); object;
list of object rows with `$id`; list of scalars by index; record of scalars or objects
by key; and counter. [Document types](reference/document-types.md) describes each
kind's snapshot, merge, write rules, handles and CLI paths.

The same core compiles to WASM for `slop dev`, for `slop build`'s validation of
descriptors, initial values, theme defaults and window shapes, and for the Bun tests.
It never edits documents outside the app. Authoring (`init`, `check`, `dev`, `build`)
needs no helper and runs on macOS and Linux: the build stages the compiled app and its
evaluated descriptor, initial values and theme, and the CLI's file engine
(`crates/slop-engine`, the same core built natively) packs the stage into a template,
checking it as the app opens it. Only `--artwork native` and `register` render artwork
with the app, from a draft of the template. The CLI and the app update separately: the CLI
checks that the selected helper serves its command protocol (`hitslop-native
--protocol`) before document commands and native artwork, and the helper checks that the live owner has its exact core build, since
both ship in one app bundle. Saved documents carry what they need to be read: the
file's `packageFormat` and `runtimeABI` requirements (`app` columns), the SQLite
application ID and storage version, and the document's layout (`meta.layout`). A build refuses a newer one with `requires_update`;
[compatibility](engineering-contract.md#compatibility) has the rules, and the
[compatibility corpus](testing.md#compatibility-corpus) replays every release's
documents.

Manifest acceptance is native-only Rust validation of the TypeBox-generated manifest
schema (newer `packageFormat` or `runtimeABI` requirements are refused first), followed
by the shared shape parser. Swift decodes the validated manifest into its generated model
and decodes the window skin. Authoring keeps TypeBox manifest validation; the manifest
validator dependency is excluded from WASM. Packing validates descriptors, initial values
and theme defaults. Every open checks the file before reading a value: its application
ID, storage version and exact tables, one `app` row, the markers, the sizes of
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
contract. WebKit correlates replies; native view tokens and socket epochs fence
retired sessions without page-carried identity. Shared limits and codes live in TypeBox-free
`packages/schema/src/constants.ts` and are generated into Rust and Swift.

## Layers

| Layer | Where | Owns |
|---|---|---|
| Core | `crates/hitslop-core` | Descriptors, validation, `$id` rows, atomic batches, publications, issues, counters, text merges, frontier version tokens, window-shape geometry (`shape`, Loro-free) |
| File | `crates/hitslop-core/src/file.rs` (feature `storage`, native only) | The `.slop` file's layout and the checks every open runs; pack, create, copy; the app's assets and artwork |
| Storage | `crates/hitslop-core/src/{store,registry}.rs` | Saved state, theme overrides and attachments on the platform SQLite (saved by the same jobs), append-or-checkpoint choice, size limits, identity checks; the writer lock and discovery in the registry |
| Engine | `crates/slop-engine` | The CLI's file tool on any platform: `pack`, `inspect`, `schema` |
| Adapters | `crates/hitslop-core-{ffi,wasm}` | Records and typed errors (`Rejected`, `Invalidated`, and the storage failures); no semantics |
| Owner | `HitSlopDocument/DocumentOwner.swift` | Owner queue (core calls, save jobs), persistence queue (store calls), save scheduling, epochs, view tokens |
| Session | `HitSlopDocument/DocumentSession.swift` | WebView, the `hitslop` message handler, the push queue, socket and discovery; the window is its `DocumentSessionDelegate` |
| Window | `HitSlopHost/SlopWindow.swift` | How a document looks: shape, toolbar, pin level, page-failure overlay, the save-failure sheet (from the owner's save status); its commands go to the app |
| Quick Look | `apps/apple/App/QuickLook{Thumbnail,Preview}` | Finder, Mail and share-sheet thumbnails and previews from the file's artwork, read through the core in a sandbox |
| App | `HitSlopFeatures` (TCA), `HitSlopCatalog/SlopApplicationCoordinator.swift` | Opening, one command at a time per document (a close or a save recovery requested meanwhile runs next), quit, and alerts for failures that are not save failures |
| Author SDK | `packages/document` | Descriptors, public types, errors, Svelte adapter; no host runtime |
| Page shell | `packages/shell` (served at `/__shell__/`) | Store, handles, text binding, write queue, barrier, attachments, theme application |
| Contracts | `packages/schema` (TypeBox) | Manifest, core wire, page protocol, socket; `bun run schema:generate` emits Rust and Swift |

## An edit

1. **Page.** A handle write (`set`, `insert`, `remove`, `move`, `increment`) or a
   `change(tx => …)` collector becomes one batch. Batches go through one FIFO queue, so an
   `insert` followed by a `move` cannot reorder. A scalar `set`, `clear` or assigned
   `value` also shows at once as a local preview over the snapshot; acceptance settles
   it and a refusal reverts it. Assigned values commit after 150 ms without another
   assignment, or at the next barrier.
2. **Host.** The page posts `apply {batch}`, the batch as JSON text. The native session checks
   the sending WebView, frame and origin and supplies its view token. The owner job
   checks that token before calling `apply_batch`. The reply is
   `{sequence, ids}`. A batch that changes nothing publishes nothing and leaves the
   document clean.
3. **Push.** The core's publication, `{previous, sequence, version, ops, issues?}`, is
   appended to the session's push queue on the owner queue, so pushes keep owner order.
   One drain at a time delivers everything buffered through a single awaited
   `__slop.publish(pushes)` call. Swift never parses publications. An edit to an
   existing text field publishes a `text` op with its hunks (retain, insert, delete, in
   code points of the previous text), not the whole field. `issues`, the complete list,
   is present only when it changed.
4. **Store.** The page applies publications in sequence order, ignores any at or below
   its sequence, and copies only the objects on the changed paths; unchanged rows keep
   their identity. The write's promise resolves once the store reaches the reply's
   sequence, so the snapshot has updated when `await` returns.

Issues name rows by their effective `$id`, as edits and the snapshot do. The core keeps
them current by recomputing only the places a change touched (a map entry, or a list
whose rows changed); a full recomputation from the stored value remains the test oracle.

A gap (`previous` above the store's sequence) or an apply failure makes the page call
`open` again and replace its state. Text still in a field survives: bindings keep their
DOM text and their confirmed version, which stays valid because versions name owner
history.

## Text

A text binding keeps the user's text in the field. It sends at most one request at a
time: `text {base, path, from, to, selection}`, meaning "this field was `from` at
`base` and is now `to`".

- The core computes the edit script on a throwaway document (its diff mutates while it
  runs, so it never touches the owner) and checks that the script reproduces `to`.
- **Fast path:** the owner's field still equals `from`, so the script applies directly.
- **Slow path:** the field changed concurrently. The script is applied on a branch at
  `base` (a state-only copy, which trimmed documents allow) and merged with Loro; the
  caret is mapped through cursors.
- The reply names `authored`, the version right after this edit on its own branch. If the
  user kept typing, the next request goes from the sent text at `authored`.
- During IME composition nothing is sent. Close and export commit a composition.
  Retargeting or unmounting a binding sends its unsent text first.
- Every token is checked against the document's history before Loro sees it, so a
  malformed or foreign base returns `stale_base`, never a panic.

A text handle's `set(value)` and the CLI's `set` replace the whole field as it is when
the owner applies it, through the same precomputed script.

## Undo

Edit ▸ Undo and Redo revert changes made since the document opened: the person's, and
an agent's made through the CLI or socket. A document opens with nothing to undo. The
core keeps up to 100 steps per open document, each holding Loro frontiers before and
after the edit. Loro's `revert_to` restores either version as a new change; there are no
JSON snapshots or persistent undo records.

- **Steps.** Each page batch is a step. A typing run is one step: consecutive text edits
  to one field, each starting at the caret the last one left. Consecutive agent batches
  are one step, so one undo reverts what the agent just did. Any other change, a
  concurrent page text merge or undo/redo itself ends a run. The concurrent text edit
  is its own step, even though the text implementation imports a temporary branch.
- **Refusals.** A batch or JSON replacement refused after a partial mutation rebuilds
  the owner at its pre-call version. Its history references survive replay, so undo,
  redo and the current run remain available. No-op edits also preserve history.
- **Replica imports.** A raw replica import that adds operations clears the available
  steps, so whole-document undo cannot erase external changes. Duplicate and refused
  imports that add nothing leave history alone. This boundary does not apply to JSON
  import or page typing. Selective undo across remote changes is deferred with collaboration.
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
  updates since the last save, or a checkpoint. The persistence queue runs every store
  call, including the write. At most one write is in flight, and edits during it
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
  `close` refuses new edits, flushes, trims history (below), then releases the lock.
  `discard` rotates the epoch,
  waits for the write in flight, and reloads saved bytes; requests captured before it are
  refused with `owner_replaced`.
- Save status flows one way, owner to window: the edited mark and the failure sheet. The
  page learns durability only through `flush`, which resolves once saved and rejects
  when the save fails.

Storage is `document(doc_id, theme)`, `checkpoint(schema_key, bytes)` (absent until
the first save), and `updates(seq, bytes)`. Saved updates without a checkpoint are
refused and preserved for recovery. A checkpoint replaces the log at 256 updates or
4 MiB; the limits are 4,096 updates and 32 MiB (`StorageLimits`).

History is trimmed when nothing is editing. After its final save, a session that edited
a document larger than 4 MiB writes one more checkpoint (`Store::close_job`) that keeps
no history: undo covers the open session only, and Loro 1.16.2 keeps everything deleted
before a cut in the cut's starting state, so only a cut at the latest version reclaims a
document that deletes a lot. While open, a checkpoint over 16 MiB keeps the session's
history when that fits and none otherwise, so a concurrent text edit can still branch
from where the session opened; `compact` trims to the latest version. Only the checkpoint may
start history late. Rollback rebuilds from where history starts; a version before it
is `stale_base`, and a concurrent text edit never branches from before the latest cut,
so no saved update depends on trimmed history. New
databases use incremental auto-vacuum, and every checkpoint frees the pages the log
used. The store links the platform SQLite, the one library every other in-process user
loads, and the core is the only code that opens a `.slop` file.

## The file, its lock and copies

A template holds an app; a document holds an app and its saved state. Creating a
document copies a template and gives it an identity; nothing is ever unpacked. The writer
lock is an `flock` on a registry file outside the document (`~/.hitslop/live`, named by
the file's device and inode), never on the database: closing any second descriptor on a
SQLite file drops SQLite's own locks. The lock holder publishes discovery beside it and
removes a crashed owner's. A rename stops the writer (`Moved`): SQLite names its journal
after the path, and Apple's SQLite never writes again through a connection whose file
was renamed. When the file is back where it was opened, the store reconnects and saves.

Duplicate and Share a Copy flush what the page accepted, then copy from the owner's own
connection with SQLite's online backup, so saves queue behind the copy. The copy gets a
new identity and is published without replacing anything. Artwork rendered from a closed
document's saved state is written into the file only while it still holds that state.
Finder, Mail and the share sheet show it through the app's Quick Look extensions, which
read the file's artwork read-only; a file without artwork shows the `.slop` document icon.

## Close, export and capture

The page barrier sends unsent text, waits for queued writes and attachment imports, then
flushes; it never joins a flush that already passed its drain point. Inputs stay
enabled, so focus survives a cancelled barrier. Swift then saves and closes the owner.
An attachment import stores the blob and submits its reference through a collector
admitted past an active barrier, so a blob is never saved without its reference.

## CLI

`slop` forwards to `hitslop-native`. If the document is open, the command goes to the
owner's socket, which lives as long as the owner, not the page. Commands run on the owner
directly: they never blur the field being typed in, and a live `get` returns
owner-accepted state. If the document is closed, the helper takes the writer lock and
runs the owner in process, without WebKit or authored code. Socket commands run off the
main actor, and operations travel as JSON text. Edits print `{ids, sequence, value}`;
mutations are never replayed automatically. The socket has one edit method, `batch`;
CLI `apply` validates and wraps one operation. Socket `get` returns `{schema, state}`
from one owner frame after flushing. The CLI prints `state.value` by default and the
complete payload with `get --snapshot`.

## Themes and attachments

A theme is a palette: the app's theme defaults (`slop.ts`'s `theme`, stored in its `app` row)
declare the colors a person may change, as lowercase `#rrggbb` or `#rrggbbaa` (one
spelling per color), and fonts and derived values stay in the app's CSS. The overrides
live in the file's `document` row, outside Loro.
`theme.rs` holds every rule (declared tokens, the color spelling, 256 tokens, a 64 KiB
effective theme) as hand-written checks, so authoring validation in WASM and native
writes share them. The store keeps the palette in memory from `document` on: a change
(set, reset or import) is accepted on the edit queue like an edit, drops overrides equal
to their default, and is saved by the next save job in the same transaction as any Loro
updates, so flush, close, retry and Duplicate cover it. A snapshot reads the overrides
with its rows. Socket theme commands reply once saved, as `batch` does. Checking
ownership reads a flag, never the mutex a save holds, so a theme change never waits for one.

A theme file is `{template, values}` (`ThemeFileSchema`): the manifest slug and the full
effective palette. Import validates the whole file first, refuses another template or an
undeclared color, and replaces the overrides; colors it leaves out return to their
defaults. Export reads the palette and then flushes, so a failing save fails the export. The window's theme panel (`SlopThemePanel.swift`) is a child window beside the
document; it changes the palette through `DocumentSession.changeTheme`, in the order
changes are made, and follows CLI and agent changes through `themeChanged`. The page only
applies effective values: config includes the initial theme, later deliveries are
serialized, and `flush` waits until the page shows the latest palette, so an export
captures it. A capture in progress is never restyled.

Attachments are content-addressed immutable blobs in the file's `attachments` table,
stored on the owner's persistence queue through its own connection, before the edit
that references one is accepted.

## Tests

Tests live at the boundary that owns the behavior; see [testing](testing.md).

| Boundary | Proves |
|---|---|
| Rust (`crates/hitslop-core/tests`) | Semantics, publications equal fresh snapshots, text merges (`text.rs`), token validation (`tokens.rs`), storage (`store.rs`) |
| SDK over WASM (`packages/shell/tests`) | Write timing, snapshot identity, collectors, text binding, stream recovery, barriers, attachments |
| Swift (`apps/apple/Packages/HitSlopApple/Tests`) | Persistence scheduling, lost replies, view and epoch fences, CLI, WebView bridge, export |

Performance evidence is in [`evidence/`](evidence/). At 1,000 rows a window opens in under
a second and a checkbox is accepted in about 12 to 14 ms (p95); see
`release-window-measurements-2026-09-30.json`. Publication cost from owner commit to page
at 1,000 and 5,000 rows is in `codebase-pass-phase2-2026-10-01.json`, edit latency at
5,000 rows in `edit-latency-2026-10-01.json`, and core keystroke cost at 10,000 and
100,000 characters (core only, not a system IME) in `long-text-2026-09-30.json`. A theme
panel color drag reaches the page within a frame at 1,000 rows (`theme-drag-2026-10-02.json`).
