# Engineering contract

How the system works is in [architecture](architecture.md).
**Start fresh: earlier 1.x builds are unsupported.** There is no legacy handling,
migration or backwards-compatible reader. The release markers are SQLite
application/schema identity and exact core build identity between CLI, helper and
live owner. These checks refuse unsupported storage or mismatched tools before
operations; they do not negotiate versions or migrate documents.

## Ownership

How an edit, a save and a close move is in [architecture](architecture.md). The rules:

- `hitslop-core` (Rust on Loro, `crates/`) owns document semantics and durable storage
  (the OS writer lock, SQLite, the save policy), and is the only code that opens
  `state/document.sqlite`. Swift `DocumentOwner` schedules saves and owns the socket and
  delivery; Loro bytes never reach Swift. The page shell (`packages/document`) holds no CRDT.
- A slop package contains only its app: `manifest.json`, `assets/` (including
  `app.js`), `state.schema.json`, `initial.json`, optional QuickLook images and the
  embedded `.agents/skills/hitslop-document` guidance. No engine, runtime metadata,
  state, source, dependencies or caches.

## Rules

- One edit path. The CLI forwards to the live owner's socket or takes the writer lock
  and runs the owner in-process, without WebKit or authored code. Never bypass a busy
  lock or unlink `writer.lock`.
- Storage: one write in flight. Failed or unacknowledged saves keep ownership, all edits
  and the last confirmed saved version, and show a native retry. The store re-reads its
  sizes after a failure; it advances the saved version only on confirmed success.
  Loro deduplicates overlapping updates, but mutation intents are never replayed.
- Flush unsent text and pending writes before close or export. Successful close destroys WebViews.
- Author schemas with `defineDocument`/`s`. `state.schema.json` is a descriptor, not
  JSON Schema. `initial.json` is creation-only. Descriptor kinds exist only once Rust,
  the SDK and a fixture implement them (today: text, boolean, string, number, integer, enum,
  optional (of a scalar, text or an object), object, list(object), list(scalar),
  record(scalar or object) and integer counter; see
  [document types](reference/document-types.md)).
- Writes are asynchronous and resolve after the snapshot updates; `change(tx => …)`
  collects synchronously. Reads come from immutable snapshots. Preserve `$id`
  identity; merged anomalies are preserved and flagged, never repaired on read.
- Never add `stores/data.json`, persistent JSON mirrors, JSON reconciliation, a
  JavaScriptCore engine or a second document engine. The WASM core ships only in the
  CLI, for authoring validation, `slop dev` and tests.
- TypeBox owns platform contracts (`packages/schema`). Run `bun run schema:generate`;
  never edit generated files.
- Native code validates package isolation, symlinks, envelopes and resource bounds, not
  app semantics.
- Attachments are host-owned immutable content-addressed blobs in `state/attachments`,
  referenced by ordinary document fields.
- Preserve the macOS client: TCA features, catalog and Recents, slop windows and
  toolbar, PNG/PDF export, Finder previews, Firebase Analytics/Crashlytics and Sparkle.
- Active examples are the manifest-bearing directories under `examples/slops`;
  `bundled.json` selects shipped templates; slops not yet on the implemented document
  kinds are in `archive/slops`. Dedicated fixtures own platform semantics. Use plain CSS and
  `defineTheme`; read `examples/slops/PRODUCT.md` and `docs/guides/authoring.md` for
  visual changes.
- Deferred: collaboration, document history undo UI, schema evolution, history pruning, synced folders,
  hosted catalog/publishing, accounts and sharing.

Testing policy lives in [testing](testing.md). Agent entrypoint: [AGENTS.md](../AGENTS.md).
