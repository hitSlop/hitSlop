# SQLite format review and optimization plan

Status: proposed, 2026-10-05. Comparison and initial size measurements complete;
implementation has not started. This plan assumes breaking pre-launch changes are
allowed. It does not authorize changing released documents or frozen corpus entries.

Keep the single SQLite file, Rust ownership and Loro semantics. The immediate work is
to strengthen the SQL schema, measure smaller database pages and optimize artwork.
First-class attachment references are the most useful larger format change to design
next. Carrying source for remixing is a separate product capability, not a size saving.

## Comparison and evidence

The comparison uses the current working tree, the local
[uApp checkout](../_docs/uApp-main/README.md), and
`~/Desktop/Recipes.capsule`. The latter is one artifact, not a review of
Capsule's host implementation. uApp source under `_docs` is reference material, not an
active hitSlop contract, and may be absent from another checkout.

| Concern | hitSlop | Local uApp implementation | Recipes capsule |
| --- | --- | --- | --- |
| Container | SQLite; seven application tables | SQLite; six housekeeping tables plus arbitrary app tables | SQLite; eight application tables plus SQLite's sequence table |
| App representation | Immutable `app` row and path-addressed assets | Editable files in `sqlar`, separated into `app/` and `data/` paths | `app_ui` holds versioned HTML and optional source bundle/framework; `app_assets` holds files |
| Saved data | Loro checkpoint and ordered incremental updates; theme overrides share Loro | App-defined SQL tables, queried and edited directly | `doc_records(collection, _id, data_json)` and `doc_storage(key, value)` |
| App history | No embedded source or app revisions | Compressed file revisions, chat logs and conversation metadata | UI table supports multiple versions with a unique active version; sample has one |
| Attachments | Immutable SHA-256-addressed blobs; references are ordinary document fields | Files under `data/`, using the same archive table | Separate `doc_assets` metadata and `doc_asset_blobs` bytes; sample has neither |
| Compression | Brotli for text/WASM assets when smaller; media remains range-readable | SQLAR zlib compression when smaller, also retained in file history | Sample HTML is stored as uncompressed text |
| SQL typing | Ordinary tables with declared types and selected checks | Ordinary tables with app-controlled SQL schemas | All application tables are `STRICT`; small metadata tables also use `WITHOUT ROWID` |
| Format markers | SQLite application ID/storage version, package format, runtime ABI and Loro layout | Metadata includes `format_version` | `app_meta.schema_version=9`; SQLite application ID and user version are both zero |
| Durability | On-disk DELETE rollback journal, synchronous EXTRA and fullfsync | Live engine uses a MEMORY journal and FULL synchronization, with external recovery snapshots | Host write/recovery policy cannot be established from the sample |

The models serve different needs. uApp makes ordinary SQL tooling and indexed queries
available to authors. Capsule's collection API exposes JSON records. hitSlop supplies
descriptor validation, stable row identities, text merging, ordered publications and
undo through one engine. Replacing Loro with either model would change those semantics;
it is not a container optimization. Do not introduce persistent SQL/JSON mirrors of
Loro to imitate their inspectability. Our CLI remains the semantic inspection path.

The implementation sources are
[hitSlop file layout](../crates/hitslop-core/src/file.rs),
[hitSlop storage](../crates/hitslop-core/src/store.rs),
[uApp storage](../_docs/uApp-main/src/store.rs) and
[uApp engine](../_docs/uApp-main/src/engine.rs).

### Measured packaging costs

Recipes is 131,072 bytes (128 KiB), with 4,096-byte pages and no freelist pages. It
contains one active HTML version, no source bundle, no saved records, no stored
key/value data, and no app or document assets. Its HTML is 65,471 UTF-8 bytes;
in-memory zlib level 6 compression produces 11,701 bytes. That is a payload measurement,
not the size of a rebuilt capsule. Its file size is not comparable to a populated slop.

