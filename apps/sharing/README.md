# Persistent cloud documents

The service stores one cloud document per canonical document UUID. Its SQLite Durable
Object owns owner identity, invitation generation, signed-in membership, lifecycle,
opaque update frontier and resource inventory. R2 stores immutable app resources,
attachments and checkpoint seeds. It does not execute apps or interpret Loro bytes.

Private synchronization and collaborative editing use the same document. Enabling an
invitation creates a capability; revoking it removes generation-bound guest memberships
and closes guest connections. Owner connections and canonical bytes remain. Only explicit
document or account deletion purges data. Offline documents never expire due to inactivity.
Portable local `.pop` files remain usable independently of the service.

## API

All document paths use a canonical UUID, identical to the UUID inside the accepted `.pop`.
There are no temporary room aliases or CloudKit handoff endpoints.

| Request | Authorization and result |
|---|---|
| `POST /v1/documents` `{document}` | Named Firebase identity; idempotently reserves the UUID and returns `CreatedRoom` with `room` equal to that UUID. An existing other-owner UUID is refused. |
| `GET /v1/account/documents` | Identity; lists registered UUID/role hints for repairing a missing personal catalog reference. Access still resolves through the DO. |
| `GET /v1/account/documents/{uuid}` | Identity; returns current `AccessState` from the authoritative DO. This is resolution, not catalog listing. |
| `GET /v1/documents/{uuid}` | Owner/member identity or active capability; `RoomInfo` with current seed/app metadata. |
| `GET /v1/documents/{uuid}/metadata` | Current authorized actor; reads title and template ID retained for catalog repair. |
| `PATCH /v1/documents/{uuid}` `{title,templateId}` | Owner identity; stores display metadata. |
| `POST /v1/documents/{uuid}/sharing` | Owner identity; enables an invitation idempotently and returns `AccessState` with its capability. |
| `GET /v1/documents/{uuid}/sharing` | Owner/member identity; returns current role, generation and sharing status. Only the owner receives the capability. |
| `DELETE /v1/documents/{uuid}/sharing` | Owner identity; revokes the invitation and its memberships, preserving the document. |
| `POST /v1/documents/{uuid}/members` `{capability}` | Named Firebase identity plus explicit active invitation; registers this account as an editor at the current generation. |
| `DELETE /v1/documents/{uuid}/members/me` | Member identity; leaves without deleting the document. Owners use explicit document deletion. |
| `DELETE /v1/documents/{uuid}` | Owner identity; revokes access, purges payloads and retains an identity/deletion tombstone. |
| `DELETE /v1/account` | Identity; durably fences the account, then retries deletion of owned documents and removal of joined memberships. |

Existing typed transport DTO names such as `RoomInfo` denote this persistent document,
not a second identity. The invitation is `https://share.dotpop.click/join#{uuid}.{capability}`;
the capability stays in the fragment. Anonymous browser editing remains supported.

Bootstrap uses owner-only `PUT /seed`, `POST /resources`, `PUT /asset/{hash}` and
`POST /open` beneath the document path. After activation, owner/member identities or an
active capability can read `/seed`, `/resources`, `/blobs`, `/blob/{hash}`, exchange
opaque updates over `/sync`, and publish bounded immutable attachments. Identity
verification uses Firebase's fixed certificate endpoint; clients cannot select signing
keys or assert a UID.

Checkpoint publication uses `GET /epoch`, immutable `PUT /epochs/{hash}`, candidate
attachment `PUT /epochs/{seed}/blobs/{hash}`, and exact-frontier `POST /epoch`. A losing
candidate can release its finite lease with `DELETE /epochs/{hash}`. Only the current
seed/inventory is canonical. Old sockets must refresh after an epoch change. `/compact`
retains one opaque current base, without old history archives. Rust owns request types,
resource limits and generated OpenAPI; do not hand-edit `openapi.json`.

## Client-written discovery

Firestore is a personal discovery index, never a permission or document-byte authority.
After Cloudflare acknowledges an upload, the client writes its own catalog reference
using the Firebase SDK. Rust retains the local pending catalog intent so SDK failure or
a crash can be retried. The backend does not query Firestore, publish catalog records,
hold a service account, or coordinate cross-service revisions.

If an upload succeeded before its catalog reference was written, the client can list
`GET /v1/account/documents`, resolve each UUID's current access and metadata, and repair
its own reference. The account registry is an index/quota and repair hint; only the
Document DO grants access. Revoking access may leave another person's catalog reference
visible until their client checks access. This is intentional; the stale reference cannot
read bytes or write changes after revocation. The client retains already downloaded local
work. Firestore rules and client SDK integration live outside this Worker.

Future template catalogs and hosted template publication remain deferred.

## Deployment and qualification

`wrangler.jsonc` deploys the new `dotpop-documents` Worker; the trusted host binds to it.
The retired production relay and resource-test Workers were deleted after the new
host binding was verified. The shared R2 bucket remains in use.
`wrangler.jsonc` and `wrangler.fresh.jsonc` declare new SQLite classes `CloudDocument`
and `DocumentAccount`, with `DOCUMENTS` and `ACCOUNTS` bindings. This is a clean cloud
cutover; downloaded local files are retained, but old temporary rooms are not migrated.
The existing R2 binding is `SEEDS`. No deployment runs as part of local verification.

Run `bun run verify sharing` through the repository runner. `check-accounts.ts` uses real
Rust WASM, SQLite DOs and R2 in Miniflare, substituting only external identity certificates,
identity certificates in its local outbound harness. Production has no test route,
asserted-UID bypass, configurable certificate endpoint or clock override. It covers
private identity, UUID ownership, access generations, uploaded-but-uncataloged registry
repair and persistent deletion. `check-compaction.ts` additionally covers opaque frames, bounds,
epoch conflicts, resource staging and cleanup. `check-live.ts` exercises anonymous browser
copies against the local service; `check-firestore.ts` qualifies the client-written personal catalog rules in the emulator. Deployment/platform acceptance remains separate from local test results.
