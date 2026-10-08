# Engineering contract

How the system works is in [architecture](architecture.md).

## Compatibility

Before launch we start fresh: no legacy handling, migration or backwards-compatible
reader, and pre-launch documents are unsupported. From the first public release, every
document a released build wrote must open, render, edit, save and reopen in every later
build, on supported macOS versions. Downgrades are not supported: an older build refuses
newer formats with `requires_update` and writes nothing.

Old documents depend on a few public boundaries; everything behind them may change.

| Boundary | Marker | A later build |
|---|---|---|
| The app in a `.slop` file: catalog fields and `definition_json` (window, descriptor, theme, commands), plus resource semantics | `app.package_format`, stamped by the engine from the build | Dispatches to the reader for that format |
| App behavior: `ctx`, handles, errors, the command prelude and host DOM/CSS conventions | `app.runtime_abi`, stamped from the project's resolved SDK | Dispatches to the app-facing context adapter for that ABI |
| The file's tables, and the layout every open checks | SQLite `user_version` (storage version) | Migrates forward under the writer lock, in one transaction |
| How descriptor kinds map to Loro containers ([layout 1](reference/document-types.md#storage-layout)) | `meta.layout` in each document, written when it is created | Reads it, or migrates it losslessly (same value, row IDs, text, theme and attachments) in one commit with its marker; snapshots migrate in memory only |
| CLI ↔ document engine ↔ live owner | The command protocol: `--client-protocol N` on `slop-engine` (and on the helper it runs), and `protocol` in every socket request; `--protocol` reports the protocol served | Serves exactly its own protocol, with no adapters for older programs, and refuses any other before touching the document (exit status 2, or `requires_update`), naming the older side to update. The refusal path never changes: `--client-protocol N` first, exit status 2 and one stderr line; the live discovery record's location and its `socket` and `documentPath` fields (other fields are ignored); newline-delimited framing; `protocol` read before any other check; and the reply `{ok: false, code: "rejected", reason: "requires_update", error}`. Protocol 1 is today's commands, arguments, request and reply JSON, outputs and exit statuses |

### How documents evolve

A slop carries its own copy of the SDK: its `ui.js` and `commands.js` were compiled with
the SDK its project pinned, Svelte included. Changing the SDK therefore never affects a
released document, and authors move to a new SDK when they rebuild. What old documents
depend on is the host side they call into, the runtime ABI that `sdk/abi.ts` lists. A
host change an old app cannot run raises `runtimeABI` and keeps the old behavior for apps
built at the lower level.

Each marker in the table above has its own mechanism. Only storage is rewritten:
SQL migrations move a file forward. A package format is read in place and translated into
the host model in memory, a runtime ABI keeps its adapter, and a Loro layout is read as it
is or migrated losslessly. The app inside a document is an author's compiled code, so it
is never migrated; moving a document to a newer build of its app is a separate, explicit
operation ([additive app upgrades](ideas.md#additive-app-upgrades)).

Migration is lazy. It runs only on a write that needs the newer structure, under the
writer lock and inside that write's transaction, never at launch or on a display read:
previewing, listing or exporting a file leaves it untouched. A migrated file opens only
in builds that know its new marker, so migrating only when needed keeps a file that two
Macs share openable on both for as long as possible.

Markers are requirements, not release numbers: refactors never raise them, and app,
CLI and SDK versions never stand for them. The Mac app and the npm package share one
release version. A format change that an older build cannot read correctly raises a
marker; a change without one is allowed only when older readers already handle it. An additive `ctx` API still raises
`runtimeABI` once the SDK depends on it; an app may treat an API as optional only where
it has a real fallback. A syntax reader never caps `runtimeABI`: the supported runtime is
checked before the reader is chosen. The build stamps `runtimeABI` from the project's
SDK and refuses one this CLI cannot validate or preview. Raising one
adds a reader and leaves released behavior in place. The current format reader is
`app/package_format_1.rs`; the page context is `shell/abi/1.ts`, the command prelude
is built from `shell/abi/runner-1.ts`, and `sdk/abi.ts` lists every part of the app ABI,
including the globals, markup and CSS variables that are not `ctx`. Compile-time guards require a new arm when their marker rises.
The page receives the stored descriptor unchanged, while native UI receives the translated
host model. Storage-1 SQL freezes at the first release; later physical changes use a
writer-locked transactional migration. Every open reads
the markers (application ID, storage version, then `package_format` and `runtime_abi`)
before it compares the exact tables, so a newer file is refused with `requires_update`
even when its tables differ. Saved state belongs to the descriptor in its file's `app`
row, which is written once; no copy of it is stored with the state. App limits and the
checks that run on open are versioned by `packageFormat`. Persistence limits (storage
size and updates, attachments) govern every save as well as every open, so they are
versioned by the storage version: never lowered for a released one, and raised only
together with it. Tightening an authoring rule never rejects a saved document or stops
one from being edited and saved, and an accepted edit stays readable under the markers
its save writes. Each format module therefore holds two rule sets: `checked`, what opening
requires (the shape the host can interpret safely, under the format's own frozen limits;
it may only loosen), and `authoring`, what `pack` and `init` require of a new app (it may
tighten). Opening never re-runs signature sniffing or authoring grammars on a sealed app,
and damaged or oversized artwork reads as absent rather than refusing the document. New
files are created by replaying `storage-1.sql` and then each storage migration in order,
so a migrated file and a new one share one exact layout. A frozen corpus entry records
the command prelude of its runtime ABI; that prelude's bytes are then final. A security fix that must reject old documents needs an assessment and a
recovery path for their data. Installing hitSlop never replaces the app inside an
existing document; upgrading a document's app is an explicit operation. Only a write
under the writer lock migrates a file, validating the result and committing the data and
its markers together; reads that only display it (Quick Look, the catalog, `get`,
export) never migrate it. Public boundaries grow additively: `ctx` and handle
methods (new object-handle members start with `$`; reserved field names never grow),
error and issue codes (apps treat unfamiliar ones as outcomes), `--slop-*`,
`data-slop-root` and the embed relay. The engine, rendering helper and live owner ship in one
Mac app bundle and keep an exact core build check. Loro is pinned exactly and upgraded only
with the corpus passing.

The [compatibility corpus](testing.md#compatibility-corpus) is the evidence: every
release's templates and saved documents, replayed by every build. A frozen entry is never
edited, regenerated or deleted. The corpus is regression evidence, not proof that every
possible document opens. Apps that embed external services depend on those services.

## Ownership

How an edit, a save and a close move is in [architecture](architecture.md). The rules:

- `hitslop-core` (Rust on Loro, `crates/`) owns document semantics and durable storage
  (the writer lock, the `.slop` file, the save policy), and is the only code that opens a
  `.slop` file: the app, the helper, Quick Look and the CLI's `slop-engine` all read and
  write through it. The Rust owner schedules saves and serializes edits; Swift
  `DocumentOwner` delivers typed requests and events. Loro bytes never reach Swift. The page shell (`packages/hitslop/src/shell`) holds no CRDT.
- A `.slop` file is one SQLite database. A template holds its app: the `app` row
  (catalog columns, a definition document and two requirements), `assets` (including
  `ui.js`, optional CSS, media and the private command program) and optional preview and icon `artwork`, and its initial values as its
  `checkpoint`. A document also holds its identity, theme overrides, saved state and
  attachments. No engine, source,
  dependencies or caches.

## Rules

- One edit path. The CLI forwards to the live owner's socket or takes the writer lock
  and runs the owner in-process, without WebKit or authored code. Never bypass a busy
  lock or remove a lock file from the registry (`~/.hitslop/live`).
- Storage: one write in flight. Failed or unacknowledged saves keep ownership, all edits
  and the last confirmed saved version, and show a native retry. The store re-reads its
  sizes after a failure; it advances the saved version only on confirmed success.
  Loro deduplicates overlapping updates, but mutation intents are never replayed.
- Flush unsent text and pending writes before close or export. Captures render saved
  state in a fresh read-only page; the optional explicitly imported export view supplies the layout, otherwise
  a fresh App uses its default local UI state. Successful close destroys WebViews.
- Author schemas with `defineDocument`/`s`. The stored descriptor is not JSON Schema.
  Initial values are read only when a document is created. Descriptor kinds exist only once Rust,
  the SDK and a fixture implement them (today: text, boolean, string, number, integer, enum,
  optional (of a scalar, text or an object), object, list(object), list(scalar),
  record(scalar or object) and integer counter; see
  [document types](reference/document-types.md)).
- Writes are asynchronous and resolve after the snapshot updates; `change(tx => …)`
  collects synchronously. Reads come from immutable snapshots. Preserve `$id`
  identity. Every accepted operation keeps the document valid; invalid stored state is
  refused without modifying the file.
- Never add a JSON copy of the document, persistent JSON mirrors, JSON reconciliation, a
  JavaScriptCore engine or a second document engine. The WASM core is test-only. Browser development uses a native owner; the native engine validates authoring input.
- Rust owns wire types and shared limits (`crates/hitslop-core/src/wire`); ts-rs exports
  TypeScript and UniFFI carries native types to Swift. App acceptance also belongs to Rust; JSON Schema is only a descriptor projection for tool clients. Run `bun run schema:generate`; never edit generated files.
- The core checks every file it opens (its layout, rows, markers and resource bounds) and
  every build it packs, not authored UI behavior. Rust decodes and checks page/socket requests once; Swift checks the page sender and receives typed native actions. The helper receives Rust-decoded native
  request enums and returns typed outcomes for Rust to encode.
- Attachments are host-owned immutable content-addressed blobs in the file's
  `attachments` table, referenced by ordinary document fields and served by URL. Stored media types are sniffed; the first read verifies the full hash.
- Page and CLI commands share the owner's descriptor validation and restricted child. The UI bundle contains stubs only. Every evaluation gets a fresh runtime; no command globals persist across calls.
- The app row seals assets at pack time. No app or asset updates/deletes, and no asset insert after the seal, including conflict-replacement forms. Catalog summaries never parse `definition_json` or read app/attachment blobs.
- Preserve the macOS client: app lifecycle and quit, catalog and Recents, slop windows and
  toolbar, PNG/PDF export, Quick Look previews and Finder icons, Firebase
  Analytics/Crashlytics and Sparkle.
- Active examples are the directories with a `slop.ts` under `examples/slops`;
  `bundled.json` selects shipped templates. Dedicated fixtures own platform semantics. Use plain CSS and
  `slop.ts` theme colors; read `examples/slops/PRODUCT.md` and `docs/guides/authoring.md` for
  visual changes.
- Development-only exception: an opt-in native `dev-sync` loopback harness may qualify
  one authority and two temporary replica owners. It keeps Loro bytes in Rust, disables
  shared undo and attachment imports, and creates no released format or production
  endpoint. Shared-session fencing ends with its live owner; durable restart/retry
  recovery remains deferred.
- Deferred: production collaboration, a document history UI, schema evolution (changing a
  descriptor makes a new document type), synced folders, hosted catalog/publishing,
  accounts and sharing.

Testing policy lives in [testing](testing.md). Agent entrypoint: [AGENTS.md](../AGENTS.md).
