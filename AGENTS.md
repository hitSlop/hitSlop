# dotpop

dotpop’s current document format begins with `@hitslop/pop` 3.0.0. A `.pop` opens in
the Mac and iPhone apps. Files with another SQLite application ID are refused without edits.

Architecture: [architecture](docs/architecture.md). Status and open work:
[direction](docs/roadmap.md); proposals in [ideas](docs/ideas.md).
Contracts: [engineering contract](docs/engineering-contract.md). Tests:
[testing](docs/testing.md).

## Non-negotiable

- **From the first public release of this format, every released pop stays openable.** See
  [Compatibility](#compatibility).
- `pop-core` (Rust on Loro) owns document semantics and app acceptance. `pop-storage`
  owns durable `.pop` storage (one SQLite database holding the app and its saved state),
  the writer lock and save policy; `pop-runtime` owns the serial owner, persistence worker
  and socket routing, with capture in `pop-runtime::capture`. `pop-runner::render` owns
  rendering; `pop-sharing-protocol::supervisor` owns pure sync decisions. `pop-cloud`
  owns native transport and the device library. Swift `DocumentOwner`
  is the native façade; `DocumentSession` delivers events to the page. Loro bytes never reach Swift. The page shell
  (`packages/pop/src/shell`) holds no CRDT; `packages/pop/src/sdk` is the author SDK, and pop
  contain only their app.
- **Cloud documents have one authority.** Persistent Cloudflare Document DOs and R2 own
  cloud state and permissions. Document Objects publish derived library metadata to
  per-account Durable Objects; clients subscribe through one account connection and
  retain an offline local index. Library metadata never grants document access.
  One default `FirebaseApp` serves Auth, Analytics and Crashlytics. Creating cloud
  documents and joining require a named sign-in with Apple; local documents need no
  account. See [cloud documents](docs/cloud-documents.md).
- **One edit path.** The CLI forwards to the live owner or takes the lock and runs the
  owner in-process. Never bypass a busy lock or remove a lock file from the registry
  (`~/.pop/live`). Closed owner edits never start WebKit or run authored code; named commands evaluate in a restricted child that returns intents to the owner.
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
  document engine. The WASM core serves SDK tests only. `pop dev` uses the native Rust owner; native engine validation owns authoring checks.
- Preserve the macOS client (catalog/Recents, windows, PNG/PDF export, Analytics/
  Crashlytics, Sparkle). Masters are immutable; edit copies.

## Compatibility

The compatibility promise begins with this format’s first public release, 3.0.0:
from then on a newer pop must open, render, edit, save and reopen every document a
released build wrote. Downgrades are not supported. The full rules, by boundary and marker,
are in the [engineering contract](docs/engineering-contract.md#compatibility).

- **Identity is one check**: SQLite `application_id` (`0x504F_5003`), read at open.
  A mismatch is refused as “This is not a pop document” and nothing is written. SQLite
  may first recover a crashed transaction’s hot journal, a policy to preserve. The
  live-owner hello carries the same application ID so the CLI checks the owner’s format.
- **Markers** are `packageFormat` (the stored definition), `runtimeABI` (app-facing
  behavior), and the storage version (tables, columns, triggers). The package format
  implies the Loro container layout. All start at 1. Persisted formats change only additively, or behind the marker that owns the
  change. A newer marker is refused with `requires_update` and nothing is written. Markers
  are requirements, not release numbers; refactors and mechanical extractions never raise
  them. There are no adapters for values below 1; raising a marker adds a reader and keeps
  the released one.
- Every public release carries its frozen compatibility corpus as an immutable release
  asset. Affected builds replay all published corpora for this format. Never replace,
  regenerate or delete one, its expectations or the command prelude it froze. Capture is
  part of the candidate workflow ([releasing](docs/guides/releasing.md)). Pin the first
  release's corpus hash in `scripts/compat/dotpop-required-history.json` permanently.
- Opening a file applies only its format's acceptance, which never tightens. Authoring
  rules run at `pack` and `init` and may tighten freely; they never judge a saved file.
- Installing pop never replaces the app inside an existing document. Future storage
  changes run only on a write under the writer lock; display reads never change a
  file’s format.
- Public boundaries (`ctx`, handles, error codes, `--pop-*`, host markup, the embed relay,
  the command protocol's refusal path, capture program requests and replies) grow additively.
  A change an old app cannot run raises `runtimeABI` and keeps the old behavior through an adapter.
  Stored capture programs are governed by runtimeABI. Requests grow additively; reply readers
  ignore unknown hints. Captures remain faithful and usable, while renderer fixes may change pixels.
- Upgrade Loro (pinned exactly) only with the corpus passing.

## Authoring

Directories with a `pop.ts` under `@hitslop/pop` are active; `bundled.json` selects
shipped templates. Use plain CSS and `pop.ts` theme colors; read
[product guidance](pop/PRODUCT.md) and [authoring](docs/guides/authoring.md)
for visual changes. `_vibe` is inspiration only.

## Testing

- **Do not create an automated test or test suite for each pop.** Creating, styling,
  animating, or updating a pop does not require new tests. Validate it with existing
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
`spikes/`, `POP.todo`, `apps/promo/inspo/` and `_docs/` stay
local and are not active contracts. Shared decisions belong in the architecture, roadmap
and ideas pages.
