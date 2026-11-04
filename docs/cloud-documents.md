# Persistent cloud documents

This specification replaces the CloudKit private library and temporary collaboration
rooms. Implementation is tracked on `cloud-document-sync`, stacked over
`rust-foundation-redesign`. Qualification and deployment status belong in the roadmap.

## Identity and authority

Every document has an independent UUID, allocated locally. Enabling sync, downloading
on another device, and joining collaboration preserve it. An independent copy gets a
new UUID. A template ID identifies an app family; its immutable revision identifies the
exact embedded app. Neither is a document ID or an account ID.

A synced document has one owning Firebase account. The owner controls invitations and
cloud deletion. Link guests may be anonymous. A signed-in guest joins the same document
and receives a library entry on all of their devices while that invitation is active.
Guest membership is bound to the invitation generation; revocation closes affected
connections and invalidates remembered access. Knowing the UUID grants no access.

The Document Durable Object, keyed by UUID, owns permissions, lifecycle, accepted opaque
Loro updates, current epoch and resource inventory. It does not interpret application
state or replicate SQLite rows. Rust clients own semantics and local persistence.
R2 contains immutable snapshots, app resources and attachments. Each local `.pop` remains
a complete SQLite file, usable and exportable offline. One Rust owner writes it.

Firestore is a client-maintained list of personal document references. Entries under
`users/{uid}/documents/{documentId}` contain document identity, display metadata and
access hints. Clients read and write only their own list. Those hints never authorize
access to a cloud document; the DO checks every request.

After Cloudflare confirms an upload, Rust records a pending catalog write in its local
index. Swift uses the official Firestore SDK to publish it, then acknowledges that local
operation. Retry after sign-in or network failure is idempotent. The queue retains only
the latest desired entry for each document. A local operation revision prevents an old
acknowledgement clearing a newer pending write; it is not a distributed catalog clock.

Firestore listeners discover entries on other devices. Revocation is immediate at the
DO, but a guest's reference may remain listed until their client checks access. The
client then cleans up its own reference. Shared titles and other metadata may likewise
refresh when opened. This intentionally eventual library requires no server publication
or cross-user Firestore writes.

If upload succeeds before catalog publication and that client disappears, the account
registry lists registered document IDs for discovery repair. Resolve those IDs through
the DO before restoring references. The Worker has no Firestore credentials or client.
Firestore failure does not block local editing or an existing cloud connection.

## User flows

Creation offers **This device** (default) and **Sync across devices**. Local documents
need no account. Enabling sync requires Firebase authentication, preserves the UUID and
live owner, and stages a complete initial upload. Pending or failed uploads preserve the
local file and offer retry; completion requires durable cloud acknowledgement.

Sign-in retrieves metadata. Files download when opened and remain available offline.
Already downloaded files refresh when opened; pending local changes reconnect through
the same sync client when opened. Edits made by a closed-file CLI owner remain local
until the native document is opened; background ownership of closed documents is not
part of this change. Downloading validates and installs through Rust under normal
writer ownership. Invalid or newer unsupported content never modifies a valid local file.

Sharing enables an invitation on the existing cloud document. A local document must
first complete cloud initialization. Stopping sharing revokes guest access while the
owner's devices continue syncing. Guests retain downloaded copies and may save an
independent local copy. Exporting or duplicating never grants future cloud access.

Sign-out flushes owners, stops subscriptions and transport, and hides account-scoped
files and Recents without deleting files or pending edits. Failed saves retain ownership
and require retry. An account change cannot apply old listener events or credentials
to the new account. Explicit independent local copies stay visible. Automatic cloud recovery copies remain
account-scoped local documents: discoverable on sign-in, hidden on sign-out, and never
automatically uploaded. A recovery journal is indexed after restart, and final owner
unregistration preserves later accepted page edits before hiding a deleted source.

Actions have distinct effects:

- **Remove Download:** evict only a closed, clean, confirmed local download. Keep its
  catalog entry. Refuse eviction with pending or uncertain local work.
- **Save Local Copy:** create a self-contained independent document with a new UUID.
- **Leave Shared Document:** remove guest membership/discovery; preserve existing local
  work for independent copying. Other participants are unaffected.
- **Delete Everywhere:** owner-only cloud deletion, revocation and payload cleanup.
  Preserve divergent local edits in recovery copies before applying remote deletion.
- **Delete Account:** delete owned cloud documents and guest memberships before deleting
  Firebase identity. Participant local copies survive. The UI explains cloud deletion.

Cloud documents do not expire merely because they are idle. Retain the current usable
snapshot, bounded update tail and referenced blobs, plus bounded temporary staging.
Superseded payloads are garbage-collected after safe epoch publication. This is not a
cloud history service. Existing configurable size, socket and account limits initially
remain; billing and revised commercial quotas are separate work.

## Web and templates

Shared links use `/join#{documentUUID}.{capability}`. The host resolves
`/v1/documents/{documentUUID}` and uses the document's seed, epoch, resource and sync
routes. Anonymous browser joining remains supported. Firestore is not needed to render
a link. Rust/WASM, SQLite/OPFS, isolated app frames, browser-local copies and `.pop`
downloads remain the browser architecture. Chrome remains the qualified browser.
Revocation ends remote access while preserving the existing local browser copy.

Future templates use Firestore listings pointing to immutable validated R2 artifacts.
Creating from one embeds a specific revision in a new document. Updating a listing
never replaces apps in existing files. Hosted publishing, embedded-app updates,
schema evolution, signed-in web libraries, ownership transfer and read-only links
are not part of this change.

## Cutover and operations

This is a clean CloudKit cutover without an automatic importer. Previously downloaded
managed files remain local and can opt into new sync. Startup discovers only known
previous library directories and validates files through Rust. Normal adoption preserves
UUID and path, and retains a prior local-index title when available. For distinct files
with one UUID and divergent state, preserve both originals and expose a clearly labeled
independent recovery copy; a durable source mapping prevents repeated recovery copies.
Cloud-only documents must first be
downloaded/exported with the previous app. No process deletes old CloudKit records.
Released `.pop` acceptance, app ABI, writer locking and compatibility corpora remain.
No `.pop` storage schema change is required; device catalog schemas are implementation
metadata and can change independently.

Firestore rules and indexes are in the repository root. Client SDKs use the existing
Firebase project and Firebase Authentication account. Rules permit catalog reads and
writes only below the requesting user's own UID. Cloudflare verifies Firebase tokens
for document access but never connects to Firestore. Existing template and publisher
collection policies remain in place. Production service and rules deployment is recorded
in the [deployment evidence](evidence/cloud-documents-2026-11-04.md).

Run the catalog access check with:

```sh
firebase emulators:exec --only auth,firestore --project demo-pop-catalog 'bun scripts/sharing/check-firestore.ts'
```

The sharing verification tier includes this emulator check. Server integration covers
initialization, authorization, revocation, epoch recovery and registry-based discovery repair.
Native integration covers catalog delivery, owner lifetime and account switching.
Browser integration covers anonymous join, edit, offline reopen, revocation and export.
