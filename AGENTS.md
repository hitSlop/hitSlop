# hitSlop

Architecture: [architecture](docs/architecture.md). The completed host-owned reset plan
is archived at [archive/docs/HostOwnedReset.md](archive/docs/HostOwnedReset.md).
Contracts: [engineering contract](docs/engineering-contract.md). Tests:
[testing](docs/testing.md). Quick Checklist is the active template; other slops are
archived.

## Non-negotiable

- **Nothing has shipped: start fresh.** No legacy handling, migrations, backwards
  compatibility, version gates, refusal messages or compatibility tests. The app, helper,
  page shell and CLI are built from one tree. Add a version marker only when a first
  public release needs one.
- `hitslop-core` (Rust on Loro) owns document semantics. The Swift `DocumentOwner` owns
  the writer lock, SQLite, saving, the socket and delivery to the page. The page shell
  holds no CRDT, and slops contain only their app.
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
  document engine. The WASM core is for `slop dev` and tests only.
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

Collaboration, undo UI, schema evolution, history pruning, synced folders, hosted
catalog/publishing, accounts/auth and sharing. `archive/`, `_docs/` and `deferred/` are
not active contracts.
