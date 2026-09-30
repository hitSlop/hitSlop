# Engineering contract

How the system works is in [architecture](architecture.md).
**Nothing has shipped to production.** There is no legacy handling, migration,
backwards compatibility, version gate or refusal message for older packages or
databases. Add a version marker only when a first public release needs one.

## Ownership

- `hitslop-core` (Rust on Loro, `crates/`) owns document semantics: descriptors,
  validation, `$id` rows, atomic batches, publications, issues, counters and text merges.
- Swift `DocumentOwner` owns the live core handle, the OS writer lock, SQLite
  (`state/document.sqlite`), saving, the socket and delivery to the page.
- The page shell (`packages/document`, served by the host at `/__shell__/`) holds no
  CRDT. It turns intents into requests and publications into immutable snapshots.
- A slop package contains only its app: `manifest.json`, `assets/` (including
  `app.js`), `state.schema.json`, `initial.json`, optional QuickLook images and the
  embedded `.agents/skills/hitslop-document` guidance. No engine, runtime metadata,
  state, source, dependencies or caches.

## Rules

- One edit path. The CLI forwards to the live owner's socket or takes the writer lock
  and runs the owner in-process, without WebKit or authored code. Never bypass a busy
  lock or unlink `writer.lock`.
- Storage: one write in flight; each write records an attempt token in the same SQLite
  transaction, and a lost reply is resolved by comparing tokens, never by guessing from
  the generation. Failed saves keep ownership and all edits and show a native retry.
- Flush unsent text and pending writes before close or export. Successful close destroys WebViews.
- Author schemas with `defineDocument`/`s`. `state.schema.json` is a descriptor, not
  JSON Schema. `initial.json` is creation-only. Descriptor kinds exist only once Rust,
  the SDK and a fixture implement them (today: text, boolean, string, number, integer, enum,
  optional (of a scalar or an object), object, list(object) and integer counter).
- Writes are asynchronous and resolve after the snapshot updates; `change(tx => …)`
  collects synchronously. Reads come from immutable snapshots. Preserve `$id`
  identity; merged anomalies are preserved and flagged, never repaired on read.
- Never add `stores/data.json`, persistent JSON mirrors, JSON reconciliation, a
  JavaScriptCore engine or a second document engine. The WASM core ships only in the
  CLI, for `slop dev` and tests.
- TypeBox owns platform contracts (`packages/schema`). Run `bun run schema:generate`;
  never edit generated files.
- Native code validates package isolation, symlinks, envelopes and resource bounds, not
  app semantics.
- Attachments are host-owned immutable content-addressed blobs in `state/attachments`,
  referenced by ordinary document fields.
- Preserve the macOS client: TCA features, catalog and Recents, slop windows and
  toolbar, PNG/PDF export, Finder previews, Firebase Analytics/Crashlytics and Sparkle.
- Active examples are the manifest-bearing directories under `examples/slops`;
  `bundled.json` selects shipped templates. Quick Checklist is the active template;
  other slops are archived. Dedicated fixtures own platform semantics. Use plain CSS and
  `defineTheme`; read `examples/slops/PRODUCT.md` and `docs/guides/authoring.md` for
  visual changes.
- Deferred: collaboration, undo UI, schema evolution, history pruning, synced folders,
  hosted catalog/publishing, accounts and sharing.

Testing policy lives in [testing](testing.md). Agent entrypoint: [AGENTS.md](../AGENTS.md).
