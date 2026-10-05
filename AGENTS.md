# hitSlop

Architecture: [architecture](docs/architecture.md). Status and open work:
[direction](docs/roadmap.md); proposals in [ideas](docs/ideas.md).
Contracts: [engineering contract](docs/engineering-contract.md). Tests:
[testing](docs/testing.md).

## Non-negotiable

- **Every released slop stays openable.** See [Compatibility](#compatibility).
- `hitslop-core` (Rust on Loro) owns document semantics and durable storage: the `.slop`
  file (one SQLite database holding the app and its saved state), the writer lock and the
  save policy, serial owner, persistence worker and socket routing. Swift `DocumentOwner`
  is the native façade; `DocumentSession` delivers events to the page. Loro bytes never reach Swift. The page shell
  (`packages/shell`) holds no CRDT; `packages/document` is the author SDK, and slops
  contain only their app.
- **One edit path.** The CLI forwards to the live owner or takes the lock and runs the
  owner in-process. Never bypass a busy lock or remove a lock file from the registry
  (`~/.hitslop/live`). Closed edits never start WebKit or run authored code.
- **TypeBox owns the wire.** Run `bun run schema:generate`; never edit generated files.
- **Writes are async.** They resolve after the snapshot updates. `change` collectors
  are synchronous. Reads come from immutable snapshots. Preserve `$id` identity; merged
  anomalies are preserved and flagged, never repaired on read.
- Flush before close or export. A failed save keeps ownership and shows a native retry.
  Attachments are host-owned immutable blobs in the file.
- Descriptor kinds exist in the types only once Rust, the SDK and a fixture implement
  them.
- No JSON copy of the document, JSON mirrors or reconciliation, JavaScriptCore engine or second
  document engine. The WASM core is for `slop dev` and tests only; native engine validation owns authoring checks.
- Preserve the macOS client (catalog/Recents, windows, PNG/PDF export, Analytics/
  Crashlytics, Sparkle). Masters are immutable; edit copies.

## Compatibility

Before launch we start fresh: no legacy handling, migrations or backwards-compatible
readers; pre-launch documents are unsupported and `tests/compat/dev` may be replaced.
From the first public release, a newer hitSlop must open, render, edit, save and reopen
every document a released build wrote. Downgrades are not supported.

- The frozen corpus (`tests/compat/<release>/`, `"frozen": true`) passes in every build.
  Never edit, regenerate or delete a frozen entry or its expectations; capture one per
  release ([releasing](docs/guides/releasing.md)).
- Persisted formats change only additively, or behind a marker the reader dispatches
  on: the file's `packageFormat` and `runtimeABI` requirements (its `app` columns), the
  SQLite storage version (`user_version`; forward migration under the writer lock) or
  the document layout (read it, or migrate losslessly). The markers are requirements,
  not release numbers; refactors never raise them. A build refuses a newer marker with
  `requires_update` and writes nothing.
- App format changes (the `app` row, assets, artwork) raise `packageFormat`; app-facing
  behavior raises `runtimeABI`. These requirements evolve independently. Checks that run
  on open are versioned by `packageFormat`, so tightening an authoring rule never
  rejects a saved document; a security fix that must reject old documents needs an
  assessment and a recovery path for their data.
- Public boundaries grow additively: `ctx` and handle methods (new object-handle members
  start with `$`; reserved field names never grow), error codes (apps treat unknown
  ones as outcomes), `--slop-*`, `data-hitslop-root`, the embed relay, and the helper's
  command protocol (`hitslop-native --protocol`). A change an old app cannot run raises
  `runtimeABI` and keeps the old behavior through an adapter. Internals behind them are free.
- Upgrade Loro (pinned exactly) only with the corpus passing. The helper and the live
  owner ship in one bundle and keep their exact build check.

## Authoring

Directories with a `slop.ts` under `examples/slops` are active; `bundled.json` selects
shipped templates. Use plain CSS and `slop.ts` theme colors; read
[product guidance](examples/slops/PRODUCT.md) and [authoring](docs/guides/authoring.md)
for visual changes. `_vibe` is inspiration only.

## Testing

- Tests live at the owning boundary: Rust semantics, the SDK over WASM, and Swift
  integration. Delete tests together with the code they protect. No tests of private
  call sequences, CSS strings or version numbers.
- A bug regression test must fail before the fix for the intended reason.
- Everyday: `bun run check && bun run test` and `cargo test --locked --workspace`.
  Native: `bun run build && bun run swift:test && bun run test:native`.
  Release: `bun run release:check`.

## Deferred

Collaboration, a document history UI, schema evolution, synced folders, hosted
catalog/publishing, accounts/auth and sharing. `archive/`, `_docs/` and `deferred/` are
not active contracts.
