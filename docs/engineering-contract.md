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
| Package syntax: manifest, descriptor encoding and immutable package entries | `packageFormat`, stamped by the builder | Dispatches to the package reader for that format |
| App behavior: `ctx`, handles, errors, schema key and host DOM/CSS conventions | `runtimeABI`, stamped from the project's resolved SDK | Dispatches to the app-facing context adapter for that ABI |
| `state/document.sqlite` tables | SQLite `user_version` (storage version) | Migrates forward under the writer lock, in one transaction |
| How descriptor kinds map to Loro containers ([layout 1](reference/document-types.md#storage-layout)) | `meta.layout` in each document, written when it is created | Reads it, or migrates it losslessly (same value, issues, row IDs, text, theme and attachments) in one commit with its marker; snapshots migrate in memory only |
| CLI ↔ app | `hitslop-native --protocol` (`{version, minimum}`) | Selects the adapter named by `--client-protocol`; omission means 1. Keeps serving every protocol from `minimum`; protocol 1 is today's commands, arguments, outputs and exit statuses |

Markers are requirements, not release numbers: refactors never raise them, and app,
CLI and SDK versions never stand for them. An additive `ctx` API still raises
`runtimeABI` once the SDK depends on it; an app may treat an API as optional only where
it has a real fallback. A syntax reader never caps `runtimeABI`: the supported runtime is
checked before the reader is chosen. The builder stamps `runtimeABI` from the project's
SDK and refuses one this CLI cannot validate or preview. Raising one
adds a reader and leaves every released one as it is: copy `validate_v1` (with the
manifest schema it validates against), `createContextV1` or the protocol-1 command tree
rather than editing it, and keep the old layout and storage arms. Saved state
belongs to its descriptor by meaning (`same_schema`), never by spelling. Checks that run
on open are versioned by `packageFormat`, so tightening an authoring rule never rejects a
saved document; a security fix that must reject old documents needs an assessment and a
recovery path for their data. Public boundaries grow additively: `ctx` and handle
methods (new object-handle members start with `$`; reserved field names never grow),
error and issue codes (apps treat unfamiliar ones as outcomes), `--slop-*`,
`data-hitslop-root` and the embed relay. The helper and the live owner ship in one
bundle and keep an exact core build check. Loro is pinned exactly and upgraded only
with the corpus passing.

The [compatibility corpus](testing.md#compatibility-corpus) is the evidence: every
release's packages and saved documents, replayed by every build. A frozen entry is never
edited, regenerated or deleted. The corpus is regression evidence, not proof that every
possible document opens. Apps that embed external services depend on those services.

## Ownership

How an edit, a save and a close move is in [architecture](architecture.md). The rules:

- `hitslop-core` (Rust on Loro, `crates/`) owns document semantics and durable storage
  (the OS writer lock, SQLite, the save policy), and is the only code that opens
  `state/document.sqlite`. Swift `DocumentOwner` schedules saves and owns the socket and
  delivery; Loro bytes never reach Swift. The page shell (`packages/shell`) holds no CRDT.
- A slop package contains only its app: `manifest.json` (with its `packageFormat` and `runtimeABI` requirements),
  `assets/` (including `app.js`), `state.schema.json`, `initial.json`, optional
  QuickLook images and the embedded `.agents/skills/hitslop-document` guidance. No
  engine, state, source, dependencies or caches.

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
  `bundled.json` selects shipped templates. Dedicated fixtures own platform semantics. Use plain CSS and
  `defineTheme`; read `examples/slops/PRODUCT.md` and `docs/guides/authoring.md` for
  visual changes.
- Deferred: collaboration, a document history UI, schema evolution (changing a
  descriptor makes a new document type), synced folders, hosted catalog/publishing,
  accounts and sharing.

Testing policy lives in [testing](testing.md). Agent entrypoint: [AGENTS.md](../AGENTS.md).
