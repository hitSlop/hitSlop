# tinys

tinys starts a new product and compatibility lineage at 1.0.0. Legacy hitSlop `.slop` files and APIs are unsupported. Historical hitSlop release evidence and its original hash pin remain immutable; they are not tinys acceptance inputs.


Architecture: [architecture](docs/architecture.md). Status and open work:
[direction](docs/roadmap.md); proposals in [ideas](docs/ideas.md).
Contracts: [engineering contract](docs/engineering-contract.md). Tests:
[testing](docs/testing.md).

## Non-negotiable

- **Every released tiny stays openable.** See [Compatibility](#compatibility).
- `tinys-core` (Rust on Loro) owns document semantics and durable storage: the `.tiny`
  file (one SQLite database holding the app and its saved state), the writer lock and the
  save policy, serial owner, persistence worker and socket routing. Swift `DocumentOwner`
  is the native façade; `DocumentSession` delivers events to the page. Loro bytes never reach Swift. The page shell
  (`packages/tinys/src/shell`) holds no CRDT; `packages/tinys/src/sdk` is the author SDK, and tinys
  contain only their app.
- **One edit path.** The CLI forwards to the live owner or takes the lock and runs the
  owner in-process. Never bypass a busy lock or remove a lock file from the registry
  (`~/.tinys/live`). Closed owner edits never start WebKit or run authored code; named commands evaluate in a restricted child that returns intents to the owner.
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
  document engine. The WASM core serves SDK tests and local Chrome browser copies using the same Rust owner and SQLite store. `tiny dev` uses the native Rust owner; native engine validation owns authoring checks.
- Preserve the macOS client (catalog/Recents, windows, PNG/PDF export, Analytics/
  Crashlytics, Sparkle). Masters are immutable; edit copies.

## Compatibility

Starting with public v1.0.0, a newer tinys must open, render, edit, save and reopen
every document a released build wrote. Downgrades are not supported. The full rules, by
boundary and marker, are in the [engineering contract](docs/engineering-contract.md#compatibility).
Pre-launch documents are unsupported; there is no prelaunch corpus fallback.

- Every public release carries its frozen compatibility corpus as an immutable release
  asset. Affected builds replay all published corpora. Never replace, regenerate or delete
  one, its expectations or the command prelude it froze. Capture is part of the candidate
  workflow ([releasing](docs/guides/releasing.md)). Retain the first release's hash pin in
  `scripts/compat/tinys-required-history.json` permanently.
- Persisted formats change only additively, or behind the marker that owns the change:
  `packageFormat` (the stored definition), `runtimeABI` (app-facing behavior), the storage
  version (tables, columns, triggers) or the document layout. Markers are requirements,
  not release numbers; refactors never raise them. A newer marker is refused with
  `requires_update` and nothing is written.
- Opening a file applies only its format's acceptance, which never tightens. Authoring
  rules run at `pack` and `init` and may tighten freely; they never judge a saved file.
- Installing tinys never replaces the app inside an existing document. Only a write
  under the writer lock migrates a file; display reads never do.
- Public boundaries (`ctx`, handles, error codes, `--tiny-*`, host markup, the embed relay,
  the command protocol's refusal path, capture program requests and replies) grow additively.
  A change an old app cannot run raises `runtimeABI` and keeps the old behavior through an adapter.
  Stored capture programs are governed by runtimeABI. Requests grow additively; reply readers
  ignore unknown hints. Captures remain faithful and usable, while renderer fixes may change pixels.
- Upgrade Loro (pinned exactly) only with the corpus passing.

## Authoring

Directories with a `tiny.ts` under `@hitslop/tinys` are active; `bundled.json` selects
shipped templates. Use plain CSS and `tiny.ts` theme colors; read
[product guidance](tinys/PRODUCT.md) and [authoring](docs/guides/authoring.md)
for visual changes. `_vibe` is inspiration only.

## Testing

- **Do not create an automated test or test suite for each tiny.** Creating, styling,
  animating, or updating a tiny does not require new tests. Validate it with existing
  checks/builds and hands-on preview/export review. Shared SDK, storage, or host
  regressions belong in the existing tests at their owning boundary; do not duplicate
  that coverage in example-specific tests. Use minimal infrastructure fixtures, never
  a live example as a dependency of ordinary tests. Generic shipped-template smoke
  checks and frozen compatibility replay are separate artifact contracts.
- Tests live at the owning boundary: Rust semantics, the SDK over WASM, and Swift
  integration. Delete tests together with the code they protect. No tests of private
  call sequences, CSS strings or version numbers.
- A bug regression test must fail before the fix for the intended reason.
- One runner, `bun run verify` ([testing](docs/testing.md)): it runs the tiers whose
  inputs changed since they last passed. Iterate with one tier (`bun run verify rust
  store::`); run `bun run verify` before calling a step done, and `bun run verify --native`
  once at the end when Swift or the FFI changed. Release: `bun run release:check`.
- Rust builds and a full `bun run verify` can outlast an agent's command timeout: run
  them in the background and iterate with a filtered tier.

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

A document history UI, schema evolution, synced folders and hosted catalog/publishing
remain deferred. Live collaboration connects the existing Rust owners through a Durable
Object byte relay and is available in the Mac app and the iPhone app on TestFlight;
release qualification remains in the [roadmap](docs/roadmap.md). `archive/`,
`spikes/`, `plans/`, `TINYS.todo`, `apps/promo/inspo/`, `_docs/` and `deferred/` stay
local and are not active contracts. Shared decisions belong in the architecture, roadmap
and ideas pages.
