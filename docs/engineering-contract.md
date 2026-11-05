# Engineering contract

dotpop starts a new product and compatibility lineage at 1.0.0. Legacy hitSlop `.slop` files and APIs are unsupported. Historical hitSlop release evidence and its original hash pin remain immutable; they are not pop acceptance inputs.


How the system works is in [architecture](architecture.md). The current
[foundation redesign](roadmap.md#rust-foundation-redesign) uses layout 2 for new documents. Published layout-1 pop documents retain their original
representation, acceptance and sync history through the layout-1 adapter; opening never
migrates them. Historical release assets and their hash pins remain immutable.

## Compatibility

The compatibility promise starts with the public v1.0.0 release. Pre-launch documents
are unsupported. Every document a released build wrote must open, render, edit, save and reopen in every later
build, on supported macOS versions. Downgrades are not supported: an older build refuses
newer formats with `requires_update` and writes nothing.

Old documents depend on a few public boundaries; everything behind them may change.

| Boundary | Marker | A later build |
|---|---|---|
| The app in a `.pop` file: catalog fields and `definition_json` (window, descriptor, theme, commands, capabilities), plus resource semantics | `app.package_format`, stamped by the engine from the build | Dispatches to the reader for that format |
| App behavior: `ctx`, handles, errors, the command prelude, stored capture programs and host DOM/CSS conventions | `app.runtime_abi`, stamped by the build | Dispatches to the app-facing context adapter for that ABI |
| The file's tables, and the layout every open checks | SQLite `user_version` (storage version) | Migrates forward under the writer lock, in one transaction |
| How descriptor kinds map to Loro containers ([storage layout](reference/document-types.md#storage-layout)) | `meta.layout` in each document, written when it is created | Dispatches to that layout's semantics. New documents use layout 2; released layout-1 documents keep their representation and lineage. Opening, saving and same-document epoch adoption do not upgrade layout 1. An independent sanitized copy may start a new layout-2 lineage. |
| CLI ↔ document engine ↔ live owner | The command protocol: `--client-protocol N` on `pop-engine`, and `protocol` in every socket request; `--protocol` reports the protocol served | Serves exactly its own protocol, with no adapters for older programs, and refuses any other before touching the document (exit status 2, or `requires_update`), naming the older side to update. The refusal path never changes: `--client-protocol N` first, exit status 2 and one stderr line; the live discovery record's `socket` and `documentPath` fields (other fields are ignored; its location is per platform); newline-delimited framing; `protocol` read before any other check; and the reply `{ok: false, code: "rejected", reason: "requires_update", error}`. Protocol 1 is today's commands, arguments, request and reply JSON (a reply may gain members, which readers ignore), outputs and exit statuses |

### Local browser storage

The Chrome beta uses browser-container format 1, separate from the `.pop` storage
version. Each `copies/<id>/container.json` marker is checked before sahpool opens or
cleanup touches that pool. Unknown and unmarked pre-launch containers are refused
without repair or migration. IndexedDB version 1 stores only the copies catalog.
Record and qualify changes to the pinned VFS against persisted pool fixtures before
shipping browser storage publicly; current pre-launch copies remain unsupported.

Import validates the staged SQLite file before renewing its document UUID in one
transaction. Reopening a browser copy retains its UUID. Export drains page drafts,
then independently flushes the Rust owner; a failed flush never exports stale state.
The Save As destination completes only after its writable stream closes. Temporary
exports are unique and protected by the copy lock until the stream finishes or the
owning tab disappears. No browser edit writes back to the original desktop file.

### How documents evolve

A pop carries its own copy of the SDK: its `ui.js` and `commands.js` were compiled with
the SDK its project pinned, Svelte included. Changing the SDK therefore never affects a
released document, and authors move to a new SDK when they rebuild. What old documents
depend on is the host side they call into, the runtime ABI that `sdk/abi.ts` lists. A
host change an old app cannot run raises `runtimeABI` and keeps the old behavior for apps
built at the lower level.

Each marker in the table above has its own mechanism. Only storage is rewritten:
SQL migrations move a file forward. A package format is read in place and translated into
the host model in memory, a runtime ABI keeps its adapter, and a Loro layout is read as it
is or migrated losslessly. The app inside a document is an author's compiled code, so it
is never migrated; moving a document to a newer build of its app is a separate, explicit
operation ([additive app upgrades](ideas.md#additive-app-upgrades)).

Migration is lazy. It runs only on a write that needs the newer structure, under the
writer lock and inside that write's transaction, never at launch or on a display read:
previewing, listing or exporting a file leaves it untouched. A migrated file opens only
in builds that know its new marker, so migrating only when needed keeps a file that two
Macs share openable on both for as long as possible.

Markers are requirements, not release numbers: refactors never raise them, and app,
CLI and SDK versions never stand for them. The Mac app and the npm package share one
release version. A format change that an older build cannot read correctly raises a
marker; a change without one is allowed only when older readers already handle it.
New `ctx` members may be declared optional (`member?:`) with a real fallback; these
additions need no ABI bump. A required member an older host cannot supply raises
`runtimeABI` once the SDK depends on it. A syntax reader never caps `runtimeABI`: the supported runtime is
checked before the reader is chosen. The build stamps `runtimeABI`; the CLI refuses a
project whose SDK version differs from its own, and refuses an ABI it cannot validate or preview. Raising one
adds a reader and leaves released behavior in place. The current format reader is
`app/package_format_1.rs`; the page context is `shell/abi/1.ts`, the command prelude
is built from `shell/abi/runner-1.ts`, and `sdk/abi.ts` lists every part of the app ABI,
including the globals, markup and CSS variables that are not `ctx`. Compile-time guards require a new arm when their marker rises.
The page receives the stored descriptor unchanged, while native UI receives the translated
host model. Storage-1 SQL freezes at the first release; later physical changes use a
writer-locked transactional migration. Every open reads
the markers (application ID, storage version, then `package_format` and `runtime_abi`)
before it compares the exact tables, so a newer file is refused with `requires_update`
even when its tables differ. Saved state belongs to the descriptor in its file's `app`
row, which is written once; no copy of it is stored with the state. App limits and the
checks that run on open are versioned by `packageFormat`. Persistence limits (storage
size and updates, attachments) govern every save as well as every open, so they are
versioned by the storage version: never lowered for a released one, and raised only
together with it. Tightening an authoring rule never rejects a saved document or stops
one from being edited and saved, and an accepted edit stays readable under the markers
its save writes. Each format module therefore holds two rule sets: `checked`, what opening
requires (the shape the host can interpret safely, under the format's own frozen limits;
it may only loosen), and `authoring`, what `pack` and `init` require of a new app (it may
tighten). Opening never re-runs signature sniffing or authoring grammars on a sealed app,
and damaged or oversized artwork reads as absent rather than refusing the document. New
files are created by replaying `storage-1.sql` and then each storage migration in order,
so a migrated file and a new one share one exact layout. A frozen corpus entry records
the command prelude of its runtime ABI; that prelude's bytes are then final. A security fix that must reject old documents needs an assessment and a
recovery path for their data. Installing pop never replaces the app inside an
existing document; upgrading a document's app is an explicit operation. Only a write
under the writer lock migrates a file, validating the result and committing the data and
its markers together; reads that only display it (Quick Look, the catalog, `get`,
export) never migrate it. Public boundaries grow additively: `ctx` and handle
methods (host members start with `$` exactly when the handle also carries authored
fields; handles without authored fields keep plain names; reserved field names never grow),
error and issue codes (apps treat unfamiliar ones as outcomes; the CLI reports an
unrecognized outcome and tells the user to inspect state before retrying; the engine relay
forwards replies with unfamiliar codes unchanged, and the hello reply may gain members), `--pop-*`,
`data-pop-root`, the embed relay, and capture program requests and replies.
The pop sharing `/v1` HTTP/link/socket contract grows only additively; its [immutable fixtures](../crates/pop-sharing-protocol/tests/fixtures/v1/README.md), including both binary frame and socket-envelope codecs, remain valid and are never replaced, regenerated or deleted. Breaking changes require `/v2` while `/v1` continues serving old pop clients. Historical hitSlop fixtures remain preserved separately and are not pop acceptance inputs. The npm package ships the document engine; the Mac app contains its linked core
and restricted evaluator. Release acceptance checks that the engine and the app contain
the same core build. Loro is pinned exactly and upgraded only with the corpus passing.

Limits governing opening, reading and saving are versioned by their owning marker. The corpus records them, and a build may not make a released bound stricter. Acceptance vectors pin positive decoding behavior.

Stored capture programs are governed by runtimeABI. Requests grow additively; reply readers ignore unknown hints. Captures remain faithful and usable, while renderer fixes may change pixels. See the [stored capture program contract](reference/runtime.md#stored-capture-program-abi-runtimeabi-1).

### Open readers

Readers are open and writers careful. A reader of persisted or exchanged JSON ignores the
members it does not know: the stored definition's containers, shared theme files, engine
replies and the output of a stored program, so adding an optional member never breaks an
older build. A writer decides per addition: an ignorable hint needs no marker; an unknown
enum kind, or anything an old reader must not ignore, raises the marker that owns it, and
older builds then refuse with `requires_update` before decoding. Requests to something that
executes them, the descriptor and arguments grammar (an option ignored would weaken
validation), authoring input and same-build wires stay exact. Adding a member to an engine
reply is therefore not a protocol bump. A category spelling this build does not know reads
as `other`.

A named command's reply stays raw JSON until the arm for its runtime ABI decodes it.
The `{ok, intents, result}` success envelope and refusal envelope readers ignore unknown
members. Required fields and types remain checked; intents inside the ABI-1 arm still
decode strictly as executing requests. `Intent`, `Segment`, `Anchor` and `Selection`
are the frozen ABI-1 vocabulary. An incompatible change raises `runtimeABI` and freezes
the old decoder in the same change.

Code reachable from `shell/abi/1.ts` is ABI-1 behavior, including shared helpers. When
new behavior must diverge, preserve or fork the diverging implementation under ABI 1.
The checked-in command prelude is a runtime artifact: ordinary schema generation does
not rebuild it. Explicit `bun run compat:runner` is only for an unreleased ABI; published
prelude hashes are enforced by corpus verification.

The [compatibility corpus](testing.md#compatibility-corpus) is the evidence: every
release's original compiled apps, templates and saved documents, retained as an immutable
GitHub release asset and replayed by affected builds. The release record binds all historical
corpus hashes tested by its candidate. A frozen entry is never edited, regenerated or deleted. The corpus is regression evidence, not proof that every
possible document opens. Apps that embed external services depend on those services.

Counters are exact while increments summed as absolute values over their whole history
stay below 2^53 (about 9×10^15). Beyond that envelope an off-by-one result is possible,
including after replacement, undo or reopen; no exactness is guaranteed. See
[document types](reference/document-types.md#counter).

## Ownership

How an edit, a save and a close move is in [architecture](architecture.md). The rules:

- `pop-core` owns document semantics and app acceptance over Loro. `pop-contracts`
  owns passive wire types and shared limits. `pop-storage` owns the `.pop` database,
  writer lease, storage acceptance and save policy; `pop-runtime` owns the serial
  document owner, persistence worker and socket routing. Native and browser adapters
  use these same boundaries. Swift `DocumentOwner` delivers typed requests and events;
  Loro bytes never reach Swift. The page shell (`packages/pop/src/shell`) holds no CRDT.
- A `.pop` file is one SQLite database. A template holds its app: the `app` row
  (catalog columns, a definition document and two requirements), `assets` (including
  `ui.js`, optional CSS, media and the private command program) and optional preview and icon `artwork`, and its initial values as its
  `checkpoint`. A document also holds its identity, theme overrides, saved state and
  attachments. No engine, source,
  dependencies or caches.

### App identity

`appDigest` hashes a fixed encoding of named app and asset columns with stable domain
and field identifiers, never SQL query text. An inert schema extension therefore never
changes an unchanged app's digest.

Rust distinguishes `AppRevisionId` (those exact digest bytes) from `StateContractId`
(the interpreted descriptor, slug and unordered theme defaults). Descriptions, window
presentation and command metadata do not change the state contract. Descriptor option
presence remains significant; parsed number spelling and floating zero signs do not.
These are internal identities; existing wire `appDigest` fields retain their encoding.

Matching state contracts permits replica compatibility. It does not authorize replacing
an embedded app: a future updater must accept a complete revision, check its runtime
requirements and publish the replacement atomically under the writer lock. App updating
and schema evolution remain deferred.

### Document acceptance and publication

Layout 2 uses deterministic mergeable containers, creation-default projection and
selected payload generations for deletable containers. Clear/remove hides the previous
generation; stale edits to it do not recreate current content. Undo restores the prior
generation. Application row `$id`s are distinct from physical Loro container identities.
The layout-1 adapter retains the released creation, deletion and merge behavior; these
layout-2 rules must not silently change a released document's semantics.

Core is the single acceptance boundary for local edits, accepted saved state and imported
history. Authoring completeness and creation limits are not re-applied as a new rejection
of an accepted merged document: concurrent valid inserts can exceed an authoring list
limit without making that saved state unreadable. Sanitized copying reconstructs accepted
visible state rather than calling new-document authoring validation. Schema evolution is
not inferred from currently equal values.

An accepted visible edit publishes immutable data and effective theme after acceptance.
Publications are ordered by previous/current sequence; row IDs and surviving snapshot
identity remain stable for ordinary edits. A changed authority epoch publishes a full
replacement and resets session undo. Re-adopting the exact same accepted history/state is
a no-op, preserving undo. Async write completion follows snapshot publication; it is not
permission to replay an intent whose outcome is uncertain.

### Checkpoints, copies and authority epochs

An ordinary save appends history or checkpoints it. Routine and explicit checkpoints
retain replica identity and live session undo; a shallow current-frontier checkpoint may
prune old operations while still retaining hidden containers. It is not a sanitized copy.
A sanitized independent copy retains accepted visible values, theme and application row
IDs, creates a new document UUID and physical history, copies only referenced immutable
blobs, and removes stale artwork and hidden historical data. It does not copy a live
sharing membership. It may use layout 2 even when its independent source used layout 1.

A large edited unshared layout-2 document may rebuild its durable history at final close.
The still-live owner retains its undo until close succeeds. Layout 1 instead uses an
identity-preserving shallow checkpoint and remains layout 1. Saved frontier accounting
uses the bytes actually written. If an old live owner continues after a rebuilt close
write, its next save checkpoints its own identity rather than appending updates against
an unrelated durable frontier. Ambiguous write outcomes fence appends until a checkpoint;
definite rolled-back busy/refusal outcomes need not force a full snapshot.

A shared authority cutoff is explicit, not an ordinary history merge. A candidate is
accepted and staged without changing the source owner. The external authority commits
that candidate conditionally before canonical adoption. Adoption checks the exact saved
source version, document UUID, same persisted layout and compatible state contract, then
installs history and referenced blobs in one SQLite write transaction. Source app,
history and blobs are read from one accepted source transaction. Target embedded app
resources are retained; compatible UI revisions need not have the same app revision.
Relay publication independently requires its exact app/resource identity. Same-layout
legacy candidates preserve lineage; same-version adoption preserves session undo.

A valid current authority proof permits explicit recovery when the local copy has edits
outside the confirmed cutoff. Unproven legacy snapshots remain ordinary merge inputs;
they cannot authorize replacing unrelated history. Recovery is an independent, findable
private document, persisted before canonical replacement. Slots distinguish the exact
captured source version as well as the cutoff, so a later retry cannot overwrite earlier
recovered edits. Reuse verifies identity, app and referenced blob bytes. Uncertain writes
retain old undo blobs; reclamation follows acknowledged installation or successful close,
never a failed close. Post-close reclamation rechecks current identity and actual sharing
membership under a fresh writer lease (the browser's existing copy lock remains held).

### Cloud document retention

The cloud seed payload limit is 16 MiB; it is a transport limit, not a smaller
`.pop` acceptance limit. Private sync and collaboration use one persistent document
authority. Membership alone never authorizes discarding history before conditional
publication. Storage pressure or large retained history can trigger a clean candidate;
a large immutable app does not require an epoch on every ordinary sync. Layout-2
candidates rebuild current accepted state; legacy candidates retain their supported
same-lineage representation.

Preparation binds the candidate to an exact source version and confirmed frontier.
Staging is not publication: a conditional DO operation must accept the candidate against
the current seed/frontier before clients adopt it or superseded payloads are reclaimed.
Conflicts reconcile remote state rather than discarding its edits to make a candidate
win. Dirty stale clients preserve unconfirmed work in an independent recovery document
before refreshing canonical history. Interrupted staging, publication and adoption
remain retryable without invalidating the original accepted file.

Firestore references are account-local discovery metadata, not checkpoint authority.
Rust retains only the latest desired pending reference per document until Swift's SDK
acknowledges publication; operation revisions fence stale acknowledgements. An unavailable
catalog does not undo an acknowledged cloud upload or block local editing. Stopping
sharing revokes guests while owner synchronization and the byte authority remain.
There is no CloudKit catalog revision, final-snapshot receipt or authority handoff.
The detailed lifecycle is specified in [cloud document lifecycle](reference/sharing-lifecycle.md).
These rules bound retained latest-state metadata and temporary staging; they do not
promise a fixed size for every accepted visible document, app or recovery copy.

Retention and compatibility assertions are normative. Passing the published corpora is
regression evidence, not a proof over all possible documents. Timing, memory and file-size
measurements are qualification results, not new public format limits. Platform/browser
and release qualification remain tracked in the [roadmap](roadmap.md).

## Rules

- Theme token names reserve the prefixes `window-`, `host-` and `platform-` for the host.

- One edit path. The CLI forwards to the live owner's socket or takes the writer lock
  and runs the owner in-process, without WebKit or authored code. Never bypass a busy
  lock or remove a lock file from the registry (`~/.pop/live`).
- Storage: one write in flight. Failed or unacknowledged saves keep ownership, all edits
  and the last confirmed saved version, and show a native retry. The store re-reads its
  sizes after a failure; it advances the saved version only on confirmed success.
  Loro deduplicates overlapping updates, but mutation intents are never replayed.
- Flush unsent text and pending writes before close or export. Captures render saved
  state through synchronous Svelte SSR and Takumi: restricted children on Mac/Linux,
  in-process capture on iPhone ([capture architecture](architecture.md#close-export-and-capture)).
  Required root `Preview.svelte` supplies preview artwork; optional `Export.svelte`
  supplies PNG/PDF output.
  Successful close destroys WebViews.
- Author schemas with `defineDocument`/`s`: the export is definition data. Editors acquire
  their document with `getDocument(schema)` during component initialization; static views
  receive saved snapshot and attachment URL props. The stored descriptor is not JSON Schema.
  Initial values are read only when a document is created. Descriptor kinds exist only once Rust,
  the SDK and a fixture implement them (today: text, boolean, string, number, integer, enum,
  optional (of a scalar, text or an object), object, list(object), list(scalar),
  record(scalar or object) and integer counter; see
  [document types](reference/document-types.md)).
- Writes are asynchronous and resolve after the snapshot updates; `change(tx => …)`
  collects synchronously. Reads come from immutable snapshots. Preserve `$id`
  identity. Every accepted operation keeps the document valid; invalid stored state is
  refused without modifying the file.
- Never add a JSON copy of the document, persistent JSON mirrors, JSON reconciliation, a
  JavaScriptCore engine or a second document engine. The WASM core serves SDK tests and local browser copies with the same Rust owner and SQLite store. Browser authoring development uses a native owner; the native engine validates authoring input. Browser copies hold a per-copy Web Lock and save through OPFS; authored code runs on a separate per-copy origin. A download drains the page, flushes the owner and streams one SQLite read transaction.
- Rust owns wire types and shared limits (`crates/pop-contracts/src/wire`); ts-rs exports
  TypeScript and UniFFI carries native types to Swift. App acceptance also belongs to Rust; JSON Schema is only a descriptor projection for tool clients. Run `bun run schema:generate`; never edit generated files.
- Storage checks every file it opens (its tables, markers and resource bounds); core
  checks its accepted app definition and document layout/state. Packing also applies
  authoring rules, not authored UI behavior. Rust decodes and checks page/socket requests once; Swift checks the page sender and receives typed native actions.
- Attachments are host-owned immutable content-addressed blobs in the file's
  `attachments` table, referenced by ordinary document fields and served by URL. Media types are sniffed when stored; the first read verifies the full hash and refuses a stored label that does not match the content, except `application/octet-stream`, the fallback any content may carry.
- Agent commands use the owner's descriptor validation and restricted child. Pages use handles and `doc.change`; command implementations stay outside the UI graph. Every evaluation gets a fresh runtime; no command globals persist across calls.
- The app row seals assets at pack time. No app or asset updates/deletes, and no asset insert after the seal, including conflict-replacement forms. Catalog summaries never parse `definition_json` or read app/attachment blobs.
- Preserve the macOS client: app lifecycle and quit, catalog and Recents, pop windows and
  toolbar, PNG/PDF export, Quick Look previews and Finder icons, Firebase
  Analytics/Crashlytics and Sparkle.
- Active examples are the directories with a `pop.ts` under `pop`;
  `bundled.json` selects shipped templates. Dedicated fixtures own platform semantics. Use plain CSS and
  `pop.ts` theme colors; read `pop/PRODUCT.md` and `docs/guides/authoring.md` for
  visual changes.
- Deferred work is tracked in the [roadmap](roadmap.md#later). Changing a descriptor
  currently makes a new document type; schema evolution is deferred.

Testing policy lives in [testing](testing.md). Agent entrypoint: [AGENTS.md](../AGENTS.md).
