# Sharing authority, checkpoints, and recovery

This specifies the implemented sharing and private-library lifecycle. It complements
[architecture](../architecture.md) and the [engineering contract](../engineering-contract.md).
The [relay README](../../apps/sharing/README.md) owns operator setup and qualification commands.
Implementation and local qualification do not establish that a release, deployed relay,
CloudKit account, or physical-device campaign has been qualified.

## Owners and authority

`pop-core` accepts app definitions and document history; `pop-storage` owns the SQLite
file and durable copies; `pop-runtime` serializes mutations and publications. Native and
browser sharing transports use that owner. The relay validates hashes, opaque frontiers,
resource inventories and admission limits. It never interprets SQLite or Loro.

A managed document has one current byte authority. Private documents use a CloudKit
snapshot. A live shared document uses the relay; CloudKit retains bounded catalog and
transition metadata for the owner's devices, not a parallel stream of shared document
snapshots. Library catalog authority and a file's relay membership are separate records.
A guest's local file is not an owner CloudKit catalog replica.

The CloudKit record type remains `SlopDocument` in the current product's library zone;
that name is not permission to import historical product zones. Its payload is a SQLite
asset, not JSON document state. Catalog format and document layout are independent markers.

## Catalog formats and ordering

Catalog format 1 accepts the existing private-snapshot behavior without an authority
object or revision stamp. Format 2 stores `authority`, `catalogRevision`, and the existing
`title`, `template`, `snapshotVersion`, `assetRevision`, `artworkVersion`, `deleted` and
optional `payload` fields. The authority object is Rust-owned, tagged by `phase`:

| Phase | Additional fields | Meaning |
| --- | --- | --- |
| `snapshot` | optional `receipt` | CloudKit owns the private snapshot; a receipt records an unfinished finalization. |
| `checkpoint` | `proof: {transition, sourceVersion, version}` | Latest private checkpoint and its source provenance. |
| `preparing` | `transition`, `room`, optional `seedHash`, optional `priorCheckpoint` | Relay reservation exists; retained private payload must remain available until activation or cancellation. |
| `relay` | `transition`, `room`, `seedHash` | The relay owns shared bytes; catalog discovery needs no payload download. |
| `handoff` | `transition`, `room`, `freeze` | Creator is transferring a frozen relay frontier to private storage. |

A final snapshot receipt contains `transition`, `room`, `freeze`, and `snapshotVersion`.
`freeze` is a typed `{token, seedHash, root, heads}` object in catalog JSON. Swift carries
it through the FFI as opaque JSON; shared Rust decodes and validates it. No credential is
stored in these objects.

Each CloudKit compare-and-swap uses `.ifServerRecordUnchanged` and increments the
revision associated with that exact fetched or restored record. System-field archives
retain its revision alongside its change tag. The signed 64-bit positive revision and
CloudKit change tag form `CatalogStamp`; the local library stores one latest validated
stamp per document. A newly recreated tombstone uses the local revision as a floor.

All receive, acknowledgement, tombstone and metadata paths check the stamp under the
library lock before mutation. Older revisions, equal revisions with a different tag,
and unstamped legacy records after a stamped record are refused. An equal revision and
tag can finish an interrupted installation. A higher revision can cross any number of
missed sharing cycles; acceptance does not require retaining a chain of old rooms.
Outgoing local transitions still obey the strict phase rules.

For a payload installation, Rust validates document UUID, app compatibility, payload
version and any checkpoint/freeze proof before persisting the incoming authority fence.
The fence precedes canonical mutation, so an older callback cannot regain authority
if a later installation step fails. It is an admitted-record fence, not a claim that
installation completed. Same-tag retry completes the work. Invalid incoming files do
not advance it. Superseded callbacks do not rewrite catalog errors or enqueue retries.

Zone-change fetches request `authority` and `catalogRevision` with metadata and exclude
`payload`. The full-fetch unchanged shortcut requires matching stamp, committed authority,
snapshot version and asset revision. A new Relay or Handoff record cannot be skipped just
because its private payload version is unchanged. Unsupported catalog formats require an
update; no fallback decodes them as snapshots.

Source: [catalog types/transitions](../../crates/pop-library/src/catalog.rs),
[stamp fence](../../crates/pop-library/src/stamp.rs),
[receive/install](../../crates/pop-library/src/lib.rs),
[CloudKit adapter](../../apps/apple/Packages/PopApple/Sources/PopCore/LibrarySync.swift).

