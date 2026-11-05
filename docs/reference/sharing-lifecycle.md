# Cloud document lifecycle

This reference describes the implemented persistent cloud service. The former
CloudKit snapshot/temporary-room authority handoff is removed. The product contract
is [cloud documents](../cloud-documents.md); the route and authorization inventory is
[sharing operations](../../apps/sharing/README.md). Historical spike reports remain
evidence of their original campaigns, not current lifecycle instructions.

## Identity and authority

A document has one canonical UUID, allocated locally and preserved when sync or sharing
is enabled. The Document Durable Object keyed by that UUID owns cloud permissions,
invitation generation, lifecycle and accepted opaque updates. Its SQLite tables are
service metadata and update storage, not a replica of the local `.pop` SQL tables.
R2 holds immutable checkpoint seeds, application resources and attachment blobs.
Clients validate document semantics and persist through the existing Rust owner.

Private synchronization and collaboration use this same authority. Enabling or stopping
sharing changes access, never byte authority. Internal protocol names such as `RoomInfo`
and the `share.room` SQL column identify the persistent document UUID; they do not imply
a separate temporary room or CloudKit checkpoint.

## Local creation and cloud initialization

1. Create a self-contained local `.pop` from an immutable template. Local-only use needs
   no account or network.
2. To enable sync, authenticate with Firebase and reserve the existing UUID through
   `POST /v1/documents`. The same owner's retry is idempotent; another owner's UUID is
   refused. Reuse an already-open local owner rather than opening a second writer.
3. Upload the accepted seed, resource manifest, app resources and referenced attachments,
   then activate through `POST /open` beneath the document path. Pending or failed
   initialization retains the local file and allows retry.
4. Only after durable cloud acknowledgement, enqueue the latest desired personal catalog
   reference in the Rust device index. Swift publishes it through the official Firestore
   SDK and acknowledges the local operation revision after completion.

A catalog-publication failure does not roll back an acknowledged upload or block editing.
The pending queue survives restart and replaces older desired writes for the same
account/document. A late acknowledgement cannot remove a newer pending write.

## Discovery and opening

Firestore stores personal references under `users/{uid}/documents/{documentId}`. Clients
read and write their own references; the Worker has no Firestore client or credentials.
A reference contains discovery and presentation hints, not permission to access bytes.
The DO authorizes access independently on every request.

Sign-in retrieves references; opening downloads and validates missing contents through
Rust before installation. Downloaded files remain complete, usable offline `.pop` files.
Account-generation fences prevent old listeners or asynchronous results crossing into
a different signed-in account. The account registry can list registered UUID/role hints
for repairing a missing reference after an upload; resolve each through the DO first.

A definitive access denial during a download check can remove the client's stale
reference. Cached guest copies may remain listed after transport access is refused.
Retain local work; discovery is intentionally eventual and needs no server publication
or cross-user Firestore writes. Closed-file CLI edits reconnect on the next native open.

## Invitations and membership

The owner enables sharing through `POST /sharing` beneath the existing document path.
A local-only document first completes cloud initialization. The resulting link is
`https://share.dotpop.click/join#{documentUUID}.{capability}`. Anonymous browser guests
can use the active capability. A named Firebase account can join with it through
`POST /members`; membership is bound to the current invitation generation.

Stopping sharing uses `DELETE /sharing`. It revokes the invitation, invalidates its
memberships and closes guest connections. Owner devices continue private sync against
the unchanged document authority. Guests keep copies already downloaded; there is no
final-snapshot download window, CloudKit receipt or authority handoff on Stop.
Leaving removes the guest membership without deleting the owner's cloud document.
Knowing a UUID, copying a `.pop`, or editing Firestore metadata grants no future access.

## Epoch publication and recovery

Ordinary updates reconnect through the same Rust synchronization path. Fresh epoch
maintenance bounds retained history without retaining every attachment or intermediate
state forever. The cloud keeps a usable current snapshot, bounded update tail and
referenced blobs, plus bounded temporary staging.

A client prepares an accepted latest-state candidate against an exact source version
and confirmed frontier, stages its checkpoint/resources, and conditionally publishes
against the current seed and frontier. The DO owns conditional publication; the client
owns validation and local adoption. Stale publication is refused rather than replacing
newer accepted state. Interrupted staging is retryable and cleanup removes superseded
payloads only after safe publication.

Local preparation never replaces the live document. Adoption uses the existing writer
lease and fences old sync traffic. Before refreshing an outdated local history, preserve
unconfirmed divergent work as an independent recovery document with a new UUID. Failure
to create that recovery leaves the original intact. Recovery receipts and managed paths
are relative to their owning local directories so an iPhone app-container relocation
does not create spurious recovery copies. A no-op adoption preserves undo and its blobs.

See [owner publication and adoption](../architecture.md#owner-admission-callbacks-and-epoch-installation) for the owner
boundary and [cloud documents](../cloud-documents.md) for account-scoped recovery.

## Account changes, eviction and deletion

Sign-out flushes document owners, stops account transport and hides account-scoped
entries without deleting downloaded bytes. A failed flush retains the owner and exposes
native Retry. Externally lost authentication fences transport and conceals the affected
account document while preserving its file and unsaved work.

**Remove Download** evicts only a closed, clean, confirmed local download and keeps its
reference. **Save Local Copy** creates a standalone file with a new UUID. **Delete
Everywhere** is owner-only cloud deletion with revocation, payload cleanup and a
retained deletion identity. Divergent local edits are preserved before applying remote
deletion. **Delete Account** fences the account and retries owned-document deletion and
joined-membership removal before Firebase identity deletion. Participant local copies
remain independent of remote access.

## Storage boundaries and limits

The canonical `.pop` schema is [storage-1.sql](../../crates/pop-storage/src/file/storage-1.sql).
The storage version, document layout, package format and runtime ABI have separate
compatibility responsibilities. Sync and catalog presentation do not change that SQL
schema or replace the app inside an existing document. New documents use layout 2;
released layout-1 acceptance remains in the same engine's adapter.

The device's `cloud-index.sqlite` is separate implementation metadata. Its account-scoped
entries, pending reference writes, nullable sharing hint, opened flag and managed paths
are created or migrated by [pop-library](../../crates/pop-library/src/lib.rs). They are
not extra tables in exported `.pop` documents or cloud access credentials.

Transport and operational limits are distinct from local document acceptance. Current
seed/base limits are 16 MiB each; retained update storage is separately bounded at
16 MiB/32,768 rows. Attachment limits are 10 MiB per blob, 100 MiB and 256 blobs per
document. Authoritative constants live in
[sharing limits](../../crates/pop-sharing-protocol/src/limits.rs) and the
[protocol](../../crates/pop-sharing-protocol/src/lib.rs). A valid local file may exceed
cloud transport limits; upload refusal preserves it.

## Qualification

Existing Rust tests own conditional epoch publication, dirty-stale recovery, retention,
resource validation and refusal atomicity. The sharing tier runs real Rust WASM with
SQLite Durable Objects/R2 and Firebase Auth/Firestore emulator rules. Native tests own
account fences, owner reuse and local installation. Browser checks own anonymous join,
offline copies, revocation and export. Physical-device and deployment evidence, including
remaining qualification limits, is recorded in the
[cloud document campaign](../evidence/cloud-documents-2026-11-04.md).
