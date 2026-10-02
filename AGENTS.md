# hitSlop

Architecture: [architecture](docs/architecture.md). Status and open work:
[direction](docs/roadmap.md); proposals in [ideas](docs/ideas.md).
Contracts: [engineering contract](docs/engineering-contract.md). Tests:
[testing](docs/testing.md). `examples/slops/bundled.json` selects the bundled templates;
slops not yet on the implemented document kinds are in `archive/slops`.

## Non-negotiable

- **Start fresh: earlier 1.x builds are unsupported.** No legacy handling, migrations
  or backwards-compatible readers. The app, helper, page shell and CLI are built from
  one tree. The release markers are SQLite application/schema identity and the exact
  core build identity checked between CLI, helper and live owner; no version negotiation.
- `hitslop-core` (Rust on Loro) owns document semantics and durable storage: SQLite,
  the writer lock and the save policy. The Swift `DocumentOwner` schedules saves and owns
  the socket and delivery to the page; Loro bytes never reach Swift. The page shell
  (`packages/shell`) holds no CRDT; `packages/document` is the author SDK, and slops
  contain only their app.
- **One edit path.** The CLI forwards to the live owner or takes the lock and runs the
  owner in-process. Never bypass a busy lock or unlink `writer.lock`. Closed edits never
  start WebKit or run authored code.
- **TypeBox owns the wire.** Run `bun run schema:generate`; never edit generated files.
- **Writes are async.** They resolve after the snapshot updates. `change` collectors
  are synchronous. Reads come from immutable snapshots. Preserve `$id` identity; merged
  anomalies are preserved and flagged, never repaired on read.
- Flush before close or export. A failed save keeps ownership and shows a native retry.
  Attachments are host-owned immutable blobs.
- Descriptor kinds exist in the types only once Rust, the SDK and a fixture implement
  them.
- No `stores/data.json`, JSON mirrors or reconciliation, JavaScriptCore engine or second
  document engine. The WASM core is for authoring validation, `slop dev` and tests only.
- Preserve the macOS client (catalog/Recents, windows, PNG/PDF export, Analytics/
  Crashlytics, Sparkle). Masters are immutable; edit copies.

## Authoring

Manifest-bearing directories under `examples/slops` are active; `bundled.json` selects
shipped templates. Use plain CSS and `defineTheme`; read
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
