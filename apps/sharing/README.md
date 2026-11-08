# Sharing service

The Cloudflare Worker behind [cloud documents](../../docs/cloud-documents.md): a SQLite
Durable Object per document (`CloudDocument`) and per account (`DocumentAccount`), and
R2 for the seed, app resources and attachments. It validates identity, membership,
hashes, inventories and limits; it never executes apps or interprets Loro bytes. The
trusted browser host (`host.wrangler.jsonc`) serves share.dotpop.click and
`*.dotslop.com` and binds the document Worker.

Identity, the API, compaction and revocation are specified in
[cloud documents](../../docs/cloud-documents.md); receipts and recovery in
[sharing lifecycle](../../docs/reference/sharing-lifecycle.md). This page covers
operating the service.

## Routes

`crates/pop-sharing-protocol/src/routes.rs` holds the one route table (`ROUTES`: method,
path, scope, actor, rate-limit bucket, request and response types). Worker dispatch, the
per-IP rate-limit classification and the generated OpenAPI (`openapi.json`) all read it. A
method, path or trailing segment it does not list is a 404 before any limiter, identity
check or Durable Object. Request bodies go through one reader (`input::body`): 413 over
the byte limit, 400 when malformed, never a 500. `tools/pop-bindgen` is the only OpenAPI
generator (`bun run schema:generate`); do not edit `openapi.json`.

## Operator checklist

Nothing here runs as part of local verification, and nothing is provisioned by this
repository. Complete it before the first deployment.

1. **Firebase console.** Enable the Anonymous sign-in provider (browser and link guests
   depend on it) in the `FIREBASE_PROJECT` named in the wrangler files.
2. **Firestore rules.** Deploy `firestore.rules` from the repository root. They allow a
   person to read and write only `users/{uid}/pops/{documentId}`.
3. **Host Worker secret.** Set `FIREBASE_WEB_API_KEY` on the host Worker
   (`host.wrangler.jsonc`) for that project. It is the public Firebase Web API key, not a
   service-account credential or an Apple client key. Read it from the project's Web app (`firebase apps:sdkconfig WEB <app-id>`, or Project settings > Your apps) and set it with `wrangler secret put FIREBASE_WEB_API_KEY --config apps/sharing/host.wrangler.jsonc`. Without it `/auth/config` returns a
   retryable 503 and browser joins cannot start.
4. **Fresh resources.** `wrangler.jsonc` names an R2 bucket `dotpop-documents` and an
   Analytics dataset `dotpop_documents`. Create the bucket before deploying; the rate-limit
   bindings reference the namespace ids in the same file. Isolation from earlier products
   comes from these fresh names, not from code. Staging (`--env staging`) declares its own
   Worker, bucket, dataset and namespaces.
5. **Worker name.** The document Worker is `dotpop` (`dotpop-staging` for `--env staging`) and
   `host.wrangler.jsonc` binds its `RELAY` service to it. It is a new Worker, so migration tag
   `documents-v1` declares the Durable Object classes without conflict. `dotpop` has never
   had a live Durable Object, so its storage tables change without a migration until it ships.
   Every client reaches the service through `share.dotpop.click`, so deploying the host Worker moves all traffic to
   `dotpop` at once: pop 1.x clients lose live sharing and cloud documents when it goes out,
   and nothing from the earlier Worker carries over. Deploy `dotpop` first, the host Worker
   with the 2.0.0 release.
6. **Deploy.** `.github/workflows/sharing.yml` qualifies on matching `master` changes and
   deploys the document Worker, then the host, on a `pop-v*` tag or manual run when the
   repository variable `POP_DEPLOY_ENABLED` is `true`. A smoke check requests
   `https://share.dotpop.click/health`. `wrangler rollback` reverses a deploy.

## Qualification

`bun run verify sharing` builds the browser and Rust Worker and runs, in pinned
Miniflare with real Rust WASM, SQLite Durable Objects and R2:

- `check-accounts.ts`: identity, UUID ownership, access generations, revocation, the
  account registry and its cleanup under racing joins, deletion, request limits and the seed hash Open requires. Only external identity certificates are
  substituted, in the local outbound harness; production has no test route, asserted-UID
  bypass, configurable certificate endpoint or clock override.
- `check-compaction.ts`: opaque frames, bounds, folding under concurrent pushes,
  resource cleanup and the base size limit.
- `check-live.ts`: anonymous browser copies against the local service.
- `check-firestore.ts`: the client-written catalog rules in the Firebase emulator
  (Firebase CLI and Java required).

Deployment and platform acceptance are separate from these local results.

## Browser guest identity

The host serves public Firebase client configuration at `/auth/config`. A browser keeps
its anonymous refresh credential in the host origin's IndexedDB, isolated by project;
cross-tab Web Locks serialize signup and refresh. ID tokens are refreshed before HTTP
calls and socket reconnects, and authored frames receive neither credential. A retained
invalid credential is preserved until the person selects **Start a new guest session**,
which flushes an open document first; local documents and exports are kept. An open tab
that sees another tab reset the identity pauses rather than rebinding.

Browser sockets send `jwt.<ID token>` beside the public protocol and replies select only
the public protocol; native clients use the `Authorization` header. The signup and
refresh calls follow the [Firebase Auth REST API](https://firebase.google.com/docs/reference/rest/auth).
