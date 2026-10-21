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
- `DELETE /account` with Firebase identity ends every indexed room through Stop,
  closes guest sockets, purges R2, then clears the account index and quota. It returns
  JSON `null`; retries (including after partial cleanup) succeed idempotently. A later
  sign-in can create rooms again. This removes relay data; Firebase/Apple identity
  revocation remains a separate native account flow.
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
in Debug. A build compiled with the `HITSLOP_SHARING_PREVIEW` condition (TestFlight preview
builds) enables it without a flag. Normal releases omit the feature. The v1 wire contract is frozen for shipped TestFlight clients.

The host serves a static, cacheable Apple App Site Association response at
`https://share.hitslop.com/.well-known/apple-app-site-association` without a redirect.
It associates `/join` with team `78UAXU8QG8`'s `com.hitslop.app` and reserves the
`com.hitslop.app.Clip` App Clip ID. iOS and Debug macOS entitlements include
`applinks:share.hitslop.com`; preview-enabled apps pass the complete URL (including
its fragment) to the existing join flow. Ordinary Mac Release entitlements are unchanged.

## Protocol compatibility

The fixtures in `crates/hitslop-sharing-protocol/tests/fixtures/v1` are immutable once
committed. Add examples; never replace or regenerate existing ones. The protocol grows
only through optional fields and new routes. Optional fields must remain readable by
shipped clients: older strict resource readers still need the original inventory shape,
so publish new resource metadata through a new route. A breaking change requires a new path
version (`/v2`); `/v1` must keep serving old clients, including its socket subprotocol
and binary codecs. Do not reorder postcard variants or change existing frame layouts.
JSON reply readers ignore unknown fields. Unknown phase/control/event enum values and
binary frame/permission/ack variants fail closed; API failure codes are opaque strings,
and binary Error codes are opaque bytes. There is no fallback phase or implicit success.
The fixture README describes canonical JSON and both binary representations.

## Abuse controls and metrics

Guest entry points have independent per-client-IP (`CF-Connecting-IP`) Workers budgets
across rooms: metadata GET 120/minute, seed GET 30/minute, sync GET 60/minute and blob
PUT 120/minute. These entry point limits also apply to owner requests on those routes;
missing IPs share a fallback budget. Creation retains its existing per-account 5/minute
limit. All 429 replies use `ApiFailure` code `limited`. Workers limits are local to each
Cloudflare location and approximate, not global quotas ([Workers rate limits](https://developers.cloudflare.com/workers/runtime-apis/bindings/rate-limit/)).

workers-rs 0.8.7 writes directly to Analytics Engine binding `METRICS`, dataset
`hitslop_sharing`. `blob1` contains only `room_created`, `room_opened`, `room_ended_stop`,
`room_ended_alarm`, `account_deleted`, `guest_join` (accepted socket) or `limited`;
`double1` is 1. No identifiers, IPs, hashes, capabilities or tokens are emitted. Lifecycle
retries do not emit another transition; metrics are best effort, not an audit ledger.

Deployment follows master: `.github/workflows/sharing.yml` builds the relay and host, runs this
qualification, deploys both with the repository's Cloudflare credentials and smoke-checks
`/health` and the app-site association. Analytics Engine must be enabled on the account (the
dataset is created on first write). The rate-limit namespaces (`ACCOUNT_CREATE_LIMIT`
`27924743`, `GUEST_METADATA_LIMIT` `27924744`, `GUEST_SEED_LIMIT` `27924745`,
`GUEST_CONNECT_LIMIT` `27924746`, `GUEST_BLOB_LIMIT` `27924747`) need no separate provisioning.

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
re-share, stale-room refusal, account deletion of Live/Uploading rooms, socket revocation,
cold re-sign-in with restored quota and bursts against all four guest budgets.

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
Public rollout needs privacy disclosures and broader device qualification.
Reliable offline web reopening and room compaction remain deferred.

`HITSLOP_SHARING_TRACE=1` logs fixed transport event names and timestamps without tokens,
capabilities, room IDs or payloads. `HITSLOP_LIBRARY_TRACE=1` and an isolated
`HITSLOP_LIBRARY_DIAGNOSTIC_NAMESPACE` support CloudKit diagnosis without deleting cursors.

See the [architecture](../../docs/architecture.md#relay), [roadmap](../../docs/roadmap.md#later),
and historical [account](../../docs/evidence/live-sharing-accounts-2026-10-19.md),
[device](../../docs/evidence/live-sharing-phone-2026-10-19.md) and
[native/browser](../../docs/evidence/authoring-environment-2026-10-19.md) evidence.
