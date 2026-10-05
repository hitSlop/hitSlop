# SQLite format review and optimization plan

Status: proposed, 2026-10-05; revised the same day after review. Implementation has not
started. This plan assumes breaking pre-launch changes are allowed: `tests/compat` holds
only `dev`, and nothing is frozen. It does not authorize changing released documents or
frozen corpus entries.

Keep the single SQLite file, Rust ownership and Loro semantics. The format's real
problem is what a copied `.slop` carries: deleted text, removed attachments and stale
artwork. Fix that first, sharing one clean-copy step with
[share links](share-links-and-browser-host.md#publication-artifact). Then make the tables
`STRICT`, shrink artwork, and reclaim unreferenced attachments once the CLI can no longer
leave a blob waiting for its reference across owner sessions. Keep 4 KiB pages.

## Evidence

The comparison covered the working tree, the local
[uApp checkout](../_docs/uApp-main/README.md) (reference material, may be absent) and one
Capsule artifact, `~/Desktop/Recipes.capsule`. It supports three conclusions:

- Keep Loro. uApp gives authors SQL tables, and Capsule stores JSON records. hitSlop
  supplies descriptor validation, stable row identities, text merging, ordered
  publications and undo through one engine. Replacing it is not a container
  optimization, and persistent SQL or JSON mirrors of Loro stay out. The CLI remains the
  semantic inspection path.
- Brotli asset compression already captures what Capsule's HTML would gain: its
  65,471-byte HTML compresses to 11,701 bytes with zlib.
- `STRICT` tables are the one idea worth borrowing. Capsule's `WITHOUT ROWID` metadata
  tables and uApp's `journal_mode=MEMORY` are not (see [Not now](#not-now)).

At 4 KiB pages, Quick Checklist is 204 KiB:

| Payload | Bytes |
| --- | ---: |
| Manifest, descriptor and theme text | 746 |
| Stored app assets (181,451 decoded) | 52,949 |
| Preview PNG, 960 × 1,240 | 88,579 |
| Icon PNG, 512 × 512 | 18,474 |
| Initial Loro checkpoint | 1,137 |

Artwork is 107 of its 162 KB of payload. The rest of the file is page slack. Rebuilding
the dev templates in memory from `iterdump()` (Python's SQLite 3.51.0, `auto_vacuum=FULL`,
only `page_size` varied) gave:

| Template | 4 KiB pages | 2 KiB pages | 1 KiB pages |
| --- | ---: | ---: | ---: |
| `fixture-checklist` | 48 KiB | 24 KiB | 14 KiB |
| `conformance` | 64 KiB | 40 KiB | 32 KiB |
| `quick-checklist` | 204 KiB | 178 KiB | 170 KiB |

The 48 KiB floor is 12 B-tree roots: page 1, the pointer map, seven tables and the
automatic indexes of the three text primary keys. These are allocation sizes, not
filesystem usage, throughput or latency.

What copies carry today:

- `Store::close_job` trims history only when a session edited a document larger than
  4 MiB ([store](../crates/hitslop-core/src/store.rs)). Below that, the checkpoint is a
  full Loro snapshot, and everything deleted since creation is recoverable.
- Share a Copy and Duplicate copy every page with SQLite backup (`Store::copy_to`,
  `file::copy`). The copy holds that history, every attachment ever imported (including
  removed ones) and the artwork of the last close, which can show deleted content.
- `secure_delete` is never set. Apple's SQLite defaults to FAST (`2`, checked on 3.51.0);
  the bundled SQLite of the Linux engines and sqlite-wasm-rs default to `0`.

## Immediate sequence

### 1. Clean copies

One Rust owner operation writes every copy that leaves the owner: Publish (share links),
Send File and Duplicate. Captures, which are temporary and never leave the machine, stay
plain backups. The operation never changes the original:

1. Back up into the staged file.
2. Replace its checkpoint with a shallow snapshot at the latest version, exported from
   the owner's document after the flush. Export only: `trimmed()` also calls
   `retain_from`, which would cut the live session's undo history.
3. Delete its `updates` and every attachment the snapshot doesn't reference
   ([attachment scan](#attachment-scan)).
4. Render artwork from the copy's state. A copy whose render fails carries no artwork,
   never the original's.
5. Run every open check, with the quick check, then publish without replacing anything.

Set `PRAGMA secure_delete=FAST` in `file::configure_writer`, so leftover bytes are
format policy rather than a property of the linked SQLite. FAST zeroes deleted cell
content; FULL auto-vacuum truncates freed pages or overwrites them with relocated ones in
the same commit.

The original keeps its history while it is under 4 MiB. Trimming at every close would
make an agent's second text edit from one `get` fail as `stale_base`, because each closed
CLI command runs its own owner session. The guarantee to document: a copy carries the
current state, the attachments it references and artwork of that state; the original
keeps history under 4 MiB and keeps blobs until [reclamation](#4-attachment-reclamation).

No marker changes: shallow checkpoints are already written today.

#### Attachment scan

A stored attachment is referenced when its ID (64 lowercase hex characters) appears in
any string or text of the current state, including inside longer text such as markdown.
The scan reads materialized values, not bytes; the IDs to look for come from the
`attachments` table (at most 256). A false match only keeps a blob. An app that stores a
transformed ID loses that attachment, so author docs say to store the reference's `id`
as given. Once released, a later build may widen what counts as referenced, never narrow
it. Copies and reclamation use this one function.

A typed `s.attachment()` descriptor is not a prerequisite. It would miss IDs in text,
and existing apps would still need the scan. It stays a possible authoring feature.

### 2. Strict tables

- Add `STRICT` to all seven tables. Keep their responsibilities and rowid layout. STRICT
  makes primary-key columns `NOT NULL`, so no explicit constraint is needed. No
  `CHECK(size >= 0)`: `stored_assets` already refuses negative sizes on every open, and
  constraints are never evaluated when reading an untrusted file.
- STRICT adds type validation to `quick_check` on integrity opens
  ([STRICT tables](https://sqlite.org/stricttables.html)). Display-only opens skip the
  quick check and Rust writes typed values, so no Rust check is removed: bounded reads,
  `typeof(size)`, path checks, decompression limits, PNG headers and descriptor checks
  stay.
- Supported SQLite: the macOS 15.2 deployment target ships 3.43 or later; the Linux
  engines and the browser build bundle their own.
- The exact-layout check refuses files written before the change. Repack templates and
  regenerate the dev corpus with the compat writers; no migration or adapter.

### 3. Artwork

Capture encodes every image as 8-bit RGBA with default settings
(`SlopPreviewImage.png`), including fully opaque rectangular windows. Optimize
losslessly in one Rust step that both `pack` and `Store::set_artwork` use: drop an
opaque alpha channel, use a palette when there are at most 256 colors, and choose better
filters and compression. Keep a result only when it is smaller and its decoded pixels
are identical. Pin the optimizer's latest release.

Adopt it in `pack` first: it costs nothing at runtime and covers the shipped templates.
Time it on every bundled template's preview and icon (release build, p50 and p95)
before putting it on the close path, and record the result under `docs/evidence/`.

Preview and icon roles, dimensions and PNG/PDF exports stay as they are. A lower capture
resolution is a separate experiment across Finder, Quick Look, the catalog and
high-density displays. No new artwork codec.

### 4. Attachment reclamation

Today an import stores its blob before its reference edit. A refused or abandoned import,
and a removed reference, leave the blob behind, so repeated imports can reach the
100 MiB or 256-file limit while the document uses little media.

Reachability is the saved current state only. Undo covers the open session, and nothing
reads retained history after close, so neither is a root. A future history UI keeps its
own blobs. Dangling references are already possible (an agent can write any hash) and
read as `path_not_found`. The invariant is: never delete a blob the saved current state
references.

The blocker is the CLI's two-step import. `slop attachments import` stores a blob
(`attachments.put`) and prints its reference, and the agent writes it with a later
`apply`. On a closed document each command is its own owner session, so cleanup at
close would delete the blob between them; so would a person closing the window between
them. Fix the API first:

1. Make attachment import a `batch` intent, as the page's
   `ctx.attachments.import(file, reference)` already pairs blob and reference. The CLI
   reads the file and computes its SHA-256, so one request carries the blob and the
   operations that use its ID; no placeholders. The owner stores the blob, then applies
   the operations; a refusal leaves only an orphan. Remove the standalone
   `attachments.put` method.
2. Clean up at close: after the final save succeeds and the page barrier drained, scan
   the saved state and delete unreferenced blobs in their own transaction. A crash
   between the save and the deletion leaves only orphans. Skip the step when there are
   no attachments or an earlier step failed. Closed CLI sessions run it too.

Update the runtime reference's "Unreferenced blobs remain" sentence when this ships.

## Not now

- **Page size.** Keep 4 KiB, the SQLite and Apple default and the APFS block size.
  Smaller pages save at most 34 KiB per file, which APFS blocks, compressed downloads and
  mail attachments make invisible. Their cost is in large media: `AssetReader` opens a
  blob handle per range request, and each one walks the asset's overflow chain from the
  start, 4× longer at 1 KiB. Revisit only if small files start to matter, for example in
  a hosted catalog. A page size is a writer choice, never a marker.
- **Per-document app copies.** Every document carries its whole app, up to 50 MiB of
  assets, copied page by page on create (`file::create_document`). For media-heavy apps
  that is the largest size multiplier. Cloning the template with APFS `clonefile`, then
  checking it and adding the document row, would share the app's pages on disk and make
  create constant time. Finder still shows the full size, copies still send every byte,
  and auto-vacuum relocation unshares some pages over time. An idea to measure.
- **`WITHOUT ROWID` on blob tables.** `AssetReader` uses incremental blob I/O through
  `rowid`, which `WITHOUT ROWID` tables don't support
  ([SQLite](https://sqlite.org/withoutrowid.html)).
- **uApp's `journal_mode=MEMORY`.** uApp accepts torn writes and keeps external recovery
  snapshots; SQLite warns that a crash during a MEMORY-journal transaction can corrupt
  the database ([journal modes](https://sqlite.org/pragma.html#pragma_journal_mode)).
  Keep DELETE, synchronous EXTRA and fullfsync.
- **Incremental vacuum.** FULL auto-vacuum frees pages at each commit but doesn't repack
  partly filled ones. Incremental vacuum is a possible later latency experiment.
- **A second compression layer around Loro**, without a measured benefit on realistic
  saved documents.
- **Embedded source** belongs to [remix](../docs/ideas.md#remix).

## Compatibility

Keep the Rust owner, external writer lock, one edit path, async writes, save before
close and failed-save ownership retention, and the independent package, runtime, storage
and layout requirements. Pre-launch, the schema change replaces development artifacts
and keeps every requirement at 1. After release, a SQL layout change needs a
storage-version migration under the lock, and the attachment scan only widens. Physical
tuning (`secure_delete`, artwork encoding) raises nothing.

## Verification

Tests live at the owning boundary, never on SQL spelling or version constants:

- **Rust file and storage:** strict files pack, create, open, edit and reopen; files with
  wrong types fail the quick check; older-layout and newer files stay untouched. A
  clean copy holds no `updates`, its checkpoint has no history before the copy, its
  attachments are exactly the referenced ones, and it passes every open check while the
  original and its undo history are unchanged. A string deleted before the copy appears
  nowhere in the copy's bytes.
- **Owner:** an attach batch stores the blob and its reference together, and a refused
  one leaves only an orphan; cleanup at close keeps every referenced blob (in a field,
  inside text, shared by two references) and removes orphans; a crash between the final
  save and cleanup reopens with the saved state and the orphans; failed saves skip it.
- **Native:** clean copies render their own artwork, and a failed render yields none.
  Finder, Quick Look and PNG/PDF export keep working. Artwork optimization checks
  decoded pixels, not only PNG headers.

Iterate with the affected `bun run verify` tier and run `bun run verify` before calling
a step done; `bun run verify --native` at the end when Swift, the FFI or the helper
changed. Generate TypeBox-owned files with `bun run schema:generate` (the batch intent).
A bug regression test must fail before its fix.

The pass is complete when the four steps ship with their tests, the artwork decision is
recorded under `docs/evidence/`, the dev artifacts are regenerated, and the architecture,
runtime reference and engineering contract describe the result. Their stale wording: the
engineering contract lists initial values in the `app` row, but a template's
checkpoint holds them.
