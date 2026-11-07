# Cloud documents

A cloud document is an ordinary `.pop` file plus a persistent copy on Cloudflare that
keeps its owner's devices and any invited guests in sync. Each device keeps a complete
local file, usable and exportable offline; one Rust owner writes it. The cloud holds
opaque Loro updates and never interprets them. Operators deploy it from
[apps/sharing](../apps/sharing/README.md); receipts and recovery are in
[the sharing lifecycle](reference/sharing-lifecycle.md).

## Identity and authority

- **Document ID.** An independent UUID allocated locally. Enabling sync, downloading and
  joining preserve it; an independent copy gets a new one. A template ID names an app
  family; it is neither a document nor an account ID.
- **Owner.** One named Firebase account (Sign in with Apple). Only a named identity can
  create a cloud document. The owner controls the invitation and deletion.
- **Guests.** A link guest signs in with Firebase Anonymous Auth, automatically; local
  editing needs no identity. Linking Apple upgrades a guest and keeps its UID. A guest
  holds the editor role while the invitation generation they joined under is current.
- **Authority.** The per-document Durable Object (named by the UUID) owns permissions,
  lifecycle, accepted updates and the resource inventory. A per-account Durable Object
  (named by the SHA-256 of the Firebase UID) registers the account's documents. R2 holds
  the seed, app resources and attachments under `{uuid}/…`. Every request is checked
  against Firebase identity and current membership; knowing a UUID grants nothing, and
  a link secret is exchanged once at `POST /members`, never reused as a credential.
- **Limits.** Up to 64 members per document (429 beyond), 32 live sockets per document.
  There is no cap on how many documents an account owns.

## API

Every route is a row of `ROUTES` in
`crates/pop-sharing-protocol/src/routes.rs`, which drives dispatch, per-IP rate limiting
and the generated [`openapi.json`](../apps/sharing/openapi.json). An unlisted method,
path or trailing segment is a 404 before any limiter, identity check or Durable Object.
JSON bodies over their limit are 413 and malformed ones 400. Sockets negotiate
`pop.sync.v2`; an older protocol gets 426 `requires_update`.

| Route | Who | Effect |
|---|---|---|
| `GET /health` | public | Liveness. |
| `POST /v1/documents` | named identity | Idempotently reserve the UUID; 204 with no body. |
| `GET /v1/account/documents` | identity | Registered UUIDs and roles, a hint for repairing a missing catalog entry. Paged: up to 1,000 per reply, `?after=<last uuid>` continues and `next` marks another page; the client follows every page. |
| `DELETE /v1/account` | identity | Fence the account, delete owned documents, leave joined ones. |
| `GET /v1/documents/{document}` | member | Phase, seed and app metadata, and whether compaction is due. It never carries the invitation. |
| `PATCH /v1/documents/{document}` | owner | Store title and template ID. |
| `DELETE /v1/documents/{document}` | owner | Revoke everyone, purge payloads, keep a tombstone. |
| `GET /v1/documents/{document}/metadata` | member | Title and template ID. |
| `GET`, `POST`, `DELETE …/sharing` | member (read), owner | The caller's current access, read from the document; enable (idempotent); revoke the invitation and its memberships. Only the owner receives the capability. |
| `POST …/members` | identity | Exchange the invitation secret for editor membership. |
| `DELETE …/members/me` | member | Leave without deleting the document. |
| `PUT …/seed`, `GET …/seed` | owner, member | The seed snapshot, uploaded with its SHA-256, app digest and version in `X-Sharing-*` headers; immutable once live. Its version is the document's root. |
| `GET`, `POST …/resources`; `PUT …/asset/{hash}` | member, owner | App resource manifest and its files. |
| `POST …/open` | owner | Make a fully uploaded document live; replies with the document metadata. |
| `GET …/blobs`; `GET`, `PUT …/blob/{hash}` | member | Immutable attachments. |
| `POST …/compact` | member | Replace the retained updates with one base; 204 with no body. |
| `GET …/sync` | member | WebSocket carrying opaque update frames. |

The invitation is `https://share.dotpop.click/join#{uuid}.{capability}`; the capability
stays in the URL fragment. Browser guests edit through the same routes with a Firebase
ID token; authored frames never receive credentials.

## Sync and compaction

On connect a replica sends its version and receives what it lacks; pushes carry whole
causal changes, one in flight at a time. The first upload seeds the document with the
creator's saved file as one snapshot: attachments and artwork no longer referenced are
dropped. Live sharing carries the document's retained history, not a sanitized copy: a person
who joins receives everything the saved file holds, including content since removed from view
(**Share a Copy** sends a sanitized copy instead).

The cloud keeps one opaque **base** plus the updates since. **Compaction folds only**:
a client posts `/compact` with the base and its frontier, and the retained updates it
covers are replaced by one base that starts at the root stored with the seed; nothing
replaces history. Clients may compact once retained updates reach 4 MiB or 8,192 records.
The relay keeps at most 16 MiB or 32,768 records of updates and a base of at most
16 MiB (`BASE_BYTES`, a transport limit: the base travels as one WebSocket frame under
the 32 MiB frame limit and is stored in 1.8 MB chunks).

When a base would exceed 16 MiB, `/compact` answers 413 and the session pauses
`Outgrown`. Local edits keep saving. The person chooses **Save Unshared Copy…** to
fork the document into a new, independent one. A full attachment budget (10 MB per
file, 100 MiB and 256 files per document; they sit outside history) pauses sharing as
`FilesFull`.

History grows slowly for typical edits (about one byte per typed character, about ten
bytes per checklist commit), so fold keeps small documents far below the cap. Rewriting
a large scalar value every commit stores the whole value again each time (ratio 1.00;
[measurements](evidence/room-compaction-2026-10-22.md)). Put large values in attachments.

## Revocation, leaving and deletion

- **Stop sharing** revokes the invitation and every guest membership, and closes guest
  connections. Owner devices keep syncing; guests keep their downloaded copy and can
  save an independent one.
- **Leave** removes the guest's membership. Others are unaffected.
- **Delete everywhere** (owner) revokes access, purges payloads and keeps a tombstone.
  A device holding unsynced edits keeps them as a local "Recovered edits" copy.
- **Delete account** deletes owned documents and memberships, then the Firebase identity.
- Cloud documents never expire from inactivity.

## Personal catalog

Firestore is each person's list of document references at
`users/{uid}/pops/{documentId}` (see `firestore.rules`): a client reads and writes only
its own entries. After Cloudflare acknowledges an upload, Rust records a pending
catalog write, Swift publishes it with the Firestore SDK and acknowledges it; retry is
idempotent. The entry carries identity and display metadata, never access: the
Durable Object checks every request. A guest's entry can outlive revocation until
their client next checks access; it cannot read bytes. If an upload succeeded but the
entry was never written, the client repairs it from `GET /v1/account/documents`. The
Worker has no Firestore credentials, and a Firestore outage never blocks editing or an
open connection.

## Operations

Deployment, the operator checklist and local checks are in
[apps/sharing/README.md](../apps/sharing/README.md). Verify with `bun run verify sharing`
(Rust WASM Worker in Miniflare, plus the Firestore rules check below) and
`bun run verify browser` for anonymous join, edit, offline reopen and export.

```sh
firebase emulators:exec --only auth,firestore --project demo-pop-catalog 'bun scripts/sharing/check-firestore.ts'
```