## Starting and canceling a room

1. A verified creator reserves the document through `POST /v1/rooms`. Account lookup
   makes retries recover the existing Uploading or Live room.
2. A managed document conditionally publishes Preparing with its retained private
   snapshot before opening the relay. A same-room Preparing or Relay record found on
   another device supplies the authoritative transition nonce; a different room is refused.
3. Explicit Preparing resume reconciles the retained payload through the existing owner
   before preparing the relay seed. Without a prior checkpoint proof this is merge-only;
   a proven cutoff uses recovery-before-adoption. Missing/deleted Preparing payloads and
   mismatched proof versions are refused. Background Preparing discovery remains
   metadata-only, and Relay discovery never merges CloudKit bytes.
4. The creator uploads the seed, app manifest/assets and attachments and calls `open`.
   The relay validates its stored resources before entering Live. The client records the
   committed seed/epoch and conditionally adopts the accepted canonical file. Local edits
   that raced preparation are retained through recovery. CloudKit then confirms Relay.

Initial Uploading seed replacement remains supported. The accepted seed is bound to the
same document UUID and app digest; owner-independent native resolution validates it before
attaching an owner. Sharing does not replace the app inside an existing document.

Cancel uses creator-authenticated `DELETE /rooms/{room}?phase=uploading` first. The room
checks Uploading and marks Ended before yielding; Live or frozen returns 409. Only after
that succeeds does CloudKit restore the retained snapshot or prior checkpoint under CAS.
If restoration fails, the ended reservation and Preparing record remain retryable. A
concurrent activation cannot reopen the canceled room.

## Private checkpoints

An eligible private upload may stage a sanitized accepted-state checkpoint rather than
carry obsolete history and hidden attachments indefinitely. The candidate is derived
from the owner, preserves document UUID and app, and leaves the live owner and undo intact
while awaiting CloudKit CAS. Candidate source/version/receipt and file survive restart.
An unissued reservation may be refreshed after a source edit or failed oversized attempt;
an already issued candidate keeps its binding until reconciliation.

A successful current-record receipt drives conditional installation through the owner.
CAS failure alone never installs the candidate. The accepted latest file keeps visible
attachment bytes; recovery protects local work that is not covered by the receipt.
Ordinary writes preserve checkpoint provenance. Preparing cancellation restores the prior
proof. Completed handoff receipts normalize to the bounded latest checkpoint proof, not
an ever-growing list of historical rooms.

A superseded private candidate does not block joining an authoritative relay. Its payload
is retained until accepted canonical installation/recovery confirms it can be retired.
These are private latest-state copies, not a document history interface or device-ack registry.

Source: [checkpoint staging](../../crates/pop-library/src/checkpoint.rs),
[recovery catalog](../../crates/pop-library/src/recovery.rs).

## Relay epoch publication

The following room routes are relative to `/v1/rooms/{room}`.

| Route | Request | Success |
| --- | --- | --- |
| `GET /epoch` | Capability while Live; creator identity while frozen | `200 EpochState` |
| `PUT /epochs/{sha256}` | Raw seed bytes matching the path hash | `200 {seedHash, size, expiresAt}` |
| `PUT /epochs/{sha256}/blobs/{hash}` | Raw attachment bytes under the seed lease | `200` |
| `POST /epoch` | `PublishEpoch` JSON | `200 EpochState` |
| `DELETE /epochs/{sha256}` | Candidate release | `200`; current seed `409` |
| `POST /handoff/begin` | Creator identity, `{token, expected: {seedHash, heads}}` | `200 {token, seedHash, root, heads}` |
| `POST /handoff/finalize` | Creator identity, `{token}` | `200`; already Ended `410` |

`GET /epoch` returns `{seedHash, root, heads, handoff?}`. A wire version is an
array of unique ascending `[peer, counter]` pairs: peer is 16 lowercase hex digits and
counter is in `0..=2147483647`. `heads` must cover `root`. The root is the original seed
frontier for the current epoch; ordinary `/compact` does not advance it.

A capability writer or creator can publish a fresh epoch:

1. Read the expected seed hash and heads. The owner atomically checks that its accepted
   history covers those heads before preparing the candidate. The candidate can include
   unsent accepted local edits; the relay CAS prevents omission of a newly accepted remote push.
2. `PUT /epochs/{sha256}` stages immutable SQLite bytes. Upload referenced attachments to
   `PUT /epochs/{sha256}/blobs/{hash}`. Staging does not change bootstrap or visible inventory.
