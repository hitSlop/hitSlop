# Cloud documents and sharing

The Rust Worker hosts persistent cloud documents. Firestore provides discovery;
SQLite Document DOs authorize access and store opaque Loro updates; private R2 holds
snapshots, immutable app resources and attachments. See the complete
[cloud document specification](../../docs/cloud-documents.md) and
[architecture](../../docs/architecture.md#sync).

## Identity and lifecycle

Each document DO is addressed by the document UUID. The Account DO reserves creation,
enforces quotas and coordinates owned-document deletion. Initial upload becomes usable
only after the seed and resource inventory are durable. Retry targets the same UUID.
Another account cannot claim an existing document.

An owner accesses a private document with verified Firebase identity. Sharing enables
an invitation on that same document. Link guests use its capability; signed-in guests
can register generation-bound membership for cross-device discovery. Revocation ends
all invitation-derived access and guest sockets, while owner sync continues. Private
cloud documents have no idle expiration. Explicit deletion removes payloads and fences
later writes; account deletion completes owned-document cleanup before removing identity.

Firestore entries contain metadata, never document state, capabilities or service
credentials. Backend publication is a durable outbox operation. Revision checks and
tombstones prevent delayed writes restoring stale catalog entries. Client access rules
allow users to read only their own catalog and deny all client publication. The DO
always authorizes actual document access, even when discovery metadata is stale.

## Transport and storage

`POST /v1/documents` initializes a document; subsequent operations use
`/v1/documents/{UUID}` and its seed, epoch, epochs, sync, resources and blob routes.
Invitation links use `/join#{UUID}.{capability}`. Rust owns the wire contracts and
OpenAPI generation; regenerate bindings rather than editing generated files.

Epoch publication compares the current seed identity and accepted frontier. New seeds
and referenced resources are staged immutably before publication. Superseded payloads
are garbage-collected; unpublished stages remain bounded and expire. An old offline
client preserves uncertain edits in an independent recovery document before refreshing.
The server carries opaque Loro state and does not execute apps or accept SQLite row edits.

Existing resource, account, connection and rate bounds are centralized in
`pop-sharing-protocol`. Quotas are admission policy, separate from saved-file acceptance.
Local files remain usable when a cloud admission limit refuses sync. Neither R2 buckets
nor authored pages receive service credentials. Resource sessions and isolated frame
origins remain enforced by the Rust service.

## Firestore setup

Use the existing Firebase project configured by `FIREBASE_PROJECT`. Deployment needs:

- Firestore rules and indexes from the repository root (`firebase.json`).
- A backend service account with IAM permission to read/write catalog records.
- Worker secrets `FIRESTORE_SERVICE_ACCOUNT_EMAIL` and
  `FIRESTORE_SERVICE_ACCOUNT_PRIVATE_KEY` (PKCS8 RSA PEM).

The Worker obtains OAuth tokens through Google's fixed endpoint using datastore scope.
Private credentials must never be committed, embedded in clients or printed in logs.
Firebase Authentication remains the identity provider; CloudKit is no longer involved.
The service account uses IAM; client reads use Firestore security rules.

No production deployment is implied by the implementation PR. Provision credentials,
deploy rules/indexes, qualify staging with real identities, and deploy the service and
clients together. CloudKit is a clean cutover: old cloud-only documents must first be
downloaded with the old app. The new service does not delete old CloudKit records.

## Build and qualification

Use the workspace Rust, wasm-bindgen, LLVM and Binaryen setup. The sharing build uses
`worker-build 0.8.7` from `generated/core-tools` (or `POP_WORKER_BUILD`).

```sh
bun scripts/build/sharing.ts
bun run verify sharing
bun run verify
bun run verify --native
```

The sharing tier executes the actual Rust WASM Worker with SQLite DO/R2 in pinned
Miniflare under Node. Its local certificate service uses generated signing keys; the
production verifier has no test identity bypass. The tier also runs Firebase Auth and
Firestore emulators to verify catalog access rules. Install Firebase CLI and Java.
To run only the catalog check:

```sh
firebase emulators:exec --only auth,firestore --project demo-pop-catalog 'bun scripts/sharing/check-firestore.ts'
```

Keep high-value checks at their boundaries: idempotent initialization, owner/guest
permission enforcement, revocation, durable outbox retry ordering, deletion fencing,
snapshot/attachment verification, epoch adoption and offline recovery. Browser/native
qualification covers real joins, edits, reconnect, export and account switching.

## Host and infrastructure

The private R2 bucket is `hitslop-sharing`. The trusted host is
`share.dotpop.click`; authored pages use isolated UUID subdomains of `dotslop.com`.
The wildcard zone needs a proxied wildcard DNS record. The host preserves AASA,
`/join` universal links and the native Apple sign-in callback, with fragment credentials
kept off the server. Existing Analytics/Crashlytics, Sparkle and native exports are
independent of this service.

Hosted template publishing is future work. Its intended catalog is Firestore metadata
pointing to validated immutable R2 template revisions. Updating a template listing never
changes the app embedded inside an existing document.
