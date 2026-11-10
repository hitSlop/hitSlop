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
- **Owner.** One Firebase account (Sign in with Apple). Only the owner can create a cloud
  document and control the invitation and deletion.
- **Guests.** A guest joins by signing in with Apple; local editing needs no identity. A guest
  holds the editor role while the invitation generation they joined under is current.
- **Authority.** The per-document Durable Object (named by the UUID) owns permissions,
  lifecycle and accepted updates. A per-account Durable Object
  (named by the SHA-256 of the Firebase UID) registers the account's documents; that
  registration is only a discovery hint. Joins, leaves and registry cleanup on one
  document run one at a time: a join registers before it becomes a member, and a
  retried cleanup releases hints left by failed joins but never a live member's. R2 holds
  the seed and attachments under `{uuid}/…`. Every request is checked
  against Firebase identity and current membership; knowing a UUID grants nothing, and
  a link secret is exchanged once at `POST /members`, never reused as a credential.
- **Limits.** Up to 64 members per document (429 beyond), 32 live sockets per document.
  There is no cap on how many documents an account owns.

## API

Every route is a row of `ROUTES` in
`crates/pop-sharing-protocol/src/routes.rs`, which drives dispatch and per-IP rate limiting. An unlisted method,
path or trailing segment is a 404 before any limiter, identity check or Durable Object.
JSON bodies over their limit are 413 and malformed ones 400. Document sockets negotiate
`pop.sync.v2`; the library uses `pop.library.v1`. An unsupported protocol gets 426 `requires_update`.

| Route | Who | Effect |
|---|---|---|
| `GET /health` | public | Liveness. |
| `POST /v1/documents` | identity | Idempotently reserve the UUID; 204 with no body. |
| `GET /v1/account/library` | identity | WebSocket: complete library snapshot followed by metadata updates and removals. |
| `DELETE /v1/account` | identity | Fence the account, delete owned documents, leave joined ones. |
| `GET /v1/documents/{document}` | member | Phase, seed and app metadata, and whether compaction is due. It never carries the invitation. |
| `PATCH /v1/documents/{document}` | owner | Store title and template ID. |
| `DELETE /v1/documents/{document}` | owner | Revoke everyone, purge payloads, keep a tombstone. |
| `GET /v1/documents/{document}/metadata` | member | Title and template ID. |
| `GET`, `POST`, `DELETE …/sharing` | member (read), owner | The caller's current access, read from the document; enable (idempotent); revoke the invitation and its memberships. Only the owner receives the capability. |
| `POST …/members` | identity | Exchange the invitation secret for editor membership. |
| `DELETE …/members/me` | member | Leave without deleting the document. |
| `PUT …/seed`, `GET …/seed` | owner, member | The seed snapshot, uploaded with its SHA-256, app digest and version in `X-Sharing-*` headers; immutable once live. Its version is the document's root. |
| `POST …/open` | owner | Make the uploaded seed live. The body names it, `{"seedSha256": "<hex>"}`; any other seed is 409. Attachments are not checked. Replies with the document metadata. |
| `GET …/blobs`; `GET`, `PUT …/blob/{hash}` | member | Immutable attachments. |
| `POST …/compact` | member | Replace the retained updates with one base; 204 with no body. |
| `GET …/sync` | member | WebSocket carrying opaque update frames. |

The invitation is `https://share.dotpop.click/join#{uuid}.{capability}`; the capability
stays in the URL fragment. The page opened by a link is static: it sends people to dotpop on Mac
and iPhone and holds no credentials.

## Sync and compaction

On connect a replica sends its version and receives what it lacks; pushes carry whole
causal changes, one in flight at a time. Live sharing carries the document's retained
history, not a sanitized copy: a person who joins receives retained document history,
including content since removed from view (**Share a Copy** sends a sanitized copy instead).

The first upload seeds the document with the creator's saved file, staged once as an
immutable local snapshot. The seed is SQLite holding the complete embedded app and retained
history; attachments and artwork are not part of it. From that snapshot the creator uploads
the seed and every attachment the seed references, then calls
`POST …/open` with the seed's SHA-256. **The relay does not prove completeness**: Open
checks that it names the stored seed, never the attachments, so uploading every required attachment first is the creator's contract. A failed upload
leaves the document uploading and unbound; a retry stages again and sends only the blobs the
relay lacks. Downloads fetch, verify and persist attachments before becoming ready.

The seed is limited to 16 MiB of embedded app, history and SQLite overhead. A document
beyond it cannot be shared; keep large media in attachments, which have their own limits
below. The limit is checked before the cloud document is created, so a refusal leaves
nothing behind and keeps the local file. A document that is already live only reconnects,
however large it has grown. The 50 MiB app-resource budget does not guarantee that a
document fits; a build warns once the app stores 12 MiB of resources.

The cloud keeps one opaque **base** plus the updates since. **Compaction folds only**:
a client posts `/compact` with the base and its frontier, and the retained updates it
covers are replaced by one base that starts at the root stored with the seed; nothing
replaces history. Clients may compact once retained updates reach 4 MiB or 8,192 records.
The relay keeps at most 16 MiB or 32,768 records of updates and a base of at most
16 MiB (`BASE_BYTES`, a transport limit: the base travels as one WebSocket frame under
the 32 MiB frame limit and is stored in 1.8 MB chunks).

When a base would exceed 16 MiB, `/compact` answers 413 and the session pauses
`Outgrown`. Local edits keep saving. The person chooses **Save Unshared Copy…** to
fork the document into a new, independent one. A full attachment budget (10 MiB per
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

## Personal library

Each account Durable Object stores a derived library projection: document ID, title,
template ID, owner/editor role, sharing state and metadata revision. The Document
Object remains the sole content and permissions authority. After Open, rename,
sharing changes or Join, it durably queues publication and an alarm. Bounded fan-out
retries failed deliveries without requiring another edit. Leave and deletion remove
account entries. Uploading documents stay hidden until Open succeeds.

`GET /v1/account/library` negotiates `pop.library.v1` with a Firebase identity.
It sends byte/count-bounded snapshot pages, then upsert/remove messages. Clients apply
absence only after a complete snapshot; interruption preserves the last known library.
One account connection serves all library entries, and every device of that account
has its own connection. Account entries never contain invitation secrets or document bytes.

Existing registrations backfill from Document Object metadata when their account projection is missing; the cutover needs no cloud reset.

Rust owns the subscription and local index. Foreground reconnect obtains a current
snapshot; backgrounding stops the account socket. Account switches fence old callbacks.
Expired sockets cannot receive private updates and reconnect with refreshed identity.
Firebase remains responsible for Authentication, Analytics and Crashlytics.

The activity badge and its seen-state/protocol changes are deferred. Document replication
continues using the existing version-vector protocol.

## Operations

Deployment and local checks are in [apps/sharing/README.md](../apps/sharing/README.md).
Verify the Rust Worker, SQLite Durable Objects and R2 with `bun run verify sharing`.