The inspected Recipes file has SHA-256:
`4f7314c225123b2504ae50c4a2de2cb5ac68c2ea0bbecace3b570ed680e96482`.

Three development templates were reconstructed in memory with identical schema and
row contents, `auto_vacuum=FULL`, and different SQLite page sizes:

| Template | 4 KiB pages | 2 KiB pages | 1 KiB pages |
| --- | ---: | ---: | ---: |
| `fixture-checklist` | 48 KiB | 24 KiB | 14 KiB |
| `conformance` | 64 KiB | 40 KiB | 32 KiB |
| `quick-checklist` | 204 KiB | 178 KiB | 170 KiB |

Method: Python's SQLite 3.51.0 opened each file under `tests/compat/dev/templates/`
read-only, obtained `iterdump()`, then executed that dump in a fresh in-memory database
after setting `page_size` and `auto_vacuum`. Sizes are `page_count * page_size`.
No original file was rewritten. These measurements establish allocation differences,
not filesystem usage, write throughput, durability or native open latency. The dev
corpus is replaceable, so future measurements must record input hashes.

At the current page size, Quick Checklist contains:

| Payload | Bytes |
| --- | ---: |
| Manifest, descriptor and theme text | 746 |
| Stored app assets | 52,949 |
| Uncompressed app assets | 181,451 |
| Preview PNG, 960 × 1,240 | 88,579 |
| Icon PNG, 512 × 512 | 18,474 |
| Initial Loro checkpoint | 1,137 |

Artwork is roughly half the physical file. The tiny checklist fixture contains only
about 1.8 KiB of app metadata, assets and state; most of its 48 KiB file is minimum
table/index allocation. Our existing asset compression already captures the obvious
compression opportunity visible in Recipes.

Existing [storage growth evidence](../docs/evidence/document-growth-2026-10-04.md)
also shows that checkpoint retention already bounds history in the tested heavy-use
workloads. Those measurements do not cover attachment garbage collection.

## Immediate implementation sequence

### 1. Strengthen the SQL schema

- Make all seven application tables `STRICT`. Keep their responsibilities and rowid
  layout. Confirm SQLite 3.37 or newer on supported native and CLI targets.
- Add explicit `NOT NULL` to text primary keys and `CHECK(size >= 0)` to asset sizes.
  Keep current singleton and encoding constraints. Leave configurable byte budgets
  and semantic validation in Rust rather than duplicating them throughout SQL.
- Retain the exact-layout check and marker checks before interpreting stored content.
  `STRICT` supplements Rust validation; it does not replace bounded reads, path checks,
  decompression limits, PNG validation or descriptor checks.
- Repack pre-launch templates and recapture only the development corpus. No migration
  or old-format adapter is needed under the pre-launch contract.