3. `POST /epoch` supplies `{expected: {seedHash, heads}, seedHash, root, resources, attachments}`.
   Exact expected hash/heads and resource checks precede one transactional change of the seed
   pointer, root, heads, base/record set and inventories. A race returns 409. Successful
   publication can be retried after a lost response using the committed candidate identity.
4. Only after the server receipt does the owner conditionally install the accepted candidate.
   Cleanup issues `DELETE /epochs/{sha256}` for uncommitted leases, including uncertain responses:
   absent/expired is idempotent 200; the current seed is protected by 409.

At most two unpublished seeds exist per room, each with a ten-minute lease. Each seed is
at most 16 MiB; each stage has a separate attachment budget of at most 16 MiB. Current
attachments and another live stage protect shared immutable blobs from reclamation. Stage
registration precedes R2 writes; expiry, explicit release, failed uploads and obsolete
objects have bounded, retryable cleanup. The SQLite pointer/inventory switch is atomic;
R2 deletion is subsequent cleanup, not a distributed transaction.

Publication fences old sockets and in-flight uploads. Socket error 6 (`ERROR_EPOCH`)
requests checkpoint recovery; a new Hello must cover the current root. A raced
`GET /seed?hash={expected}` returns 409. Neither the relay nor JavaScript interprets Loro
frontiers to manufacture a merge. Rust validates seed hash, UUID, app and accepted root.

Source: [wire types](../../crates/pop-sharing-protocol/src/lifecycle.rs),
[relay epochs](../../apps/sharing/src/epochs.rs), [resources](../../apps/sharing/src/resources.rs).

## Refresh and local recovery

Native and browser connections retain an epoch-bound owner confirmation of version and
heads. Missing or uncertain confirmation is conservative: before replacing an incompatible
checkpoint, preserve unconfirmed local work as an independent accepted local document with
a new UUID and no relay membership. Recovery retains visible values and referenced blobs.
Canonical adoption preserves the shared document UUID and emits one full publication with
a monotonic sequence; its old undo history ends at the cutoff.

Native receipt journals and independently discoverable recovery files survive failed or
lost adoption acknowledgements. The library's recovery reservations include source version,
so a failed install followed by more edits can preserve another latest local copy without
overwriting the earlier recovery. Browser IndexedDB reserves a `recovering` catalog entry
before OPFS adoption. A completed file remains discoverable after a lost catalog ACK;
reopening validates it in Rust before promoting it to Ready. The worker and Web Lock own
SQLite/VFS lifetime; pages never edit SQLite or reconcile a JSON document mirror.

A failure before canonical writes, including recovery quota failure, keeps the old owner
usable and retryable. An uncertain canonical write invalidates that installation until
reload. Recovery bytes are not silently overwritten to make a retry succeed. Ordinary
open-session checkpoints preserve live CRDT identity; they do not perform a fresh epoch cut.

Layouts remain explicit. New layout 2 can use fresh accepted-state epochs. Supported layout 1
keeps its live lineage and uses identity-preserving seed/checkpoint behavior and `/compact`;
opening or displaying it never migrates it. `/compact` remains the same-history full-update
base path, with bounded archive expiry. A layout-2 history cut rejects pre-epoch snapshots
and updates with missing-history recovery instead of silently merging hidden old roots.

Source: [native epochs](../../crates/pop-sharing-client/src/epochs.rs),
[native journal](../../crates/pop-sharing-client/src/journal.rs),
[browser transport](../../packages/pop/src/sharing/worker.ts),
[owner epoch effects](../../crates/pop-runtime/src/owner/actor/epochs.rs),
[storage epoch copies](../../crates/pop-storage/src/store/epoch.rs).

## Stopping and finalizing

Creator `POST /handoff/begin` supplies an idempotent 32-hex token and exact expected
seed hash/heads. It freezes writes at that frontier. Guests immediately receive Ended,
keep their existing local copy, and have no final-download window. Native/browser guest
cleanup removes local relay membership before flushing, keeps UUID and accepted edits,
and preserves ownership on save failure.

A frozen room remains recoverable by creator identity through account lookup. Creator-only
seed, resource and binary `GET /handoff/history` reads allow catch-up to the fixed frontier.
History pages contain `Frame::Records`, at most 32 records with a 1 MiB target; one larger
base may exceed that target. `X-Sharing-Next` supplies the next `after` cursor and
`X-Sharing-Done` is `0` or `1`.

