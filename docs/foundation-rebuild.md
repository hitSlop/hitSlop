# Foundation rebuild

Branch `rust-foundation-rebuild`. The rebuild ships as a fresh v1: pop 2.0.0, document
lineage 2, on top of the existing app. Nothing from the earlier product is supported:
no legacy readers, migrations, adapters or fallback paths. This page records what
shipped and what was deleted; current behavior is in [architecture](architecture.md),
the [engineering contract](engineering-contract.md) and [cloud documents](cloud-documents.md).

## What shipped

- **One lineage check.** SQLite `application_id` at open refuses a file made by an
  older product ("made by an older version") and writes nothing. Format markers restart at
  1; a newer marker is refused with `requires_update`. Hot-journal recovery, one-owner edits
  and opening any `.pop` from disk are unchanged.
- **Cloud documents.** Persistent Cloudflare Durable Objects and R2 own cloud state and
  permissions; clients maintain their own Firestore references.
- **Fold-only compaction.** `/compact` swaps retained updates for one opaque base. Past the
  16 MiB base cap sync pauses as `Outgrown`, local edits keep saving, and the person forks
  with **Save Unshared Copy…**. The first upload seeds the document from the creator's saved
  file as one snapshot (`Request::StageSeed`, `Store::stage_seed`).
- **One Firebase app.** One default `FirebaseApp` (Auth, Firestore, Crashlytics, Analytics)
  on macOS and iOS; telemetry collection is Release-only. Link guests use Anonymous Auth
  automatically; Apple linking keeps the UID. The catalog is `users/{uid}/pops/{documentId}`.
- **Event-driven transport.** A native Tokio transport without polling, and the pure
  `pop-sync` supervisor shared by native and browser drivers.
- **One route table.** `ROUTES` in `pop-sharing-protocol` drives Worker dispatch, rate
  limiting and the generated OpenAPI; unknown routes 404, oversize bodies 413, malformed
  400. There is no account document cap.
- **Tooling.** `tools/pop-bindgen` is the only generator of bindings and OpenAPI; errors are
  typed through storage, runtime and the adapters.

## What was deleted

- Sharing epochs, checkpoint publication and epoch recovery: client, storage, runtime,
  WASM, protocol types and the Worker's `/epoch` and `/epochs/*` routes.
- The layout-1 adapter and `package_format_1`; `cloudkit://library`, CloudKit sync and the
  iCloud library; `adopt_local_cache` and the old library migrations; local library
  discovery.
- The second Firebase app; `appRevision`, `previewRef` and `accessGeneration` in catalog
  entries; the Worker's lineage prefixing, the `X-Pop-Lineage` header and the abuse module.
- Old-product compatibility pins and corpora from the active verification tree (the
  published release assets are untouched).

## Decisions

- **Fold only.** Epochs cost more complexity than their benefit; a document that outgrows
  the cap is forked.
- **One lineage check.** Isolation in the cloud comes from fresh infrastructure names at
  deploy, not from code.
- **No cap, one app.** No account document cap; one Firebase app on both platforms.
- **Fresh names.** R2 bucket `dotpop-documents` and Analytics dataset `dotpop_documents`
  (see `apps/sharing/wrangler.jsonc`). The Worker name and migration tag are the owner's
  decision; see the [operator checklist](../apps/sharing/README.md#operator-checklist).

## Verification

Run `bun run verify`, `bun run verify --native` (Swift and FFI changed),
`bun run verify browser,sharing` (Miniflare and the Firebase emulators) and
`bun run release:check` ([testing](testing.md)).

Final qualification (2026-11-07): `bun run verify` (contracts, types, bun, cli, render, rust, landing,
packed), `bun run verify --native` (Swift and FFI), `bun run verify browser,sharing`
passed on the final tree. `pop-iOS` compiles for the simulator and the device and runs on two physical
iPhones. iOS unit tests pass; the lifecycle UI tests for edge swipe, keyboard and creating from a template
pass, and the Files round-trip UI test is known to be flaky (the system Save sheet sometimes does not
appear within 15 s). iOS UI tests need `-framework absl` on the command line. `release:check` needs the
candidate corpus captured by the release workflow and has not run locally; the hands-on checks below
have not run.

## Open release steps

- Firebase Anonymous Auth is enabled and `FIREBASE_WEB_API_KEY` is set on the host Worker.
  An additive `users/{uid}/pops` rule is live; deploy the repository's final `firestore.rules`
  (which drops the older collections) with the 2.0.0 app, and create the fresh R2 bucket.
- Deploy the `dotpop` Worker, then the host Worker.
- Capture the first lineage-2 corpus on the release candidate and pin it after publication.
- Hands-on: open an older `.pop` (refused, file unchanged), two-device edit, offline then
  reconnect, grow a shared document past the cap, sign out, anonymous join then Apple upgrade.