This changes the accepted SQL schema but introduces no public SDK or command API.
SQLite documents strict typing and type validation by `quick_check` in
[STRICT tables](https://sqlite.org/stricttables.html).

Do not convert our BLOB tables to `WITHOUT ROWID`. `AssetReader` uses incremental BLOB
I/O through `rowid`, and SQLite does not support that API on WITHOUT ROWID tables.
Large rows can also perform worse there. Capsule's small metadata tables are a different
case. See [SQLite's guidance](https://sqlite.org/withoutrowid.html).

### 2. Select a page size from production-path measurements

Run the page-size comparison after the strict-schema change, holding every other
setting constant. Compare 1,024, 2,048 and 4,096 bytes; use 4,096 as the control.
Set the candidate before creating tables in the template writer. Do not impose a
page-size requirement on readers or rewrite existing documents merely to resize pages.

Use the real Rust owner, storage and asset reader for these workloads:

- Empty/tiny templates, the conformance fixture, and Quick Checklist with artwork.
- Saved 1,000-row and 5,000-row documents, including long text and normal edits.
- Append saves, checkpoints, close-time trimming and repeated close/reopen cycles.
- 10 MiB attachment imports/reads and documents near the existing state/asset/attachment
  limits, including range reads beside saves and saved-state backup/capture.

Record physical bytes, logical payload bytes, page/freelist counts, peak memory,
open-to-ready time, save/read/copy median and p95, and all failures. Keep DELETE,
EXTRA, fullfsync and FULL auto-vacuum unchanged. Use release builds, one warmup and at
least 30 measured samples for each timed comparison, alternating candidates to reduce
run-order effects. Record platform, SQLite version, build identity and input hashes.

Proposed selection gate: a candidate must pass all correctness/crash cases and must
not worsen a workload's p95 by more than the larger of 10% or 1 ms against the paired
4 KiB control. Repeat a marginal result before deciding. Select the smallest candidate
that passes; retain 4 KiB if neither passes. These are proposed acceptance thresholds,
not already demonstrated performance. Publish the report under `docs/evidence/`.

A page-size default is a writer choice, not a new semantic format. Do not raise package
or runtime requirements for it. Avoid merging unrelated tables to reduce allocation.

### 3. Reduce artwork cost while preserving native presentation

First measure lossless PNG optimization at the current dimensions. Preserve PNG
encoding, transparency and the current preview/icon roles. Keep an optimized result
only when it is smaller and decoded pixels are identical; measure the added capture
cost before placing optimization on the close path.

Changing capture dimensions is a separate experiment: compare Finder, Quick Look,
catalog and high-density displays before accepting a lower resolution. Do not silently
trade preview quality for the largest percentage reduction. PNG/PDF exports keep their
existing behavior. Adding a new artwork codec or compressing already-compressed PNGs
inside another wrapper is outside this pass.

No public API or SQL schema change is needed for lossless PNG optimization. Keep Brotli
asset compression and uncompressed, range-readable media. Do not add a second compression
layer around Loro without a separate measured benefit on realistic saved documents.

## Larger changes to design separately

### First-class attachment references and reclamation

Current imports store the blob durably before submitting its reference as a document
edit. If the collector throws or the edit is refused, the blob remains. Removing a
reference also leaves the blob behind. Deduplication prevents duplicate bytes, but it
does not reclaim unreachable bytes. Repeated imports can reach the 100 MiB or 256-blob
limit even when the current document uses little media.

Propose an `s.attachment()` descriptor whose snapshot retains the existing reference
information: content ID, byte length, display name and MIME type. Rust must recognize
the reference semantically rather than infer it from strings that resemble hashes.
Keep content-addressed immutable bytes in SQLite; names and MIME types belong to each
reference because identical bytes can be used with different metadata.

This requires a coordinated TypeBox descriptor, Rust semantics, SDK types, shell
behavior, native attachment handling and a conformance fixture. Typed references must
survive create, replacement, copy, undo/redo, save and reopen, and native admission must
ensure a referenced blob exists before accepting an edit. The WASM development path
must offer equivalent observable behavior with its in-memory attachment store.

Garbage collection needs an explicit reachability contract covering current state,
retained history, undo/redo, imports awaiting their reference edit, and failed or
unacknowledged saves. A current-value scan alone is insufficient. A safe first design
target is collection only at a quiescent close after the saved checkpoint has discarded
older history, no import is pending and no undo can restore a removed reference.
If these conditions cannot be established, retain the blob. Preserve crash-safe ordering
between the saved state and blob deletion under the existing owner and persistence path.

The follow-up design must settle historical reachability and atomic reclamation before
shipping deletion. Do not introduce an independent mutable reference-count database as
a second authority. Do not change history retention merely to enable collection in the
immediate optimization pass. Splitting metadata and bytes into Capsule-like tables does
not solve these lifetime questions by itself.

### Optional source for remixing

uApp can edit embedded app files, and the Capsule schema has optional source-bundle and
framework fields. hitSlop's immutable compiled app cannot reconstruct its original
authoring project. A compressed, optional source payload would support the existing
[remix proposal](../docs/ideas.md#remix), although it increases file size.

Keep this outside the optimization implementation. The later design should specify
which source files and dependency metadata are included, size limits, licensing and
provenance, exclusion of secrets/private notes, and extraction without executing code.
Source must remain outside the page-serving namespace; rebuilding produces a new app.
Do not embed dependencies, build caches, credentials or unlimited chat/revision history.
Updating an existing document's descriptor remains the separate additive-upgrade decision.

## Durability and compatibility boundaries

Keep the Rust owner, external writer lock, one edit path, async writes, save-before-close
and failed-save ownership retention. Keep Loro as the only document engine and preserve
the independent package, runtime, storage and document-layout requirements.

In the inspected uApp engine, live writes use `journal_mode=MEMORY`, with ten retained
external snapshots refreshed on open and periodically while dirty (ten-minute interval).
The source explicitly accepts torn writes. SQLite likewise warns that a crash during a
MEMORY-journal transaction can corrupt the database. Do not adopt this to eliminate the
temporary sibling journal. [SQLite journaling documentation](https://sqlite.org/pragma.html#pragma_journal_mode).

Keep FULL auto-vacuum for the initial comparison. It moves free pages and truncates on
commit, but does not repack partially filled pages and can increase fragmentation.
Incremental vacuum is a possible later latency experiment, with disk-growth and
maintenance behavior measured explicitly. It is not part of the page-size change.
[SQLite auto-vacuum documentation](https://sqlite.org/pragma.html#pragma_auto_vacuum).

The pre-launch pass can replace unsupported development artifacts and keep baseline
requirements at 1, following the current contract. Before doing so, confirm no public
release/frozen corpus has established the old layout as supported. After release,
changes to SQL layout need a storage-version migration under the lock; source/package
changes need the appropriate package requirement; app-facing descriptor behavior needs
the appropriate runtime/layout treatment. Refactors and physical tuning do not raise
semantic requirements. Never rewrite a frozen corpus or relax refusal of newer markers.

## Verification and completion criteria

Extend tests at their owning boundary rather than testing SQL spelling or numeric
version constants:

- Rust file/storage: valid strict files pack, create, open, edit and reopen; invalid
  types, null keys and negative sizes are rejected; invalid/newer files remain
  untouched; template/document distinction, limits and bounded decoding remain intact.
- Storage/owner: normal append/checkpoint/trim behavior, attachment deduplication,
  failure/retry, disk-full handling, copies and concurrent reads remain correct across
  candidate page sizes. Existing crash tests still yield the old or committed state,
  never a torn document or a lost acknowledged edit.
- Native integration: artwork, Finder/Quick Look and saved-state PNG/PDF capture keep
  working. Lossless artwork experiments check decoded pixels, not only PNG headers.
- Future attachment work: refused imports, shared references, replacement, deletion,
  undo/redo, historical references, pending imports and crashes around reclamation
  must never produce a dangling reference. Test recovery of space only when collection
  is proven safe.

Iterate with the affected `bun run verify` tier, then run `bun run verify` before
calling each implementation step complete. Run `bun run verify --native` at the end
when Swift, the FFI or helper changes, and exercise the native rendering/capture paths
for any capture optimization. Generate TypeBox-owned files with
`bun run schema:generate`; never edit generated output by hand. Bug regressions must
fail before their fix for the intended reason.

The immediate pass is complete when strict tables are validated, the page-size report
records an explicit adopt/retain decision, artwork has an evidenced adopt/retain
decision, affected development artifacts are regenerated, and the architecture and
engineering contract describe the actual resulting layout. Check their initial-state
wording: the current implementation stores template initial values in `checkpoint`,
not an `initial` column in `app`. Leave attachment reclamation and source embedding as
separate proposals until their contracts are settled.