For a managed document, the creator catches up to the frozen frontier and flushes. The library
persists Handoff and conditionally publishes a private snapshot covering those heads. Its receipt binds room, transition,
freeze and snapshot version. Finalization flushes and checks the live owner against the durable
receipt. Edits after the receipt require restaging the payload under the same room/token/heads;
this does not unfreeze the room or create a new transition. Only after the durable destination
receipt and local independent membership state are established does creator
`POST /handoff/finalize` delete the room. Local, unmanaged documents require their accepted
saved local copy instead of a CloudKit receipt.

Frozen rooms do not idle-expire while completion is pending. Ordinary DELETE cannot bypass
a frozen handoff. Explicit account deletion is separately authorized to discard it. A finalized
room returns 410, so lost-finalize acknowledgements are retryable; catalog normalization follows
successful finalization. Creator recovery does not require the original device's link capability.

## Authentication, bounds, and failures

The creator is a verified non-anonymous Firebase identity; clients offer Apple sign-in. It can
reserve, recover capabilities, open, cancel Uploading, freeze/finalize, and delete its account.
A guest capability grants live collaboration and resource access, including eligible maintenance,
not creator control. Credentials remain device-local and outside document/CloudKit payloads.
Signing out does not revoke an already distributed capability; creator operations require renewed
identity. A cold signed-in device resolves the account's room, validates its canonical seed, then
attaches through the owner. Authored frames receive neither capability nor identity token.

| Bound | Current value |
| --- | --- |
| Rooms per account / connections per room / per-IP room sockets | 10 / 32 / 8 |
| Relay seed / private CloudKit upload / retained update bytes / same-history base | 16 MiB each |
| Retained update rows; maintenance hint | 32,768; 4 MiB or 8,192 rows |
| App resources | 256; 25 MiB per resource; 50 MiB total |
| Current attachments | 256; 10 MiB each; 100 MiB total |
| Staged epochs | 2; 10 minutes; 16 MiB candidate attachment bytes each |
| Push budget per socket | 8 MiB and 1,800 messages per minute, including retries |
| Catalog authority / archived system fields / change tag | 32 KiB / 64 KiB / 4 KiB |
| Live idle expiry / ended tombstone / legacy compaction archives | 90 days / 30 days / 14 days |

Entry-point budgets are shared across rooms per IP: metadata 120/minute, seed 30,
connections 60, blobs 120, and compaction/epoch stage-publication-release 6. They also
apply to creator requests on those routes. Account creation is 5/minute per account.
These Cloudflare limits are approximate per-location admission controls, not global quotas.

409 means a conditional race or unavailable current phase. 410 means Ended. Ordinary blob
413 means FilesFull; staged-epoch or base 413 means Outgrown. Layout 2 may attempt one fresh
maintenance cycle for FilesFull; confirmed successful sync resets eligibility for a later
independent episode. Oversized visible state remains paused rather than dropping content.
429, storage errors and network failures use retry/backoff policy. `requires_update` means
NeedsUpdate; malformed/unauthorized inputs are refused. Core missing dependencies require
history recovery. Socket error codes 1–5 keep their existing meanings; code 6 adds epoch
refresh. Unknown controls/variants fail closed. Frozen v1 fixtures are not rewritten.

The shared [limits](../../crates/pop-sharing-protocol/src/limits.rs),
[failure classification](../../crates/pop-sharing-protocol/src/failure.rs) and
[maintenance decisions](../../crates/pop-sharing-protocol/src/maintenance.rs) are executable
sources for these policies. Generated TypeScript/OpenAPI project Rust types, rather than
maintaining another handwritten wire schema.

## Qualification boundary

Owning checks cover core acceptance/convergence/publications, SQLite persistence and failed
recovery, native owner/CloudKit adapters, Chrome OPFS/catalog recovery, and the real Rust relay
in Miniflare. The relay checker exercises conditional publication, stage cleanup, frozen history,
creator/guest separation, rate budgets and cancellation. No production test clock or identity
bypass is compiled into the relay.

Run the repository's changed-tier verification and final native qualification before release.
Local successes do not replace signed-device Apple/Firebase/CloudKit checks, deployed relay
qualification, operational lifecycle configuration, or rollout verification. Keep those results
and remaining manual gates explicit in the [roadmap](../roadmap.md) and evidence reports.
