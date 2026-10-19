# Live sharing developer preview

This is a deployable Cloudflare application. Reusable contracts live in
`crates/hitslop-sharing-protocol`; the native transport lives in
`crates/hitslop-sharing-client`. The Worker has no dependency on Loro or the document
engine. It stores opaque updates in one SQLite Durable Object per room and immutable
seed/app/attachment bytes in a private R2 bucket.

`share.hitslop.com` hosts trusted storage and connection workers. Each browser copy runs
authored UI on its own `<copy UUID>.dotslop.com`. The trusted worker holds the invitation;
the authored page receives the existing SDK bridge and host-bound, short-lived HttpOnly
resource cookies. No embedded Service Worker is needed. Browser SQLite uses OPFS sync
access handles; IndexedDB stores the copy catalog. Opening needs a connection. An open
copy remains editable offline; connected reload/reopen is qualified.

The native UI is disabled by default. Debug builds require `--live-sharing-preview` or
`HITSLOP_LIVE_SHARING_PREVIEW=1`. Release builds additionally require the explicit
`HITSLOP_SHARING_PREVIEW` Swift compilation condition. Native creators sign in with
Apple through the separate Firebase account target. Guests need only an edit link;
neither side needs an iCloud library. Invitations are stored in device-local Keychain
items. CloudKit distributes the document, never its room credentials. A guest opens
the link again on each device; an owner recovers it from their verified account.
Signing out preserves existing capability-based sync and cached Copy Link, while
creation, recovery and Stop require sign-in.

One SQLite Account Durable Object per verified UID reserves/indexes rooms. It keeps
a recoverable plaintext edit capability privately until Stop; never log its responses.
The room stores only the capability hash plus an immutable verified owner UID and
room generation. It independently verifies Firebase identity on administration.
Reservation → immutable local SQLite seed → room init/uploads → room publication →
account finalization is retryable. The seed file itself is the local creation journal;
there is no JSON creation-state credential or staging marker. Competing devices can
reserve the same room, but cannot replace its first accepted seed. A failed upload
can be finished on the device holding that seed, or cancelled with Stop.

Share preserves UUID, path, owner and full history. Join reuses an open or known
matching file, or installs a missing canonical file in Application Support. Conflicting
active membership is refused. Stop tombstones before cleanup and forgets the registry's
plaintext capability; in-flight uploads check the tombstone and remove late writes.
Re-share explicitly increments the generation and creates a new room. Stale requests
cannot stop the replacement. Each UID may have 10 active/pending/stopping rooms;
each room admits 32 connections. `CREATION_ENABLED` disables new reservations;
operator rate limits start at 5 creates/minute/UID and 20 requests/minute/IP.

The preview remains disabled by default. Account deletion, full-history recovery,
remaining abuse controls and physical-device qualification still gate a public beta.
Old unversioned preview endpoints and credentials are unsupported; their remote
resources still need operator retirement. Local documents are retained.

## Build and deploy

Use the workspace Rust/wasm-bindgen tools, LLVM and Binaryen described in the normal
build setup. Install `worker-build 0.8.7` into `generated/core-tools`, or supply
`HITSLOP_WORKER_BUILD` pointing to that version.

```sh
cargo install worker-build --version 0.8.7 --locked --root generated/core-tools
bun scripts/build/sharing.ts
wrangler deploy --dry-run --config apps/sharing/wrangler.jsonc
wrangler deploy --dry-run --config apps/sharing/host.wrangler.jsonc
```

Create the private `hitslop-sharing-preview` R2 bucket once. The `dotslop.com` zone
needs a proxied wildcard DNS record (`AAAA * → 100::`); the host config installs its
wildcard Worker route and the `share.hitslop.com` custom domain. Existing exact spike
custom domains are retained. The landing site's apex Worker is separate.

Deploy the relay first (the v2 migration adds the account namespace), then the host.
No developer creation secret is shipped or accepted. The deployed fixture driver now
requires a freshly refreshed Firebase ID token in an owner-readable file supplied by
`HITSLOP_SHARING_TOKEN_FILE`; never put its contents in arguments or logs.

```sh
wrangler deploy --config apps/sharing/wrangler.jsonc
wrangler deploy --config apps/sharing/host.wrangler.jsonc
cargo build --locked -p hitslop-sharing-client --example preview
HITSLOP_SHARING_TOKEN_FILE=/private/path/to/token bun scripts/sharing/check-preview.ts
bun run verify sharing
```

