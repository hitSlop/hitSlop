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
| The app in a `.slop` file: the `app` row (manifest, descriptor encoding, theme), assets and artwork | `app.package_format`, stamped by the engine from the build | Dispatches to the reader for that format |
| App behavior: `ctx`, handles, errors and host DOM/CSS conventions | `app.runtime_abi`, stamped from the project's resolved SDK | Dispatches to the app-facing context adapter for that ABI |
| The file's tables, and the layout every open checks | SQLite `user_version` (storage version) | Migrates forward under the writer lock, in one transaction |
| How descriptor kinds map to Loro containers ([layout 1](reference/document-types.md#storage-layout)) | `meta.layout` in each document, written when it is created | Reads it, or migrates it losslessly (same value, row IDs, text, theme and attachments) in one commit with its marker; snapshots migrate in memory only |
| CLI ↔ document engine ↔ live owner | The command protocol: `--client-protocol N` on `slop-engine` (and on the helper it runs), and `protocol` in every socket request; `--protocol` reports the protocol served | Serves exactly its own protocol, with no adapters for older programs, and refuses any other before touching the document (exit status 2, or `requires_update`), naming the older side to update. The refusal path never changes: `--client-protocol N` first, exit status 2 and one stderr line; the live discovery record's location and its `socket` and `documentPath` fields (other fields are ignored); newline-delimited framing; `protocol` read before any other check; and the reply `{ok: false, code: "rejected", reason: "requires_update", error}`. Protocol 1 is today's commands, arguments, request and reply JSON, outputs and exit statuses |

Markers are requirements, not release numbers: refactors never raise them, and app,
CLI and SDK versions never stand for them. The Mac app and the npm packages share one
release version. A format change that an older build cannot read correctly raises a
marker; a change without one is allowed only when older readers already handle it. An additive `ctx` API still raises
`runtimeABI` once the SDK depends on it; an app may treat an API as optional only where
it has a real fallback. A syntax reader never caps `runtimeABI`: the supported runtime is
checked before the reader is chosen. The build stamps `runtimeABI` from the project's
SDK and refuses one this CLI cannot validate or preview. Raising one
adds a reader and leaves every released one as it is: before the change, move the current
reader (`manifest::validate` with the manifest schema it validates against, the shell's
`createContext`, or the protocol's command tree) under the released marker's name, then
add the new one beside it; keep the old layout and storage arms. Until then, raising
`packageFormat` or `runtimeABI` fails to build at its reader (an assertion in
`manifest.rs` and `boot.ts`). Every open reads
the markers (application ID, storage version, then `package_format` and `runtime_abi`)
before it compares the exact tables, so a newer file is refused with `requires_update`
even when its tables differ. Saved state belongs to the descriptor in its file's `app`
row, which is written once; no copy of it is stored with the state. App limits and the
checks that run on open are versioned by `packageFormat`. Persistence limits (storage
size and updates, attachments) govern every save as well as every open, so they are
versioned by the storage version: never lowered for a released one, and raised only
together with it. Tightening an authoring rule never rejects a saved document or stops
one from being edited and saved, and an accepted edit stays readable under the markers
its save writes. A security fix that must reject old documents needs an assessment and a
recovery path for their data. Installing hitSlop never replaces the app inside an
existing document; upgrading a document's app is an explicit operation. Only a write
under the writer lock migrates a file, validating the result and committing the data and
its markers together; reads that only display it (Quick Look, the catalog, `get`,
export) never migrate it. Public boundaries grow additively: `ctx` and handle
methods (new object-handle members start with `$`; reserved field names never grow),
error and issue codes (apps treat unfamiliar ones as outcomes), `--slop-*`,
`data-hitslop-root` and the embed relay. The engine, rendering helper and live owner ship in one
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
  (manifest, descriptor, theme defaults and the two requirements), `assets` (including
  `app.js`) and optional preview and icon `artwork`, and its initial values as its
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
  state in a fresh read-only page; optional `Export.svelte` supplies the layout, otherwise
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
  JavaScriptCore engine or a second document engine. The WASM core ships only in the
  CLI, for `slop dev` and tests. The native engine validates authoring input.
- TypeBox owns platform contracts (`packages/hitslop/src/schema`). Run `bun run schema:generate`;
  never edit generated files.
- The core checks every file it opens (its layout, rows, markers and resource bounds) and
  every build it packs, not authored UI behavior. Rust validates generated page and
  socket envelopes; Swift checks the page sender and decodes the accepted native models.
- Attachments are host-owned immutable content-addressed blobs in the file's
  `attachments` table, referenced by ordinary document fields.
- Preserve the macOS client: app lifecycle and quit, catalog and Recents, slop windows and
  toolbar, PNG/PDF export, Quick Look previews and Finder icons, Firebase
  Analytics/Crashlytics and Sparkle.
- Active examples are the directories with a `slop.ts` under `examples/slops`;
  `bundled.json` selects shipped templates. Dedicated fixtures own platform semantics. Use plain CSS and
  `slop.ts` theme colors; read `examples/slops/PRODUCT.md` and `docs/guides/authoring.md` for
  visual changes.
- Deferred: collaboration, a document history UI, schema evolution (changing a
  descriptor makes a new document type), synced folders, hosted catalog/publishing,
  accounts and sharing.

Testing policy lives in [testing](testing.md). Agent entrypoint: [AGENTS.md](../AGENTS.md).
