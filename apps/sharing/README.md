# Sharing service

The Cloudflare Worker behind [cloud documents](../../docs/cloud-documents.md): a SQLite
Durable Object per document (`SharedDocument`) and per account (`AccountLibrary`), and
R2 for the seed and attachments. It validates identity, membership, hashes, inventories
and limits; it never executes apps or interprets Loro bytes. The same Worker owns
share.dotpop.click: it serves the API, and static assets from `public/` (the join page for
invitation links, the Sign in with Apple callback, the app-site association file and the
share icon). `public/_headers` sets their security and cache headers and `public/_redirects`
sends `/` to the public site.

Identity, the API, compaction and revocation are specified in
[cloud documents](../../docs/cloud-documents.md); receipts and recovery in
[sharing lifecycle](../../docs/reference/sharing-lifecycle.md). This page covers
operating the service.

## Routes

`apps/sharing/src/routes.rs` holds the one route table (`ROUTES`: method,
path, scope and rate-limit bucket). Worker dispatch and
per-IP rate-limit classification read it. A
method, path or trailing segment it does not list is a 404 before any limiter, identity
check or Durable Object. Request bodies go through one reader (`input::body`): 413 over
the byte limit, 400 when malformed, never a 500. Rust clients and the Worker share protocol types directly.

## Operator checklist

Nothing here runs as part of local verification, and nothing is provisioned by this
repository. Complete it before the first deployment.

1. **Firebase console.** Enable Sign in with Apple in the `FIREBASE_PROJECT` named
   in `wrangler.jsonc`. Cloud documents require a signed-in account.
2. **Resources.** Create the R2 bucket named in `wrangler.jsonc` before deploying.
   That file also names the Analytics dataset and rate-limit namespaces. Staging
   (`--env staging`) declares its own Worker, bucket, dataset and namespaces.
3. **Worker.** `dotpop` (`dotpop-staging` for `--env staging`) owns the
   `share.dotpop.click` custom domain. The migrations in `wrangler.jsonc` declare its two
   SQLite Durable Object classes and retire the classes an earlier deployment created.
4. **First single-Worker deployment.** Coordinate deployment with removal of the old
   `hitslop-sharing-host` Worker and its unused `FIREBASE_WEB_API_KEY` secret. Check the
   deployed resources first; removing the old host before the replacement is ready
   interrupts the join page, Apple callback and association file. Deployment remains
   release qualification work.
5. **Deploy.** `.github/workflows/sharing.yml` qualifies on matching `master` changes and
   deploys the Worker with its assets on a `pop-v*` tag or manual run when the repository
   variable `POP_DEPLOY_ENABLED` is `true`. A smoke check requests
   `https://share.dotpop.click/health` and the association file. `wrangler rollback` reverses a deploy.

## Qualification

`bun run verify sharing` builds the Rust Worker and runs, in pinned Miniflare with real Rust
WASM, SQLite Durable Objects and R2:

- `check-accounts.ts`: named identity, UUID ownership, revocation, the
  revisioned library outbox and cleanup under racing joins, deletion, request limits
  and the content-addressed seed and attachment inventory Open requires. Only external identity certificates are
  substituted, in the local outbound harness; production has no test route, asserted-UID
  bypass, configurable certificate endpoint or clock override. It also reads the files in
  `public/` that deploy (the callback's script hash against its policy, the association
  file, the join page's preview metadata) and checks that the Worker answers nothing outside the API.
- `check-compaction.ts`: opaque frames, bounds, folding under concurrent pushes,
  blob cleanup and the base size limit.

Deployment and platform acceptance are separate from these local results.

Native sockets upgrade without a subprotocol header. The route and relay Hello identify
what they speak. A 2.0.0 client is recognized by its `Sec-WebSocket-Protocol` header
(`pop.sync.v2` or `pop.library.v1`) and receives HTTP 426 with `requires_update`.
A Hello with an unsupported protocol or app digest receives the relay protocol error
frame, which native clients present as an update request.
