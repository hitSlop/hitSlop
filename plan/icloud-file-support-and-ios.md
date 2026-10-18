# Current delivery decision (2026-10-18)

This section supersedes the earlier proposals below. Earlier spike results are retained as evidence.

Use one checkout with sequential branches from freshly fetched origin/master:

1. `feat/local-save-location`: remember a usable local save folder; fall back from synced Desktop to ~/hitSlop; offer an explicit move-and-open action for documents already in iCloud. The Rust file layer holds the source lease, validates and durably publishes the local copy, then authorizes coordinated trashing. Cleanup failure retains the destination and reports that the original remains.
2. `feat/ios-local-editor`: prove the existing Rust owner/storage and page shell on iOS 18+, without CloudKit. Build device and simulator slices, provide UIKit adapters, and verify data/theme edits, undo, attachments, Files export, reopening on Mac, and bounded background flushing. Retain useful build/host work for M1.
3. M2: Synced Library in the existing Mac app and iPhone app. One account-level CKSyncEngine; Rust owns sealed payloads and delivery semantics. Library documents use the existing shared state for history and attachment retention. Add stable CLI discovery/addressing, suppress Revert/Discard throughout the UI, and add live-state rescue without flushing full history first. Imported source files remain independent. Library data, theme and attachments sync both ways.
4. M3: qualify automatic delivery on two physical devices. History compaction must define membership, retirement, offline rejoin and frontiers jointly with the relay; acknowledgement alone is insufficient. Reassess whether iCloud Drive publication is still useful after M2.

The pre-existing dirty work was saved in named stash `93177537d664a709f1382a3babc123eacc0a4707` on `feat/icloud-file-support`. The same tracked/untracked snapshot is also committed as `402944e947a91d2a8cb04dcbe0c4f3596bb71814` on `wip/icloud-file-support-before-ios`. Ignored spikes remain in place and are not in that commit. No pause/resume backend is being ported.

## Implementation evidence

Mac implementation:
- Rust relocation uses coordinated read and delete accessors, a source registry lease, SQLite Backup into the existing exclusive/durable staged publisher, saved-state validation and a source metadata fence before trashing. It preserves UUID and retained history. Cleanup failure returns the usable destination plus a warning.
- The local save default checks both ubiquitous ancestors and the iCloud Drive prefix. Catalog creation and Duplicate remember successful local destinations. Finder open resolves URLs off the main actor and offers the explicit move.
- Passed: relocation boundary test (busy lease, collisions, source replacement, cleanup failure, immutable templates), Clippy, and Swift local-folder integration test. Full `bun run verify --native` passed in 724.9 seconds: 404 Rust tests, all selected Swift/CLI/render/packaged/docs tiers and frozen 1.0.0/1.0.1 compatibility replay. Evidence: `.hitslop/evidence/runs/2026-10-18T16-11-03-654Z-60111`. No physical iPhone result is claimed yet. Real-provider relocation UI still needs hands-on qualification; failure semantics were exercised with provider stand-ins.

Verification and device results will be recorded here as they complete. Physical-device results are required for the phone gate; simulator evidence alone is insufficient.

---

# iCloud file support: implementation, hardening, and iOS handoff

- Updated: 2026-10-18
- Branch: `feat/icloud-file-support`
- Starting point: fetched `origin/master`, `67bf4de4`
- Status: the pause-based implementation remains unqualified and unchanged. A separate
  private-working-copy core spike now tests the alternative below, including APFS clones
  and Foundation replacement. A separate signed development-container CloudKit proof
  tests the language split. No iOS implementation or production backend replacement is included.

This is a working handoff and proposal, not a new engineering contract. Accepted
decisions belong in the [architecture](../docs/architecture.md),
[roadmap](../docs/roadmap.md), and [engineering contract](../docs/engineering-contract.md).
The user requested this singular `plan/` location; the repository also has a separate,
ignored `plans/` directory.

## Review and the next storage decision

Claude's review is recorded in
[`generic-greeting-dragonfly.md`](/Users/jordan/.claude/plans/generic-greeting-dragonfly.md).
The user approved a **core proof first** spike before deciding whether to replace the
OS 26 pause design. This is a scoped experiment, not a change to the shipping backend.
The original implementation and its verification are retained below for comparison.

The earlier private-working-copy approach was removed when the product choice narrowed
to OS 26 sync controls. There was no recorded benchmark or decisive safety finding
against private publication. Calling it a second document engine was not a sufficient
reason: one Rust owner can edit one private SQLite store and export a complete `.slop`
to the public path using the same format. The public file is an artifact of that owner.

Two architectural concerns justify the spike:

- The pause applies to the main item. SQLite's rollback journal is a distinct sibling.
  Local hot-journal recovery does not establish that a provider keeps the pair in sync.
  Transferring a journal from a later main-file generation to an older copy can make
  recovery unsafe. Provider-specific behavior remains unproven; do not claim that an
  automated local crash test rules out this scenario.
- The OS 26 pause survives termination. A suspended or terminated iPhone cannot be
  relied on to finish a resume, so this couples other devices' progress to relaunch.