`verify sharing` is an explicit qualification tier. It builds the real Rust WASM relay
and runs it with SQLite DOs and private R2 in pinned Miniflare. Only that local
runtime's Google certificate fetch is replaced by a local public test key. Production
has no test identity path. It exercises two creators, concurrent/idempotent reservations,
immutable init, partial publication, guest-admin refusal, cold owner recovery,
Stop during upload with empty retained R2 afterward, re-share and delayed init.
The normal `verify`/`--native` tiers remain the required portable/native checks.

The explicit deployed check creates and stops disposable fixture rooms. It qualifies
real native/browser edits, restricted-cookie private assets, late join, interrupted
file reception, process reopen, `.slop` download/native reopening and revocation.
Reports and credentials stay under ignored `.hitslop/sharing-preview/check-*`; secrets
are never included in the report. The preview example is a test driver, not a shipped
CLI sharing command. Ordinary CI uses `bun run verify` (including Worker WASM clippy);
use `bun run verify --native` when native/FFI work is complete.

## Native identity

The developer-gated native **Sharing Account** panel signs in with Apple through a
separate, named Firebase app. It refreshes a Firebase ID token and passes that transient
token through UniFFI to the Rust client. `GET /v1/auth/probe` verifies it in the Rust
Worker against Google's signing certificates and the configured Firebase project.
The panel compares the returned UID with Firebase's UID and displays a short hash for
cross-device comparison. Tokens and full UIDs are not logged or put in documents.
`AUTH_PROBE_ENABLED` controls this route; `AUTH_LIMIT` throttles it by IP.
Firebase manages its own local login persistence. The signed Mac proof completed
Apple sign-in and deployed verification successfully; the iPhone comparison is pending.

The account registry and native lifecycle now use this identity. The old creator
admin capability, preview creation key, synchronizable credentials and credential
polling have been removed. Transport callbacks drive status; owner recovery runs on
open, foreground, account change and opening the sharing controls. Matching native
Mac/iPhone identity and real second-device recovery are still qualification gates.

The current build IDs remain `com.hitslop.app` (direct-download Mac) and
`com.hitslop.iospreview` (development iOS). Both are grouped under the primary Apple
sign-in ID `com.hitslop`, and both have Firebase registrations in `hitslopapp`.
Production iOS and a future Mac build distributed through its same App Store record
must use that record's `com.hitslop` bundle ID. Changing these development IDs is not
needed to prove shared sign-in identity.

For CloudKit diagnosis, a Debug build can opt into `HITSLOP_LIBRARY_TRACE=1` and a
fresh `HITSLOP_LIBRARY_DIAGNOSTIC_NAMESPACE` (1–64 ASCII letters, digits or hyphens).
Use the same namespace on both test devices. It isolates the local library metadata
and CloudKit zone from the ordinary library. Traces describe engine lifecycle,
cursor persistence and event counts, with hashed cursor/version fingerprints and no
document payloads or record IDs. Do not delete cursors as a recovery step. This
instrumentation does not yet establish the cause of the earlier discovery misses.

Rust owns HTTP DTOs, the invitation, resource policy, binary envelope and shared limits.
`bun run schema:generate` generates TS and `openapi.json`. HTTP callers use those types;
OpenAPI describes metadata/file routes, while WebSocket updates use the separately
versioned binary protocol. There is no oRPC or second Swift HTTP client.

## Preview limits and remaining qualification

- Initial seed: 16 MiB; retained relay updates: 16 MiB. No room compaction yet.
- Immutable app and attachment limits are the core's existing acceptance limits.
- Automatic CloudKit checkpoints for relay documents are at most once a minute per
  process. Manual/background saves bypass the interval; ordinary documents are unchanged.
- Account-backed ownership is implemented behind the developer flag. Public beta
  still needs account deletion, full-history rescue and remaining abuse controls.
  Person-level permissions, read-only invitations, public catalog, universal links
  and reliable offline web reopening remain deferred.
- The earlier signed Mac preview passed creation, Copy Edit Link, credential restoration,
  native/browser edits, Stop and continued local editing. The account-backed replacement
  requires a fresh signed Mac/deployed campaign; phone UI, same-UID recovery, native Join
  and CloudKit coexistence remain qualification gates. CloudKit discovery/cursor misses
  remain open.
- No new latency claim: the previous resource probe measured 430–439 ms application
  echo RTT, including inventory/file coordination. One-way and paint latency are unknown.

See the [architecture](../../docs/architecture.md#sync),
[roadmap](../../docs/roadmap.md#later) and
[account lifecycle evidence](../../docs/evidence/live-sharing-accounts-2026-10-19.md)
and [earlier preview evidence](../../docs/evidence/live-sharing-preview-2026-10-19.md).
