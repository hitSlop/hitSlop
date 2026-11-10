# Sharing service

The Cloudflare Worker behind [cloud documents](../../docs/cloud-documents.md): a SQLite
Durable Object per document (`CloudDocument`) and per account (`DocumentAccount`), and
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

`crates/pop-sharing-protocol/src/routes.rs` holds the one route table (`ROUTES`: method,
path, scope and rate-limit bucket). Worker dispatch and
per-IP rate-limit classification read it. A
method, path or trailing segment it does not list is a 404 before any limiter, identity
check or Durable Object. Request bodies go through one reader (`input::body`): 413 over
the byte limit, 400 when malformed, never a 500. Rust clients and the Worker share protocol types directly.

## Operator checklist

Nothing here runs as part of local verification, and nothing is provisioned by this
repository. Complete it before the first deployment.

1. **Firebase console.** Enable the Anonymous sign-in provider (link guests depend on it)
   in the `FIREBASE_PROJECT` named in `wrangler.jsonc`.
2. **Library feed.** Account Durable Objects receive derived metadata from Document Objects. No Firestore configuration is required.
3. **Fresh resources.** `wrangler.jsonc` names an R2 bucket `dotpop-documents` and an
   Analytics dataset `dotpop_documents`. Create the bucket before deploying; the rate-limit
   bindings reference the namespace ids in the same file. Isolation from earlier products
   comes from these fresh names, not from code. Staging (`--env staging`) declares its own
   Worker, bucket, dataset and namespaces.
4. **Worker name.** The Worker is `dotpop` (`dotpop-staging` for `--env staging`) and owns
   the `share.dotpop.click` custom domain. It is a new Worker, so migration tag
   `documents-v1` declares the Durable Object classes without conflict. `dotpop` has never
   had a live Durable Object, so its storage tables change without a migration until it ships.
   Every client reaches the service through `share.dotpop.click`, so the deploy moves all
   traffic to `dotpop` at once and nothing from the earlier Worker carries over.
5. **Deploy.** `.github/workflows/sharing.yml` qualifies on matching `master` changes and
   deploys the Worker with its assets on a `pop-v*` tag or manual run when the repository
   variable `POP_DEPLOY_ENABLED` is `true`. A smoke check requests
   `https://share.dotpop.click/health` and the association file. `wrangler rollback` reverses a deploy.

## Qualification

`bun run verify sharing` builds the Rust Worker and runs, in pinned Miniflare with real Rust
WASM, SQLite Durable Objects and R2:

- `check-accounts.ts`: identity, UUID ownership, access generations, revocation, the
  account registry and its cleanup under racing joins, deletion, request limits and the seed hash Open requires. Only external identity certificates are
  substituted, in the local outbound harness; production has no test route, asserted-UID
  bypass, configurable certificate endpoint or clock override. It also reads the files in
  `public/` that deploy (the callback's script hash against its policy, the association
  file, the join page's preview metadata) and checks that the Worker answers nothing outside the API.
- `check-compaction.ts`: opaque frames, bounds, folding under concurrent pushes,
  blob cleanup and the base size limit.

Deployment and platform acceptance are separate from these local results.
