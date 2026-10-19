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
`HITSLOP_SHARING_PREVIEW` Swift compilation condition. Native creation requires
`HITSLOP_SHARING_CREATE_KEY` in the developer launch environment; never put it into a
bundle, invitation, browser build or tracked launch scheme. Receiving an invitation
requires neither that key nor an iCloud library. Both signed Apple apps use the shared
synchronizable Keychain group `$(AppIdentifierPrefix)com.hitslop.sharing`.

Share preserves the current document UUID, path, owner and full history. Credentials are
saved before staging; a local preparation marker prevents another device from staging
a different seed. Its stable hash/version are saved before publication. Creation can
retry the same room, seed and immutable resources. The invitation appears after
publication and credential persistence. Join reuses an open or known matching file, or
installs a missing canonical file in Application Support. Conflicting membership is
refused. Stop tombstones the room, revokes new resource access and deletes its retained
remote files; local copies remain usable.

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

Deploy the relay first, configure `PREVIEW_CREATE_KEY` through `wrangler secret bulk`
using an owner-readable, ignored JSON file, then deploy the host. The development
machine's file is `.hitslop/sharing-preview/secrets.json`; it is not an installation
artifact. Do not print the key or pass its value as a command argument.

```sh
wrangler deploy --config apps/sharing/wrangler.jsonc
wrangler secret bulk .hitslop/sharing-preview/secrets.json --config apps/sharing/wrangler.jsonc
wrangler deploy --config apps/sharing/host.wrangler.jsonc
cargo build --locked -p hitslop-sharing-client --example preview
bun scripts/sharing/check-preview.ts
```

The explicit deployed check creates and stops disposable fixture rooms. It qualifies
real native/browser edits, restricted-cookie private assets, late join, interrupted
file reception, process reopen, `.slop` download/native reopening and revocation.
Reports and credentials stay under ignored `.hitslop/sharing-preview/check-*`; secrets
are never included in the report. The preview example is a test driver, not a shipped
CLI sharing command. Ordinary CI uses `bun run verify` (including Worker WASM clippy);
use `bun run verify --native` when native/FFI work is complete.

Rust owns HTTP DTOs, the invitation, resource policy, binary envelope and shared limits.
`bun run schema:generate` generates TS and `openapi.json`. HTTP callers use those types;
OpenAPI describes metadata/file routes, while WebSocket updates use the separately
versioned binary protocol. There is no oRPC or second Swift HTTP client.

## Preview limits and remaining qualification

- Initial seed: 16 MiB; retained relay updates: 16 MiB. No room compaction yet.
- Immutable app and attachment limits are the core's existing acceptance limits.
- Automatic CloudKit checkpoints for relay documents are at most once a minute per
  process. Manual/background saves bypass the interval; ordinary documents are unchanged.
- No accounts, person-level permissions, read-only invitations, public catalog,
  universal links or reliable offline web reopening.
- The signed Mac app passed creation, Copy Edit Link, Keychain restoration after reopen,
  native/browser edits, Stop and continued local editing. Native Join submission, phone
  UI, cross-device Keychain delivery and CloudKit coexistence remain qualification gates.
  Historical probes do not qualify these new controls. CloudKit discovery/cursor misses
  remain open.
- No new latency claim: the previous resource probe measured 430–439 ms application
  echo RTT, including inventory/file coordination. One-way and paint latency are unknown.

See the [architecture](../../docs/architecture.md#sync),
[roadmap](../../docs/roadmap.md#later) and
[integrated preview evidence](../../docs/evidence/live-sharing-preview-2026-10-19.md).
