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
  (`packages/hitslop/src/shell`) holds no CRDT; `packages/hitslop/src/sdk` is the author SDK, and slops
  contain only their app.
- **One edit path.** The CLI forwards to the live owner or takes the lock and runs the
  owner in-process. Never bypass a busy lock or remove a lock file from the registry
  (`~/.hitslop/live`). Closed owner edits never start WebKit or run authored code; named commands evaluate in a restricted child that returns intents to the owner.
- **Rust owns wire types and shared limits; ts-rs generates TypeScript, and UniFFI
  carries native types to Swift.** Rust also owns app acceptance; authors use `s.*` descriptors. JSON Schema is only a `describe` projection. Run
  `bun run schema:generate`; never edit generated files.
- **Writes are async.** They resolve after the snapshot updates. `change` collectors
  are synchronous. Reads come from immutable snapshots. Preserve `$id` identity. Every
  accepted operation keeps the document valid; invalid stored state is refused without
  modifying the file.
- Flush before close or export. A failed save keeps ownership and shows a native retry.
  Attachments are host-owned immutable blobs in the file.
- Descriptor kinds exist in the types only once Rust, the SDK and a fixture implement
  them.
- No JSON copy of the document, JSON mirrors or reconciliation, JavaScriptCore engine or second
  document engine. The WASM core is for tests only; `slop dev` uses the native Rust owner; native engine validation owns authoring checks.
- Preserve the macOS client (catalog/Recents, windows, PNG/PDF export, Analytics/
  Crashlytics, Sparkle). Masters are immutable; edit copies.

## Compatibility

Before launch we start fresh: no legacy handling, migrations or backwards-compatible
readers; pre-launch documents are unsupported and `tests/compat/dev` may be replaced.
From the first public release, a newer hitSlop must open, render, edit, save and reopen
every document a released build wrote. Downgrades are not supported. The full rules, by
boundary and marker, are in the [engineering contract](docs/engineering-contract.md#compatibility).

- The frozen corpus (`tests/compat/<release>/`, `"frozen": true`) passes in every build.
  Never edit, regenerate or delete a frozen entry, its expectations or the command
  prelude it froze; capture one per release ([releasing](docs/guides/releasing.md)).
- Persisted formats change only additively, or behind the marker that owns the change:
  `packageFormat` (the stored definition), `runtimeABI` (app-facing behavior), the storage
  version (tables, columns, triggers) or the document layout. Markers are requirements,
  not release numbers; refactors never raise them. A newer marker is refused with
  `requires_update` and nothing is written.
- Opening a file applies only its format's acceptance, which never tightens. Authoring
  rules run at `pack` and `init` and may tighten freely; they never judge a saved file.
- Installing hitSlop never replaces the app inside an existing document. Only a write
  under the writer lock migrates a file; display reads never do.
- Public boundaries (`ctx`, handles, error codes, `--slop-*`, host markup, the embed relay,
  the command protocol's refusal path) grow additively. A change an old app cannot run
  raises `runtimeABI` and keeps the old behavior through an adapter.
- Upgrade Loro (pinned exactly) only with the corpus passing.

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
- One runner, `bun run verify` ([testing](docs/testing.md)): it runs the tiers whose
  inputs changed since they last passed. Iterate with one tier (`bun run verify rust
  store::`); run `bun run verify` before calling a step done, and `bun run verify --native`
  once at the end when Swift, the FFI or the helper changed. Release: `bun run release:check`.

## Commit and PR attribution

- Do not credit AI assistants in author/committer fields, `Co-authored-by` trailers,
  or generated-by signatures on commits or pull requests. Human co-authors are welcome, and ordinary
  discussion of AI tools is allowed.
- The required `Attribution` check examines every incoming commit and the PR title
  and description using trusted code from the default branch. Remove flagged credit,
  amend/rebase the affected commits, and update the PR; changing the checker in the
  same PR does not change the policy applied to it.
- Known assistant identities and signatures are maintained in
  `scripts/ci/attribution.ts`. Shared Claude settings prevent its default attribution;
  they do not replace the required check. Review the final merge message too.

## Deferred

Collaboration, a document history UI, schema evolution, synced folders, hosted
catalog/publishing, accounts/auth and sharing. `archive/`, `_docs/` and `deferred/` are
not active contracts.
