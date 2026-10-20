# Live sharing developer preview

The Rust Worker is a Loro-free byte relay with private R2 resources. Contracts and limits
live in `crates/hitslop-sharing-protocol`; native transport lives in
`crates/hitslop-sharing-client`. Rust generates TypeScript and `openapi.json` through
`bun run schema:generate`. The released `.slop` format and binary sync protocol are unchanged.

Each Room owns one phase: **Uploading → Live → Ended**. Its SQLite meta stores the owner
UID, document UUID, plaintext capability, app digest and seed hash/size; other tables hold
opaque update records and resource inventory. Only the verified owner can recover its
capability. The Account object stores only the current room per document and non-Ended
room membership for the ten-room quota. It holds no credentials or lifecycle phases.

## Lifecycle

All HTTP routes use `/v1/`:

- `POST /rooms` with Firebase identity and `{document}` returns `{room, capability, phase}`.
  Retrying returns the existing Uploading/Live room. New rooms require `CREATION_ENABLED`,
  per-UID rate limits and available quota.
- During Uploading, the owner uploads `PUT /rooms/{room}/seed` with
  `X-Sharing-SHA256`, `X-Sharing-App` and `X-Sharing-Version` headers; the server checks
  the hash and stores seed metadata. Seed and `POST /rooms/{room}/resources` manifest
  can be replaced. App bytes use `PUT /rooms/{room}/asset/{hash}`; attachments use
  `PUT /rooms/{room}/blob/{hash}`.
- `POST /rooms/{room}/open` verifies the seed, manifest and all app assets in R2 and
  makes the room Live. Seed and app resources then become immutable.
- `GET /rooms/{room}` returns document, phase, seed hash/size and app digest. Firebase
  owner authentication also returns the capability. Guests use a capability bearer.
  `GET /account/rooms/{document}` returns `{room}` or null for owner recovery.
- `DELETE /rooms/{room}` by the owner ends sharing, closes sockets, purges R2 and tables,
  and releases the account quota slot. Uploads crossing Stop delete late writes.
  Uploading rooms expire through a 24-hour alarm. Re-share creates a fresh random room;
  stale rooms return 410.

Guest seed/resource/blob reads, blob writes and `/sync` require Live plus the capability.
WebSockets use the capability subprotocol; hosted app resources use a short-lived,
host-scoped partitioned HttpOnly cookie. An edit link is
`https://share.hitslop.com/join#<room>.<capability>`; Join reads seed metadata from the room.

Native creation backs up the current owner to a temporary file and retries uploads;
there is no stable seed journal or original-device requirement. Share preserves UUID,
path, owner and full history. Join reuses a matching file or installs a canonical copy.
Credentials contain only `{link, ended}` and stay device-local, outside documents and
CloudKit. Sign-out preserves existing capability sync and cached links; creation,
owner recovery and Stop require Firebase identity. The account panel verifies its session
through account lookup and displays a short UID hash.

`share.hitslop.com` hosts trusted workers and storage. Authored UI runs on separate
`<copy UUID>.dotslop.com` origins. One browser host serves local resources through Service
Worker channels and hosted resources through network requests and cookies. Authored code
never receives the link. OPFS holds SQLite and IndexedDB holds the copy catalog.
Connected opening is required; open copies remain editable offline and after Stop.

The native preview requires `--live-sharing-preview` or `HITSLOP_LIVE_SHARING_PREVIEW=1`
in Debug; Release also requires the `HITSLOP_SHARING_PREVIEW` compilation condition.
Normal releases omit the feature. Old preview rooms and credentials are unsupported.

## Build and qualification

Use the normal workspace Rust, wasm-bindgen, LLVM and Binaryen setup. The sharing build
installs `worker-build 0.8.7` into `generated/core-tools` when missing, or set
`HITSLOP_WORKER_BUILD` to that version:

```sh
bun scripts/build/sharing.ts
wrangler deploy --dry-run --config apps/sharing/wrangler.jsonc
wrangler deploy --dry-run --config apps/sharing/host.wrangler.jsonc
bun run verify sharing
bun run verify
bun run verify --native
```

`verify sharing` runs the real Rust WASM relay with SQLite Durable Objects and R2 in
pinned Miniflare. Only the local runtime's certificate fetch uses a generated test key;
production has no identity bypass. It covers idempotent creation, seed replacement,
open, guest access, second-session owner recovery, authorization, Stop/upload cleanup,
re-share and stale-room refusal.

The private R2 bucket is `hitslop-sharing-preview`. The `dotslop.com` zone needs a
proxied wildcard DNS record (`AAAA * → 100::`); the host config declares its wildcard
route and `share.hitslop.com` custom domain. Deployment is a separate operator action.

The optional deployed campaign uses disposable infrastructure fixtures:

```sh
cargo build --locked -p hitslop-sharing-client --example preview
HITSLOP_SHARING_INVITATION_FILE=/private/path/to/link bun scripts/sharing/check-preview.ts
```

Create the fixture with the signed owner app and save its edit link in a private file.
The driver joins as a signed-out native guest and opens signed-out browser profiles.
When prompted, Stop in the owner app. Alternatively supply `HITSLOP_SHARING_TOKEN_FILE`
with a refreshed Firebase token in an owner-readable file. Never put secrets in arguments
or reports. The campaign checks edits, offline/reconnect, private resources, attachment
barriers, download/native reopen and revocation. Ignored reports stay under
`.hitslop/sharing-preview/check-*`.

## Limits and follow-ups

Seeds and retained updates each have a 16 MiB limit; rooms admit 32 connections. App and
attachment limits use core acceptance. Automatic CloudKit checkpoints for relay documents
run at most once a minute per process; manual/background saves bypass the interval.
Public rollout needs Save Unshared Copy, account deletion/Apple revocation, privacy,
remaining abuse controls, broader device qualification and frozen protocol fixtures.
Reliable offline web reopening and room compaction remain deferred.

`HITSLOP_SHARING_TRACE=1` logs fixed transport event names and timestamps without tokens,
capabilities, room IDs or payloads. `HITSLOP_LIBRARY_TRACE=1` and an isolated
`HITSLOP_LIBRARY_DIAGNOSTIC_NAMESPACE` support CloudKit diagnosis without deleting cursors.

See the [architecture](../../docs/architecture.md#relay), [roadmap](../../docs/roadmap.md#later),
and historical [account](../../docs/evidence/live-sharing-accounts-2026-10-19.md),
[device](../../docs/evidence/live-sharing-phone-2026-10-19.md) and
[native/browser](../../docs/evidence/authoring-environment-2026-10-19.md) evidence.
