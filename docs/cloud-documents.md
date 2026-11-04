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

Firestore is a discovery projection. Entries under
`users/{uid}/documents/{documentId}` contain `documentId`, `title`, `templateId`,
`appRevision`, `previewRef`, `modifiedAt`, `role`, `accessGeneration`,
`catalogRevision`, and `deleted`. `modifiedAt` is a Firestore timestamp; transport DTOs
use milliseconds. Roles are `owner` and `editor`. Clients can read their own catalog;
only the backend publishes entries. Credentials and document state never enter it.

The DO commits projection work to a durable outbox with lifecycle changes. Publication
is conditional on catalog revision and retried after failures. Deletion tombstones fence
older publications. A stale entry may temporarily remain visible, but cannot authorize
a document request. Firestore failure does not block an already opened document.

## User flows

Creation offers **This device** (default) and **Sync across devices**. Local documents
need no account. Enabling sync requires Firebase authentication, preserves the UUID and
live owner, and stages a complete initial upload. Pending or failed uploads preserve the
local file and offer retry; completion requires durable cloud acknowledgement.

Sign-in retrieves metadata. Files download when opened and remain available offline.
Already downloaded files refresh when opened; pending local changes reconnect through
the same sync client. Downloading validates and installs through Rust under normal
writer ownership. Invalid or newer unsupported content never modifies a valid local file.

Sharing enables an invitation on the existing cloud document. A local document must
first complete cloud initialization. Stopping sharing revokes guest access while the
owner's devices continue syncing. Guests retain downloaded copies and may save an
independent local copy. Exporting or duplicating never grants future cloud access.

Sign-out flushes owners, stops subscriptions and transport, and hides account-scoped
files and Recents without deleting files or pending edits. Failed saves retain ownership
and require retry. An account change cannot apply old listener events or credentials
to the new account. Explicit independent local copies stay visible.

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
managed files remain local and can opt into new sync. Cloud-only documents must first be
downloaded/exported with the previous app. No process deletes old CloudKit records.
Released `.pop` acceptance, app ABI, writer locking and compatibility corpora remain.
No `.pop` storage schema change is required; device catalog schemas are implementation
metadata and can change independently.

Firestore rules and indexes are in the repository root. The Worker uses the existing
`FIREBASE_PROJECT`, with `FIRESTORE_SERVICE_ACCOUNT_EMAIL` and
`FIRESTORE_SERVICE_ACCOUNT_PRIVATE_KEY` supplied as deployment secrets. The key is PKCS8
RSA PEM. OAuth goes only to Google's fixed token endpoint with the datastore scope.
No service credential belongs in the app or repository. IAM governs backend publication;
Firestore rules govern client reads. Production deployment is a separate release step.

Run the catalog access check with:

```sh
firebase emulators:exec --only auth,firestore --project demo-pop-catalog 'bun scripts/sharing/check-firestore.ts'
```

The sharing verification tier includes this emulator check. Server integration covers
initialization, authorization, revocation, epoch recovery and outbox retry ordering.
Native integration covers catalog delivery, owner lifetime and account switching.
Browser integration covers anonymous join, edit, offline reopen, revocation and export.