Apple's [UIDocument](https://developer.apple.com/documentation/uikit/uidocument)
documents safe saving through a temporary file and replacement. That supports the
alternative's direction; it does not automatically supply our Rust ownership, private
recovery or provider-conflict policy. SQLite's
[corruption guidance](https://www.sqlite.org/howtocorrupt.html) explains both the
main-file/journal pairing and descriptor-close lock hazards addressed in the spike.

### Review items retained as merge/release gates

These findings apply to the existing pause implementation. The spike does not fix them
in production, and prior green tests do not supersede them.

| Finding | Required outcome | Acceptance evidence still needed |
| --- | --- | --- |
| Read policy on macOS 15.2 (`file/access.rs`, `Access::run`) | Allow read-only inspection, Quick Look and iCloud templates; enforce the edit policy at writer/destination entry points. | Real 15.2 previews, Recents summaries and create-from-template work; unsupported edit opens explain the local alternative. |
| Catalog materialization (`CatalogScanner`, `SlopFile.resolvedRoot`) | Listing a cloud-only recent shows a placeholder; only explicit open downloads it. | Evict several test files, list offline and online, observe no downloads; explicitly open one. |
| Permanent external-change refusal (`Store::check`, `Store::access`) | Keep an owner-mediated rescue action for accepted edits, including referenced attachments. Scope local-file fences separately until that action exists. | External history/artwork/attachment edits cannot be overwritten; Save a Copy preserves current edits and obtains a fresh UUID. |
| Local read/write cost (`Access::run`, `Store::access`, `ResourceReader`) | Avoid per-resource provider coordination and serialization on private/local owned reads; measure the write policy. | Compare open/save/range-read/close timings with master and play media during saves. A timed cloud-location cache alone does not close a sync-enabled-mid-session race. |
| Global recovery worker (`Store::open`, `start_recovery`) | App-controlled startup and document-specific recovery; a read-only CLI command must not recover unrelated files. | Open A through the CLI with B's abandoned intent present; B stays untouched and no unrelated lease is taken. |
| Public availability claims | Keep iCloud experimental or hold the claims until the real-provider matrix passes. | Record OS/device/provider/network and results, including two devices and macOS 15.2. |
| Unbounded provider waits | Caller-visible pending/retry state, bounded waiting and retained operation ownership until the OS callback finishes. | Delayed/missing/late callbacks cannot free context or release ownership early. |
| Replaced file while paused | Surface unresolved recovery; never silently resume a different file or retry forever without an action. | Replace/move/delete while paused, relaunch, retain edits and expose recovery or reselection. |
| Provider detection | Detect iCloud explicitly and qualify it; other providers need their own policy. | Dropbox/Google Drive do not accidentally inherit an iCloud guarantee from a generic ubiquity flag. |
| Error handling | Shared typed failures for conflict, unavailable, busy, permission, newer format and uncertain publication/control outcome. | Native UI offers the appropriate Retry, Save a Copy or reselection; no generic string parsing. |

Optional conflict merging through Loro is deferred. Equal UUIDs do not guarantee mergeable
history after shallow snapshots or close-time trimming. UUID plus version vector also
misses attachment metadata, artwork and other non-Loro stored changes; the spike uses
a whole-file fingerprint for conservative conflict detection.

## Core spike: implementation and measured result

Implemented locally in [`spikes/icloud-publication`](../spikes/icloud-publication/README.md).
`spikes/` is ignored by git, so the harness and reports stay local; this handoff is in
user-requested `plan/`. There are no production imports, generated contract edits,
format/ABI marker changes, or production Swift/app changes from the spike. The separate
CloudKit experiment has its own native target and generated bindings.

### Refinement result, 2026-10-18

This supersedes the original copy-only cost estimates below. The original prototype,
binary and reports remain in `spikes/icloud-publication/evidence-v1/` for comparison.
The refinement preserves the whole-file content fence, empty SQLite reservations,
inode lease transfer, pending-candidate state, directory sync and rescue behavior.
It does not adopt the review's proposed stat-only fence or remove ownership protections.

Three separately measured variants use the same real Rust Owner and Store:

| Variant | Snapshot and publication |
| --- | --- |
| A: `copy-rename` | Existing private SQLite backup, candidate byte copy, rename. |
| B: `clone-rename` | Clone the quiescent private store directly into the candidate, rename. |
| C: `clone-foundation` | Same clone, destination-volume replacement directory, Foundation safe-save. |

Clone creation runs inside the store's real backing mutex after flush, between SQLite
transactions. A concurrent accepted edit can update the owner, but its persistence
waits for this barrier; the cloned candidate contains the complete preceding generation.
The explicit barrier test verifies both the wait and the old/new contents. This is
stronger than assuming that a request happened to arrive between transactions.

The clone fast path removes `complete.slop`. EXDEV and unsupported-clone failures use
the established private Backup plus byte-copy path; space and permission failures remain
failures. No connection uses `journal_mode=OFF`. An exact-content snapshot intent is
durable before cloning, followed by candidate inode/hash bookkeeping. After a kill
between cloning and recording its inode, recovery removes the candidate only if its
bytes match that recorded intended snapshot. Unknown candidates are retained for rescue.

Foundation receives the coordinator's accessor URL, uses default replacement metadata
behavior and honors the returned URL. Local testing found a real metadata regression:
mode `0640` became `0600`. The corrected path restores mode and syncs through a descriptor
retained before the SQLite reservation. Opening and closing a fresh descriptor there
would risk dropping the process's POSIX SQLite locks. The failing metadata report is
retained as `foundation-metadata-before-fix.json`; mode, ACL and xattr preservation now pass.
This does not prove preservation of real iCloud sharing participants or version history.

Final evidence:

- `refined-correctness-copy-rename.json`: **43 passing cases**.
- `refined-correctness-clone-rename.json` and `refined-correctness-clone-foundation.json`:
  **50 passing cases each**. Each includes all eight frozen 1.0.0/1.0.1 documents, the
  original crash/conflict matrix, two clone kill windows, the persistence barrier and
  forced fallback/permission/space classifications.
- `refined-disk-clone-foundation.json`: **two passing real-filesystem cases**. A
  disposable APFS image exercises EXDEV fallback and actual ENOSPC after CoW allocation.
  The next private save fails; the public acknowledged copy remains intact. After kill,
  restoring space and recovery, the previous acknowledged private edit is preserved.
  The image is detached and removed; no test fills the boot volume.
- `bridge.m` compiles against the macOS 15.2 deployment target with availability warnings
  as errors. Execution was on macOS 26.6.2; this is still not a macOS 15.2 runtime test.

Local APFS measurements (three trials per variant/size; publish p50 / nearest-rank p95):

| File size | A: copy/rename ms | B: clone/rename ms | C: clone/Foundation ms |
| --- | ---: | ---: | ---: |
| 1 MiB | 103 / 217 | 83 / 101 | 84 / 145 |
| 25 MiB | 204 / 1,526 | 131 / 140 | 150 / 186 |
| 100 MiB | 612 / 849 | 326 / 349 | 541 / 652 |

At 100 MiB, median OS-accounted writes per publish fall from **210,444,288 bytes
(about 201 MiB) to 16,384 bytes** in both clone variants. Clone paths retain only the
private working store rather than another complete private backup. Logical file sizes
do not establish physical peak storage because APFS shares blocks and later writes
consume CoW space. Hashing, SQLite acceptance and syncing remain proportional to size;
this is not constant-time publication. Normal machine and compiler load, fixed variant
order and the small sample limit latency comparisons; the 25 MiB copy outlier is retained.
These counters measure neither physical NAND writes nor provider uploads.

Reproduction commands and raw/source identity are in the
[spike README](../spikes/icloud-publication/README.md). `summarize-refined.py` reproduces
`refined-summary.json` from all 18 publish samples per variant/size. Raw reports retain
their tested binary hash; benchmark builds precede the final test-only barrier helper.

**Decision supported by this refinement:** keep private publication as the candidate
for provider-managed documents, with clone plus proven fallback and Foundation safe-save
as the next integration path. Keep ordinary local files on master's direct path.
Do not promote this spike into production yet. Real iCloud staging, sharing metadata,
downloads, provider conflicts, scoped access and bounded waits remain release gates.
Start with explicit flush/CLI, close and background publication. Add coalesced idle
publication only after measuring a useful freshness requirement. Detect enabling
Desktop/Documents sync during an existing direct session at a write boundary; safely
convert under the owner or refuse further saves with a recoverable reopen action.

### What the prototype actually does

1. Coordinate the public file's initial read, take its existing registry lease,
   validate through core acceptance and take a whole-file SHA-256 base fingerprint.
2. Use SQLite Backup to create a private `working.slop`. Open the existing Rust Owner
   on that store. Its SQLite journals, attachments, saves and range reads stay private.
3. A normal save resolves only after the private owner flush succeeds. It does **not**
   claim that the public document or another device has the new state.
4. On explicit publish/flush, close/background, make and validate a private SQLite
   backup. Close it, sync it, then byte-copy it to a unique target-side candidate.
   The candidate's identity is recorded before copying, so a killed partial copy can
   be removed safely on recovery. It is not the document at its published name.
5. Copy safe-save metadata, then take a coordinated replacement claim. Under an
   empty SQLite reservation, verify the original identity and whole-file fingerprint.
   Acquire the candidate's registry lease and publish discovery before rename. A
   second empty reservation guards the new inode through durable bookkeeping.
6. Atomically rename, transfer the held lease, sync the destination directory, and
   persist the new base fingerprint/bookmark. These public SQLite reservations change
   no pages and create no journal. No public database connection is used for a save.
7. If killed, lock the private session and original registry identity before bookmark
   resolution. Accept only the recorded base or the exact pending published candidate.
   Recover private SQLite, preserve unpublished edits, and retry publication explicitly.
8. If the public file changed incompatibly, leave it intact and retain the private
   working copy. A harness rescue creates a new document with a fresh UUID and retained
   referenced attachments through the existing core clean-copy implementation.

`prototype.rs` contains the Rust path; `bridge.m` supplies coordination, opaque
bookmarks and safe-save metadata. It uses no pause/resume controls or OS 26-only APIs.
The bridge compiles with a macOS 15.2 deployment target and availability warnings
as errors. Symbol inspection of the baseline/prototype executable found no sync-control
symbol. Actual execution on macOS 15.2 remains untested.

The JSON recovery file is **metadata**, not document state: target/bookmarks, inode
identities, base hash and pending candidate. There is still one Loro owner and the same
SQLite format. No JSON mirror, new serializer, second engine or automatic merge exists.
The small discovery payload proves ownership visibility during replacement; it does
not implement the production owner socket or prove live CLI forwarding.

### Correctness evidence

The final [`correctness.json`](../spikes/icloud-publication/correctness.json) records
**43 passing cases**, including:

- SIGKILL at 13 phases: working-copy ready, private saved, backup copying/completed,
  partial/completed staging, metadata copied, base validated, candidate leased,
  renamed, lease transferred, directory synced, and private record committed.
- A spilling, uncommitted transaction killed with a hot **private** rollback journal.
  Recovery restores the last committed edit; the public document remains readable.
- An independent reader observed 67 complete, accepted snapshots across 10 publishes.
  The public document was always old or new, and no public `-journal`/`-wal`/`-shm`
  appeared during the tested publication operations.
- Separate-process lease/discovery probes at the compare and old/new inode handoff.
  Raw SQLite commits are refused while either destination inode is guarded. Recovery
  refuses a live session, including after a move or replacement.
- External Loro history, attachment metadata, artwork, same-path replacement, deletion
  and a replacement injected at the handoff. Conflicts keep the external bytes and
  permit private-copy rescue. A same-volume rename follows the same file identity.
- Real SQLite busy contention and real directory permission denial; injected ENOSPC
  during staging, pre-rename failure, and post-rename directory-sync uncertainty.
  Both relaunch recovery and same-process close retry retain edits and ownership.
- POSIX mode, an extended attribute and an ACL survive replacement. Symlink leaves
  and hard links retain the core's refusal behavior. Newer storage is refused without
  changing source bytes or creating a working database.
- All eight saved documents from the prepared frozen **1.0.0 and 1.0.1** corpora take
  their recorded edits through the private Owner, publish, close and reopen. App digest,
  document UUID, theme and retained attachments are preserved. Corpus assets are read
  without changes. This adds publication-path evidence to the existing full replay;
  it does not replace renderer, command, limit or native compatibility checks.

The tests exposed two details that a simple copy-on-close sketch missed: metadata
copying must happen before SQLite reservations because closing a second descriptor
can release the process's POSIX locks; and the replacement inode must be leased and
guarded before rename. Partial-staging cleanup and uncertain-outcome retry also need
explicit durable state. They are implemented in the spike, not hidden by a happy-path
benchmark.

`bun run verify` was run again: all exact production inputs already had passing evidence,
so the runner reused the existing green results. No new Swift/FFI changes were made in
the production targets. At the end of the refinement, `bun run verify --native` also
reused passing evidence for the exact production inputs. The separate CloudKit target
was built and signed directly with Xcode. Previous native/corpus verification remains
documented below.

### Local measurements

Apple M1, macOS 26.6.2, local APFS, release builds, three trials per size/variant.
Fixtures contain roughly 1, 25 and 100 MiB of referenced immutable attachments plus
small valid document state. There are 30 acknowledged saves per trial and, for the
prototype, a publication every five saves. Open/recovery timing is measured inside
the subprocess, excluding launch. These are warm local synthetic results, not provider
or phone benchmarks; fixed variant order and normal machine load limit precision.

Raw results: [`benchmark.json`](../spikes/icloud-publication/benchmark.json).
Aggregation: [`summary.json`](../spikes/icloud-publication/summary.json).
Build/source identity: [`build.json`](../spikes/icloud-publication/build.json).

| Approximate file size | Variant | Open median ms | Save p50 / p95 ms | 500 asset reads median ms | Close median ms |
| --- | --- | ---: | ---: | ---: | ---: |
| 1 MiB | master | 4.1 | 3.2 / 5.0 | 7.6 | 1.4 |
| 1 MiB | current pause branch, local file | 31.4 | 4.9 / 7.5 | 656.4 | 3.0 |
| 1 MiB | private publication | 41.6 | 3.1 / 5.3 | 10.5 | 76.4 |
| 25 MiB | master | 10.7 | 2.3 / 3.5 | 3.2 | 1.4 |
| 25 MiB | current pause branch, local file | 23.8 | 4.2 / 7.8 | 550.0 | 3.9 |
| 25 MiB | private publication | 141.5 | 2.6 / 5.0 | 3.7 | 224.3 |
| 100 MiB | master | 44.0 | 2.7 / 7.7 | 3.4 | 2.0 |
| 100 MiB | current pause branch, local file | 113.4 | 4.9 / 11.3 | 638.9 | 5.6 |
| 100 MiB | private publication | 666.3 | 2.8 / 12.1 | 4.2 | 941.3 |

| Prototype size | Publish p50 / p95 ms | Recovery median ms | OS-accounted bytes written per publish, median | Private retained files |
| --- | ---: | ---: | ---: | ---: |
| 1 MiB | 99 / 144 | 26.6 | 2.12 MiB | 2.12 MiB |
| 25 MiB | 217 / 233 | 56.9 | 50.26 MiB | 50.24 MiB |
| 100 MiB | 922 / 1,559 | 218.2 | 200.70 MiB | 200.66 MiB |

The write counter is Darwin's process `ri_diskio_byteswritten`, not physical NAND or
network bytes; attribution can lag an operation. The algorithm independently entails
at least one private full backup and one full candidate copy per publish. The private
store plus completed backup use about twice the document size; adding public and
in-flight candidate files raises peak storage toward **four copies**, before journal
and filesystem overhead. Normal private saves still write only roughly 48–56 KiB here.

A second workload reads alternating app/64 KiB attachment ranges on another thread
while 30 saves run; the prototype also publishes six times. Median trial p95 reads
were 0.04–0.14 ms on master, 4.48–5.22 ms on the current branch, and 0.06–0.08 ms on the
prototype. Workload durations/read counts differ because publishing takes time; this
is not a fixed-duration playback comparison. Worst observed reads still reached
roughly 201 ms on master and 96 ms on the prototype under contention/initial verification.
No native audio/video smoothness claim follows from these measurements.

### Recommendation and next work

**Continue toward private publication for cloud documents, but do not replace the
production backend from this spike alone.** The experiment supports the safety direction
and an older-API implementation. It is not a large deletion of all storage complexity:
provider pause/recovery is replaced by candidate/replacement/conflict recovery. Keep
ordinary local documents on their existing direct owner path unless a separate product
reason justifies their open latency and extra disk use.

The current branch's per-resource coordination is a measurable local regression even
without iCloud. Address the review gates above before merging it; do not ship that
regression merely because private publication may replace it later.

A small production integration experiment should do the following in order:

1. Separate the public document location from the owner's private store inside Rust.
   Route actual app and CLI requests through the same live owner. Transfer the real
   socket discovery across replacement without exposing cache paths as user documents.
   Keep window/Recents identity stable as each publish changes the inode. Preserve
   existing Duplicate/export, immutable masters, close retry and command behavior.
2. Define private durability versus publication in owner replies and native state.
   Explicit flush/close/export/CLI success must honor the promised publication barrier.
   Add typed pending/conflict/unknown-outcome errors and a live owner-mediated rescue
   action; the spike's JSON harness and closed-copy rescue are not that UI/API.
3. Add a dirty generation and coalesced close/background publication; avoid republishing
   unchanged snapshots or rebuilding the same backup on retry. Bound retained backups
   and clean candidates only by recorded identity or exact durable snapshot intent.
   Defer idle publication until freshness measurements justify intervals. Never trade
   away fingerprint, lease or directory-sync protection.
4. Replace spike coordination with the production Apple file-access boundary, carrying
   the accessor-provided URL, security-scoped grants, cancellation/pending state and
   provider-aware temporary replacement locations. Validate copyfile metadata policy
   with actual provider metadata. Do not infer provider staging behavior from local
   hidden sibling files.
5. Test real iCloud Drive and synced Desktop/Documents on macOS 15.2 and current macOS:
   cloud-only explicit opens, offline opens/publication, interrupted downloads, access
   revocation, moves, eviction, account changes and two-device conflict copies. Confirm
   no sibling journal enters sync and that recovery never overwrites a remote change.
6. Prove UI/CLI integration, frozen compatibility, Quick Look/Recents placeholders,
   attachment playback, native export and failed-close recovery. Then remove the old
   pause implementation and update public availability claims in the same adoption
   change. Keep that decision separately reviewable.

Still unproven: physical power loss, real provider staging/atomicity, cross-volume
moves (cross-volume publication fallback now passes), File Provider identity changes, old-OS
execution, access-scoped sandbox recovery, production socket forwarding and iOS
backgrounding. `NSFileCoordinator` waits are synchronous and unbounded in this proof;
process-kill safety does not establish a responsive native waiting/retry lifecycle.
Uncoordinated filesystem operations can bypass Foundation coordination; the tests fence
raw SQLite commits and check a deliberate replacement at a barrier, not an arbitrary
malicious filesystem writer at every instruction.

## Separate CloudKit transport proof, 2026-10-18

Implemented in [`spikes/cloudkit-sync`](../spikes/cloudkit-sync/README.md), separate
from file publication and production. The signed Mac app uses a dedicated entitled
development container, `iCloud.com.hitslop.syncspike`, a new private test zone per run,
and two independent local Rust replicas. It uploads synthetic fixtures only. It does
not copy `.slop` databases through CloudKit or put these fixtures in iCloud Drive.

The language boundary is useful, with two qualifications to the review:

- **Rust owns document and delivery semantics.** The real Owner/Store performs edits,
  remote Loro application, attachment acceptance and persistence. Rust owns a durable
  ciphertext outbox/inbox, stable record IDs, acknowledgements, replay and account
  isolation. Swift transports sealed CKAssets and persists CKSyncEngine's opaque state.
  Encryption happens in Rust with XChaCha20-Poly1305; authenticated context binds the
  document UUID, app digest and record ID. The fixture key stays in Rust-private local
  storage; production key creation/distribution/recovery is deliberately not solved.
- **Swift is a practical adapter choice, not an API requirement.** Real generated Rust
  bindings for [CKSyncEngine exist in objc2-cloud-kit](https://docs.rs/objc2-cloud-kit/latest/objc2_cloud_kit/struct.CKSyncEngine.html).
  A particular crate's simulated engine does not establish that Rust cannot call it.
  The native adapter still suits signing, entitlements and Apple's lifecycle APIs.
  CKSyncEngine requires macOS 14 / iOS 17, rather than the pause API's OS 26 boundary.
- **CloudKit is a transport with its own durable protocol.** It is not a WebSocket
  replacement inside the existing relay. This proof reuses the owner's receive path
  but separately manages immutable records, receipts, inbox and opaque engine cursors.
  The existing Durable Object relay is unchanged. Production needs an explicit Rust
  transport boundary and bounded delta/checkpoint packaging before supporting both.

The proof persists the inbox before remote application, flushes the real owner before
marking it applied, and persists engine checkpoints only while local acceptance/storage
succeeds. Startup rescans retained history after a document-commit/outbox crash gap.
Unacknowledged retries reuse exact ciphertext. If a server receipt was lost, a duplicate
create produces `serverRecordChanged`; Rust verifies the existing server asset byte for
byte before acknowledging. The adapter accepts the aggregate send error only if every
failed record has been reconciled this way. Wrong keys, tampering or collisions fail.
Callbacks from retired engine instances cannot overwrite the active engine checkpoint.

Local evidence is green: **four Rust integration tests and nine subprocess fault
cases**, including duplicate/reordered delivery, text/theme/attachment convergence,
closed-owner edits, account pause, missing ciphertext asset reconstruction and kills
around document/outbox/inbox/receipt/checkpoint durability. These local record fixtures
are explicitly not evidence of CloudKit access. The signed app's separate
[`live-report.json`](../spikes/cloudkit-sync/live-report.json) records the actual cloud
run, including its automatic-delivery result. Earlier failed runs remain alongside it.

**Real-cloud result:** the signed development run passes concurrent edits, the encrypted
attachment, engine-state restoration, the closed-owner edit and a lost-receipt retry.
One automatic sender uploads successfully; the established manual receiver fetches and
converges. Two-device automatic delivery remains unqualified. Replacing both test
engines with automatic instances missed a new server record even after manual retries
(11 on the server, 10 in each inbox). Keeping the original manual receiver converges;
the cause of the two-automatic-engine behavior is unresolved. The failed experiment is
retained in `two-automatic-engines-failure.json` and reproducible with
`CLOUD_PROBE_TWO_AUTOMATIC=1`. This is a narrowed test topology, not a production fix.

An important production constraint emerged: use **one CKSyncEngine per app/account/
database**, routing documents in Rust. Multiple engines for the same database in one
process are a development test technique; Apple's configuration contract warns against
that production topology. Automatic scheduling depends on the system and silent push;
a manual convergence result cannot establish push-triggered or background delivery.
See [Apple's configuration contract](https://developer.apple.com/documentation/cloudkit/cksyncengine-5sie5/configuration),
[automatic scheduling](https://developer.apple.com/documentation/cloudkit/cksyncengineconfiguration/automaticallysync),
and [two-replica sample tests](https://github.com/apple/sample-cloudkit-sync-engine/blob/main/Tests/SyncTests.swift).

Remaining CloudKit work: incremental bounded chunks (the proof uses full retained
history and a 16 MiB payload cap), document/bootstrap catalog, key distribution and
recovery, compaction/pruning, deletion policy, quotas, account UX, production schema,
network interruption and two-device/background qualification. No real account sign-out
or iOS lifecycle was exercised. The proof includes a 1 MiB encrypted attachment.

For the CLI, retain the existing live-owner forwarding rule. An app-owned CloudKit
adapter syncs live owner edits. Closed-owner CLI edits are locally durable and reach
CloudKit on the next app open; an unsigned CLI need not acquire CloudKit entitlements.
The proof exercises that reopen/export behavior through the real Rust owner, but does
not implement production CLI/socket integration with this adapter.

For iPhone, choose replication mode explicitly. iCloud Drive publication provides
sequential file handoff; CloudKit provides live record replication once an iOS host and
its durable delivery integration exist. Do not enable both paths as uncontrolled
writers to the same document. File conflict merges through Loro remain a separate
feature requiring retained-history and non-Loro metadata policy. An iOS editor, Files
permissions, scoped recovery, scene/background lifecycle and capture/command execution
remain the larger product work described below.

## Original pause implementation: intent and behavior

The original question was whether the docs should explain synced Desktop/Documents
more clearly, or whether hitSlop should support iCloud. The chosen implementation is:

- Support iCloud Drive, including synced Desktop/Documents, on macOS 26+ using Apple's
  newer file-provider sync controls.
- Retain local-file support on macOS 15.2+. On older supported macOS versions, identify
  iCloud locations and explain that they require macOS 26 or a local destination.
- Save locally while editing. Pause provider uploads/downloads while the document is
  owned, then resume on successful close. A local-save acknowledgement does not mean
  the upload finished.
- Support sequential use across devices. Close and wait for iCloud before switching
  devices. Simultaneous edits are not merged by this feature.
- Preserve one Rust document owner, the existing SQLite/Loro storage, writer locking,
  acceptance rules, and released-file compatibility.

Apple documents these controls for both macOS 26 and iOS 26, including flat files:
[FileManager sync control](https://developer.apple.com/documentation/foundation/filemanager).
The SDK header also states that a pause survives app termination and must be explicitly
resumed. That is why durable recovery bookkeeping is required.

## What was implemented

### Shared file access and protection against external changes

A small Objective-C Foundation adapter is linked into the Rust core for macOS. Rust
continues to own SQLite, Loro, acceptance, and every durable write. The adapter supplies
`NSFileCoordinator`, `NSFilePresenter`, file metadata, bookmarks, and provider controls.
Both the native app and CLI use this boundary.

- Coordinated reads materialize cloud-only files before normal file acceptance.
- Store transactions, attachment/artwork writes, copies, template publication, and
  resource reads now enter the shared access layer.
- A writer's presenter observes changes. Same-inode size/mtime changes and SQLite's
  connection-local `data_version` prevent a stale owner from saving over external edits.
- Existing device/inode and hard-link checks remain. Reconnection after an original
  file is moved away and restored additionally checks document identity and saved state.
- Rust catches panics before returning through Objective-C. Nested claims for the same
  file reuse the outer claim; a nested read cannot silently upgrade to a write.

**Important review scope:** coordination and the external-change checks currently
apply to ordinary local files too. This is a change to shared storage behavior, with
potential effects on latency, lock ordering, and save refusals outside iCloud.

### Pause, close, and crash recovery

The normal writer lifecycle is:

1. Coordinate initial file access/materialization and acquire the existing writer lease.
2. Detect whether the file is in an iCloud location.
3. Atomically write and sync a registry intent before requesting a provider pause.
4. Complete the pause before opening the SQLite writer.
5. Edit and save through the existing SQLite store while provider sync stays paused.
6. Close SQLite, resolve the bookmark, and resume with `preserveLocalChanges` while
   retaining the writer lease through the provider callback.
7. Remove the intent after a successful resume and release ownership.

The registry intent is a small `.icloud` file beside the device/inode writer lock in
`~/.hitslop/live`. It contains file identity and an opaque Foundation bookmark, not a
copy of document state. The current implementation uses no private working database or
snapshot-publication backend; an earlier explored approach was removed.

A failed resume retains ownership and permits native Retry. Subsequent editing can
re-pause and reconnect to the accepted saved file. Startup and a background worker
retry abandoned intents every 30 seconds while the process runs. Recovery takes the
original identity's lock before resolving the bookmark, skips live owners, pauses
idempotently, recovers/validates SQLite and saved Loro state, closes the connection, and
resumes. Refused or failed recovery retains the intent. Registry lock files are never
removed. A different file identity is refused even if a bookmark resolves to that path.

Desktop/Documents ubiquity is checked again before owned store access, allowing a pause
before a subsequent transaction if syncing was enabled after opening. Transition races
still need real-provider qualification.

### Native integration, documentation, and tests

- Replaced Swift's blanket local-only guard with the shared core's location policy and
  materialization path. Added the internal UniFFI `existing_file_path` function.
- Moved application-level URL resolution off the main actor because downloads can wait.
  Preserved Foundation URL normalization so Recents, creation, and open-window identity
  agree on paths such as `/var` and `/private/var`.
- Updated landing docs, availability, CLI/sharing guidance, architecture, runtime,
  roadmap, testing guidance, and authoring storage guidance.
- Added coordination, stale-write, recovery-intent, offline-resume, live-lease,
  rename/bookmark, newer-file refusal, and killed-writer tests at shared boundaries.
- Updated fixtures that intentionally corrupt attachments to mutate closed files and
  reopen them, so they exercise attachment validation rather than the new live-owner
  external-change refusal. The speech fixture restores its deliberately damaged row
  after close-time reclamation.

Primary implementation map:

| Boundary | Entry points |
| --- | --- |
| Foundation adapter and coordination | `crates/hitslop-core/src/file/access.m`, `access.rs` |
| Provider controls and recovery intents | `crates/hitslop-core/src/file/sync.rs` |
| Writer lifecycle and stale-write protection | `crates/hitslop-core/src/store/mod.rs`, `src/registry.rs` |
| Resource/copy/publication coverage | `crates/hitslop-core/src/file/`, `src/store/` |
| Native URL/opening integration | `SlopFile.swift`, `SlopCreation.swift`, `SlopApplicationCoordinator.swift` |

## Verification and present limits

Completed during this work:

- `bun run verify`: passed the affected ordinary tiers, including 411 Rust tests,
  SDK/shell, CLI, rendering, contracts, landing checks, and packed-package checks.
- `bun run verify --native`: passed after fixing URL normalization and adapting the
  speech damage fixture. The final native tier passed 32 tests, with one release-only
  host-death case skipped. Swift's ordinary suites passed; nightly-only cases remain
  outside this run.
- Frozen compatibility replay for releases 1.0.0 and 1.0.1 passed.
- The stale external-write regression was reproduced against the starting master and
  passed with the fix.
- A subprocess killed with a hot rollback journal recovered before the simulated
  provider resumed. A native bookmark followed a same-volume rename.
- The new sync-controls resource symbol was inspected as a weak reference in the engine.
  This supports older-OS loading; it is not a substitute for testing macOS 15.2.

The latest local report is `.hitslop/evidence/verify.json`, final run
`2026-10-18T02-38-33-664Z-34891`. Reports/build artifacts are local and may be replaced.

**Not yet demonstrated:** real iCloud pause/resume, real cloud-only downloads, synced
Desktop/Documents, offline/reconnect behavior, two-Mac transfer/conflicts, or running
on macOS 15.2. Automated provider-control tests use a stand-in. No tests enabled iCloud
or wrote into the developer's synced folders.

## macOS hardening before release

### P0: bound waits without releasing ownership unsafely

The adapter currently blocks a storage/recovery worker on
`dispatch_semaphore_wait(..., DISPATCH_TIME_FOREVER)`. A stalled provider callback can
hold open/close or recovery indefinitely.

Replace this with an asynchronous provider-operation lifecycle, with a bounded wait
for the caller and visible pending/retry state. The operation must retain its writer
lease and callback context until the OS request really finishes. A UI timeout must
not release that lease: a late resume could otherwise unpause a subsequent owner's
transaction. Serialize retries, fence old completions by operation/session identity,
and retain the durable intent until the provider outcome is known. Do not solve this
by merely changing the semaphore timeout and returning.

Acceptance: delayed, missing, duplicate-test, and late completions cannot deadlock
the UI, create concurrent control requests, free callback memory early, or allow a
second writer to race a pending resume. Test these at the provider boundary.

### P0: qualify actual SQLite and provider behavior

Use disposable documents in a dedicated test iCloud folder and record the OS, device,
network condition, operation, and observed result. Include:

- Main-file pause plus SQLite's sibling `-journal`: prove the sidecar cannot be
  independently evicted/replaced in a way that defeats local crash recovery, and that
  a remote device never mistakes a transferred sidecar for its own recovery journal.
  The current local crash test does not establish this. Do not assume pausing one flat
  item also protects its siblings or that iCloud automatically excludes them.
- Kill before pause completion, during a spilling SQLite transaction, after commit,
  during resume, and after resume but before intent deletion. Relaunch on that device,
  recover, finish syncing, and verify the file on another Mac.
- Offline open/edit/close, provider busy, cloud-only/unavailable download, disk full,
  account sign-out/change, revoked access, and files moved/replaced/deleted while open.
- Enable Desktop/Documents sync during an open session, including during a write.
- Conflicting offline edits on two Macs. Verify data preservation and document the
  provider's conflict behavior; `preserveLocalChanges` is not an automatic merge policy.
- macOS 15.2 local workflows and clear refusals for iCloud Drive and synced folders.

Treat unexplained corruption, unrecoverable journal behavior, or stranded pauses as
release blockers. If flat-file controls cannot meet the safety contract, revisit the
storage approach explicitly rather than silently introducing a second backend.

### P1: reduce regression exposure on local files

Audit which coordination/presenter work is necessary for local sessions. Prefer a
local fast path where it preserves safety, while retaining detection of a location
becoming provider-managed. Document the transition behavior before changing it.
Keep external-write protection as a separately reviewable change so its refusal rules
and reconnect checks can be assessed independently from provider lifecycle work.

Measure ordinary open, save, resource reads, close, copy/export, and many-window use
against the starting master. Exercise Finder renames, move-away/restore, existing
SQLite busy/full retry, and legitimate metadata changes that must not stop saving.
Audit all lock ordering: provider calls outside coordinated claims and backing locks;
resource access and close cannot introduce an inverse acquisition order.

### P1: make failures understandable and recoverable

Classify busy/transient, unavailable/offline, unsupported controls, permission loss,
replacement, and newer-format refusal instead of collapsing every provider failure
into a generic string. Rust owns any shared types; native UI consumes typed state.

Show local save state separately from provider state. Useful states include preparing
download, sync paused while editing, resuming, and waiting for access/network. Only
claim an upload completed when the provider supplies evidence. Offer native Retry and
an owner-mediated Save a Copy path for accepted edits blocked by an external change.

Recovery failures currently retry silently on a 30-second loop. Add bounded backoff
and foreground/network-driven retry opportunities, plus a way to surface unresolved
documents. Avoid repeated prompts and excessive scanning. Telemetry may record error
classes, phase, and duration; exclude document paths, bookmarks, contents, and account
identifiers. Refresh stale bookmarks only after validating the same file identity.

After qualification, align public availability claims with the evidence. Current
branch docs describe the intended supported behavior, not proof that qualification
has been completed.

## Taking the feature fully to iOS

### Choose storage before choosing the iOS baseline

The original pause implementation needs iOS/iPadOS 26+. Private working-copy publication
uses older Foundation APIs and removes that particular OS 26 requirement; it does not
by itself establish a supported iOS minimum. Make the storage decision from this spike,
then choose and qualify the iOS deployment target. The macOS minimum remains 15.2.

Start with Files-based open-in-place and sequential Mac/iPhone/iPad use. Keep Loro,
SQLite acceptance, ownership, saves, attachments, and recovery in the shared Rust core.
Swift owns document access permissions, scene lifecycle, UIKit presentation, and OS
callbacks. A UIKit document wrapper, if used, must forward to the owner rather than
become another serializer or writer. Live collaboration and automatic merging of
simultaneous file edits remain separate work in the existing relay roadmap.

### 1. Produce an actual iOS core build

The current code cannot become an iOS implementation just by widening one `cfg`:

- Build the Rust/UniFFI library for iPhone devices and simulators, package the iOS
  slices alongside the existing macOS XCFramework slice, and use the correct SDK,
  deployment target, Foundation linkage, and platform SQLite.
- Generalize the Apple coordination/control boundary from macOS-only to supported
  Apple targets, with explicit availability checks for each OS.
- Audit all macOS/Linux assumptions. One concrete example is exclusive file
  publication, whose rename branches currently cover only macOS and Linux. Prove
  equivalent no-overwrite, durability, and uncertain-publication behavior on iOS.
- Separate local storage/owner requirements from desktop CLI, process execution,
  socket discovery, catalog-path discovery, and desktop sandbox code in the Cargo
  graph. The storage feature currently pulls in the runner; the runner's desktop
  process/sandbox path needs an explicit iOS solution.
- Configure registry/recovery storage in app-private Application Support before the
  first owner opens. Do not use the password-database home plus `~/.hitslop/live` on
  iOS. Use an App Group only if a later extension actually shares ownership/storage.

Exit criterion: a device and simulator harness opens, edits, flushes, closes, and
reopens a local `.slop` through the same Rust owner, without launching authored code
in a desktop child process or adding a second SQLite library.

### 2. Build the native document and Files integration

The current Swift package declares macOS only, and `DocumentSession` and several core
helpers import AppKit. Extract reusable Foundation/WebKit pieces and add UIKit-specific
hosting without weakening the macOS app's target graph.

- Register the `.slop` document type and implement Files open-in-place, create/import,
  Duplicate/Save a Copy, share/export, and Recents restoration. Preserve the core's
  identity rules for duplicates and immutable masters.
- Have the Rust file-access layer own the lifetime of security-scoped grants through
  its Apple bridge; Swift passes the selected URL/bookmark rather than maintaining a
  separate storage policy. Recovery must restore access before touching a bookmarked
  external file and release it only after outstanding work ends. Handle
  denied/revoked access by requesting user reselection without overwriting anything.
- Start with the system document browser/picker. An app-owned iCloud container and
  its entitlements/discovery are an additional product choice, not a prerequisite for
  every Files-selected document. If added, use supported cloud metadata discovery
  rather than assuming all document contents already exist locally.
- Handle cloud-only downloads, unavailable files, device lock/data protection, account
  changes, provider moves, and conflict copies in native UI. Bookmarks and inode-based
  leases are device-local and must never be treated as cross-device identifiers.

Apple's references:
[document browser](https://developer.apple.com/documentation/uikit/building-an-app-with-a-document-browser),
[security-scoped access and bookmarks](https://developer.apple.com/documentation/uikit/providing-access-to-directories),
and [iCloud document discovery](https://developer.apple.com/documentation/uikit/synchronizing-documents-in-the-icloud-environment).

### 3. Design backgrounding as part of ownership

The Mac policy of pausing until the user closes the window is insufficient for an
iPhone that may be suspended while its document is still visually open.

If the pause implementation is retained, its first lifecycle would be:

1. While foreground editing, hold the writer lease and provider pause.
2. On a genuine scene-background transition, stop accepting new edits, drain page
   drafts and in-flight imports through the existing barrier, flush, close SQLite,
   and resume provider sync. Coordinate across scenes; one disappearing scene must
   not close a document another active scene still uses.
3. Request finite background execution time early enough to finish critical work and
   implement its expiration handler. Never depend on a periodic worker continuing
   after suspension, and never perform an unsafe resume to meet a deadline.
4. On expiration/failure, preserve durable recovery information and only acknowledge
   completed saves. Accepted but unflushed edits need a bounded owner-owned durable
   recovery strategy; current in-memory retention alone cannot survive iOS termination.
   Define that strategy before claiming background safety. Do not create a Swift/JSON
   mirror or write a failed pending save into a potentially replaced document.
5. On foreground return, restore permission, finish pending control/recovery work,
   acquire ownership, materialize and accept the latest file, pause, then remount or
   refresh the page from the current owner. Do not reuse stale live state over a
   remotely updated file. Define undo continuity when the saved document changed.

With private publication, the foreground owner instead keeps the working database
in protected app-private Application Support. Idle and background barriers publish a
closed snapshot; expiration retains the durable private session and the last complete
public file. Reentry compares the public file with the recorded base before retrying.
No provider pause needs to survive suspension. Multi-scene ownership, finite background
time, access revocation, data protection and an honest pending-publication UI still
need implementation. A locally durable working copy cannot transfer edits to another
device while its app is terminated; it preserves them for recovery on this device.

Test app switching, device lock, memory termination, force quit, rapid foreground
return while resume is pending, background-time expiration, and two iPad scenes.
Successful local persistence and eventual server transfer must remain separate states.
See Apple's [background execution guidance](https://developer.apple.com/documentation/uikit/extending-your-app-s-background-execution-time).

### 4. Port the rest of the document experience

For a complete iOS host, inventory every native operation exposed by the existing ABI:
editing, undo/redo, attachment import/media access, clipboard, export/share, captures,
camera/microphone/speech, errors, and window-derived sizing behavior. Adapt controls to
touch, keyboard/safe-area changes, rotation, iPad multitasking, and accessibility.

Rendering and authored commands are a major separate workstream. Today's native
capture and named-command paths use restricted child processes. Prove an iOS-supported
execution/isolation approach with resource and cancellation limits before promising
PNG/PDF export, template artwork, widgets, or authored commands. Retain the same
runtime ABI, stored capture contracts, Rust acceptance, and intent-only command
boundary. Do not substitute a JavaScriptCore document engine or execute commands with
unrestricted native access. Any genuinely unsupported platform capability needs an
explicit refusal and an accurate availability matrix.

Port Analytics/Crashlytics integration as appropriate, use iOS distribution/update
mechanisms, and keep Sparkle and other desktop-only dependencies out of iOS targets.
Widgets/extensions can follow the core document host; they must consume host-produced
artifacts or use the same ownership rules if granted document access.

### 5. Establish an iOS acceptance gate

- Run shared Rust/SDK tests, macOS native regression checks, and frozen compatibility
  replay throughout the extraction. Add device/simulator tests at the shared host
  boundary, not per-slop test suites.
- Replay released files on iOS: open, render, edit, save, reopen, and import/read
  attachments. Verify newer markers refuse without writes. Prove iOS-written files
  reopen in the corresponding supported Mac build without a format/ABI bump for a
  platform refactor.
- Use real devices on one iCloud account for Mac-to-iPhone, iPhone-to-Mac,
  iPhone-to-iPad, and repeated offline/reconnect cycles. Verify content, identity,
  attachments, captures, and absence of stranded pauses after background/termination.
- Test file-provider capability refusal and cross-device conflicts; initially qualify
  iCloud specifically. Other providers need their own evidence.
- Add simulator/device build and test tiers to the existing verification runner and
  an iOS release gate. Simulator-only success does not qualify real iCloud transfer,
  protected-data behavior, or background termination.

## Suggested delivery order and decisions to revisit

1. Complete the core publication spike and choose storage. Address the current
   branch regressions before merging either model; qualify recovery and the actual
   provider before advertising support.
2. Extract the Apple/shared core boundaries and prove the local iOS device harness.
3. Add Files access, native editor hosting, and background-safe ownership/recovery.
4. Qualify cross-device iCloud handoff on hardware.
5. Complete capture/command execution and the native capability matrix, then qualify
   the full iOS product and release workflow.

Before implementing the iOS host, lock these remaining product/engineering choices:
iOS/iPadOS minimum; system-browser-first versus an app-owned iCloud container;
background unsaved-edit recovery and undo continuity; the supported evaluator
isolation model; conflict-copy recovery UX; and which native capabilities must ship
in the first iOS release. The recommendations above are defaults for planning, not
claims that these future choices have already been approved or implemented.
