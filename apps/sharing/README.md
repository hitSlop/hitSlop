# Live sharing

The Rust Worker is a Loro-free byte relay with private R2 resources. Contracts and limits
live in `crates/pop-sharing-protocol`; native transport lives in
`crates/pop-sharing-client`. Rust generates TypeScript and `openapi.json` through
`bun run schema:generate`. Released document layouts and frozen v1 wire vectors remain
supported. New epoch and handoff controls are additive; they do not rewrite those fixtures.

The [sharing lifecycle specification](../../docs/reference/sharing-lifecycle.md) defines
catalog formats and revision ordering, single byte authority, Preparing payload reconciliation,
checkpoint publication, recovery, and creator finalization. This README supplies the relay
routes, operational configuration, and qualification procedure.

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
  makes the room Live. Ordinary seed/app replacement then stops; an explicit epoch CAS
  can publish a new immutable seed and its accepted inventory.
- `GET /rooms/{room}` returns document, phase, seed hash/size and app digest. Firebase
  owner authentication also returns the capability. Guests use a capability bearer.
  `GET /account/rooms/{document}` returns `{room}` or null for owner recovery.
- `DELETE /account` with Firebase identity ends every indexed room through Stop,
  closes guest sockets, purges R2, then clears the account index and quota. It returns
  JSON `null`; retries (including after partial cleanup) succeed idempotently. A later
  sign-in can create rooms again. This removes relay data; Firebase/Apple identity
  revocation remains a separate native account flow.
- Legacy `DELETE /rooms/{room}` by the owner ends an ordinary room, closes sockets,
  purges R2 and tables, and releases the account quota slot. New clients use the durable
  handoff below for Live rooms. Generic DELETE refuses a frozen handoff. Uploads crossing
  Stop delete late writes.
  Uploading rooms expire through a 24-hour alarm. Re-share creates a fresh random room;
  stale rooms return 410.

Guest seed/resource/blob reads, blob writes and `/sync` require Live plus the capability.
WebSockets use the capability subprotocol; hosted app resources use a short-lived,
host-scoped partitioned HttpOnly cookie. An edit link is
`https://share.dotpop.click/join#<room>.<capability>`; Join reads seed metadata from the room.

Native creation prepares an accepted owner-derived seed and retries uploads. Managed
creation first records Preparing with its retained private CloudKit payload. Same-room
resume adopts the committed nonce and reconciles that payload before activation; Relay
catalog discovery remains metadata-only. There is no original-device requirement.

Share preserves the application document UUID and path. Layout-2 maintenance may replace
physical CRDT history through explicit, conditional owner adoption; it does not promise
unchanged undo across a cutoff. Device-local receipt journals and recoveries survive lost
acknowledgements. Credentials contain only `{link, ended}` and stay outside documents and
CloudKit. Signing out does not revoke a distributed capability; creator recovery, reservation
and Stop still require verified identity. The account panel verifies its session through
account lookup and displays a short UID hash.

`share.dotpop.click` hosts trusted workers and storage. Authored UI runs on separate
`<copy UUID>.dotslop.com` origins. One browser host serves local resources through Service
Worker channels and hosted resources through network requests and cookies. Authored code
never receives the link. OPFS holds SQLite and IndexedDB holds the copy catalog.
Connected opening is required; open copies remain editable offline and after Stop.

Mac and iPhone build configurations enable live collaboration. Frozen v1 contracts remain
acceptance inputs; local qualification does not establish deployment or signed-device release
qualification for the new lifecycle.

Sharing links have a generic Messages preview, with the current dotpop icon and
an invitation title served directly in HTML. The preview exposes no document title,
contents or capability; the capability remains in the URL fragment. It does not
change the installed app's Universal Links handling.

The host serves a static, cacheable Apple App Site Association response at
`https://share.dotpop.click/.well-known/apple-app-site-association` without a redirect.
It associates `/join` with team `78UAXU8QG8`'s `com.hitslop.app` and reserves the
`com.hitslop.app.Clip` App Clip ID. iOS and macOS entitlements include
`applinks:share.dotpop.click`; sharing-enabled apps pass the complete URL (including
its fragment) to the existing join flow. Mac Release entitlements also include the sharing
Keychain group; the Developer ID profile must authorize these capabilities. Mac builds
use Apple's web sign-in because Developer ID does not support native Sign in with Apple.

`GET /auth/apple` on the trusted host serves a static page that forwards `location.hash`
unchanged to `dotpop://auth/apple`. It requests no assets or relay resources, has its
own script-hash CSP, `Cache-Control: no-store` and `Referrer-Policy: no-referrer`.
The app requests `code id_token` with `response_mode=fragment` and no scopes, so the
credentials never reach this server. The active Mac authentication session validates
state and sends the ID token and raw nonce to Firebase. iPhone keeps native sign-in.

Operator setup: Apple Services ID `com.hitslop.app.signin` must use primary App ID
`com.hitslop` (the same group as `com.hitslop.app`), domain `share.dotpop.click` and return
URL `https://share.dotpop.click/auth/apple`. Firebase Authentication's Apple provider must
have this Services ID plus the Apple team ID, key ID and private key. This grouping
preserves the Apple subject and Firebase account across Mac and iPhone.

## Protocol compatibility

The fixtures in `crates/pop-sharing-protocol/tests/fixtures/v1` are immutable once
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
missing IPs share a fallback budget. Compaction POST, epoch publication, and seed-stage
PUT/DELETE share a 6/minute maintenance budget. Staged attachment PUT uses the blob budget.
Each room admits at most eight sockets per IP. Each socket admits 8 MiB of Push wire
bytes and 1,800 Push messages per minute, including retries; exceeding either closes
with 1013 before storing the push. Creation retains its existing per-account 5/minute
limit. All 429 replies use `ApiFailure` code `limited`. Workers limits are local to each
Cloudflare location and approximate, not global quotas ([Workers rate limits](https://developers.cloudflare.com/workers/runtime-apis/bindings/rate-limit/)).

workers-rs 0.8.7 writes directly to Analytics Engine binding `METRICS`, dataset
`hitslop_sharing`. `blob1` contains only `room_created`, `room_opened`, `room_ended_stop`,
`room_ended_alarm`, `room_ended_idle`, `account_deleted`, `guest_join` (accepted socket) or `limited`;
`double1` is 1. No identifiers, IPs, hashes, capabilities or tokens are emitted. Lifecycle
retries do not emit another transition; metrics are best effort, not an audit ledger.

`.github/workflows/sharing.yml` builds the relay and host and runs this qualification on
every matching `master` change. It deploys both with the repository's Cloudflare credentials
only for a `pop-v*` release tag or a manual run when `POP_DEPLOY_ENABLED=true`, then
smoke-checks `/health` and the app-site association, so browser guests run the same core
as released apps. Its job summary distinguishes qualification-only runs, disabled
deployments and completed deployments; a green qualification does not prove deployment.
Roll back with
`wrangler rollback --config apps/sharing/wrangler.jsonc` (and the host config); a rollback
never undoes Durable Object migrations or stored data. `wrangler.jsonc` is the production config; its migration
transfers the Durable Object classes from the retired preview script, which exists only on the
production account. Staging, fresh accounts and local `wrangler dev` use `wrangler.fresh.jsonc`
(worker `hitslop-sharing-relay-staging`, bucket `hitslop-sharing-staging`), which creates the classes. Analytics Engine must be enabled on the account (the
dataset is created on first write). The rate-limit namespaces (`ACCOUNT_CREATE_LIMIT`
`27924743`, `GUEST_METADATA_LIMIT` `27924744`, `GUEST_SEED_LIMIT` `27924745`,
`GUEST_CONNECT_LIMIT` `27924746`, `GUEST_BLOB_LIMIT` `27924747`, `GUEST_COMPACT_LIMIT` `27924748`) need no separate provisioning.

## Checkpoint epochs and stopping a live link

The client owns document acceptance. The relay never opens SQLite or Loro; it checks
opaque seed hashes, resource bounds and conditional version metadata.

- `GET /v1/rooms/{room}/epoch` returns `EpochState`: current seed hash, canonical
  root/heads and optional creator handoff. Peer tuples use the existing wire encoding.
- `PUT /v1/rooms/{room}/epochs/{sha256}` stages immutable seed bytes. At most two
  unpublished seeds fit in a room; each expires after ten minutes. Cleanup is registered
  before R2 upload and runs through the room alarm. Staging never changes bootstrap.
  `DELETE .../epochs/{sha256}` releases an unused candidate immediately, including its
  unclaimed blobs; missing stages are idempotent and the current seed is refused with
  409. Clients release candidates in cleanup even after a lost publication response.
- `PUT .../epochs/{sha256}/blobs/{hash}` uploads referenced candidate attachments
  under that seed's lease. Each stage permits at most 16 MiB, separate from the current
  room's attachment budget. Staged blobs stay outside the visible inventory until the
  epoch CAS succeeds. Expiry reclaims a blob only when neither the current epoch nor
  another unexpired stage references it.
- `POST /v1/rooms/{room}/epoch` accepts `PublishEpoch`. An exact expected seed hash
  **and heads** comparison atomically switches the seed pointer, root, heads and resource
  inventories, discarding the prior record/base set. A concurrent push makes it conflict.
  A lost receipt can be retried by candidate hash/root. The referenced app and blobs must
  already exist with the declared lengths. Obsolete seed/blob cleanup is durable and
  retryable; a completed epoch removes old compaction archives too.
- `GET .../seed?hash={expected}` refuses a raced epoch with 409. Old sockets are fenced
  and receive error 6 (`ERROR_EPOCH`); a reconnecting Hello must include the current root.
  Legacy rooms retain their existing Hello acceptance until an epoch is published.

Canceling preparation uses creator-authorized `DELETE .../rooms/{room}?phase=uploading`.
The room checks that condition after authorization and marks Ended before yielding;
an already activated room returns 409. Clients cancel the relay reservation before
restoring the retained CloudKit snapshot, so an interrupted restore stays retryable.

Stopping a managed live link is a creator-authorized handoff, not an upload racing an
immediate DELETE. `POST .../handoff/begin` takes `BeginHandoff` (client-generated 32-hex
idempotency token and exact expected epoch frontier). It freezes writes and immediately
revokes guest sockets, capabilities and resource cookies with the existing Ended refusal.
Guests keep their existing local copy; there is no final-download window.

The creator can still recover the room from the account index and use an identity bearer
for seed, resource and `GET .../handoff/history` reads. History pages are plain binary
`Frame::Records`, at most 32 records and a 1 MiB target (one larger base is allowed).
The optional `after` query is the previous `X-Sharing-Next` sequence; `X-Sharing-Done`
is `0` or `1`. The frozen frontier stays fixed across pages. Once the client confirms its
final snapshot at the durable destination, `POST .../handoff/finalize` with the same token
ends and purges the room. Retrying an already ended finalization receives 410.

A frozen handoff does not idle-expire: it holds one bounded, non-growing final state until
completion. Ordinary DELETE refuses a frozen handoff; explicit account deletion is
separately authorized to finalize/discard it. Existing DELETE and `/compact` behavior
remain available for ordinary rooms. `/compact` does not change the checkpoint epoch.

## Compaction and retention

`RoomInfo.compact` is present and true when retained update bytes reach 4 MiB or
update rows reach 8,192; it is omitted otherwise. A capability writer may then send
`POST /v1/rooms/{room}/compact` with an opaque full-history Loro blob as the binary body
(maximum 16 MiB). This is a full-history export, never a history-trimmed snapshot.
The relay checks metadata and hashes; it does not inspect Loro bytes.

Required headers are `Authorization: Bearer <capability>`, `X-Sharing-SHA256` (lowercase
64-digit hex body digest), and `X-Sharing-Version` (F). Versions use the seed PUT
encoding: a JSON array of `["0000000000000001", 42]` pairs with unique 16-digit hex peers
and non-negative i32 counters. `X-Sharing-Root` has the same encoding and is required
only for older rooms without a stored seed version; otherwise the stored root wins.
F must cover the root and previous base, advance at least one peer beyond the previous
base (or root for the first base), and never exceed current heads.

Success is **204 with no body**. Refusals use `ApiFailure`: 400 for malformed metadata
or a digest mismatch, 403 for invalid capability, 404 for unknown room, 409 for
Uploading/not due/nonmonotonic/out-of-range F, 410 for Ended, 413 for oversized body,
and 429 for the entry budget. Storage/R2 failures return 500 and may be retried.

The relay archives covered records and the previous base before one transactional
swap. Archives are `rooms/{room}/compacted/{n}` in private R2; the counter increases
and may have gaps. Each entry is a big-endian u32 byte length followed by a v1 binary
`Frame::Records` containing one record, preserving its spans and opaque bytes. Objects
carry an `expires` Unix-millisecond custom metadata value. Failed swaps can leave an
archive, which expires normally. The swap deletes only records whose every span ends
at or before F, stores one chunked base at seq 0 with nonempty root-to-F spans, and
leaves heads unchanged. Normal AUTOINCREMENT records remain positive. Hello sends the
base first to clients behind F; connected sockets never receive a base broadcast.
The v1 binary protocol and acknowledgements are unchanged.

Clients check maintenance on connection and every ten minutes, wait a random 0–30
seconds, then reread current metadata. Layout 1 uses the full-history base path described
above: Loro **updates** through the confirmed frontier, not a shallow snapshot. Layout 2
uses a fresh accepted-state epoch with exact seed-hash/heads CAS. The serial owner checks
frontier coverage; the relay remains opaque.

`RoomFull` and eligible layout-2 `FilesFull` pauses can request maintenance and reconnect.
The FilesFull cycle gets one fresh attempt; a confirmed successful sync permits another
independent episode. A visible candidate above 16 MiB remains Outgrown rather than losing
content. Legacy FilesFull remains paused. HTTP 409 is a race/retry; 429, 500 and network
failures back off. Clients send text `ping` every twenty seconds, ignore text/Pong controls,
and reconnect on 1011/1013. Typed pauses also carry `NeedsUpdate`, `Refused`, `MissingHistory`
and `Ended` recovery choices. See the shared protocol maintenance/failure classifiers and
[recovery specification](../../docs/reference/sharing-lifecycle.md#refresh-and-local-recovery).

**Operator:** add a 14-day object expiration lifecycle rule with prefix `rooms/` to
both production and staging R2 buckets. Only compaction archives use this prefix
(`rooms/{room}/compacted/`); existing seed/resource keys begin with the room ID.
R2 lifecycle filters are prefixes, so a wildcard suffix `/compacted/` is unnecessary.
The relay also sweeps expired archives during compaction and Live alarms; Stop purges
the entire archive prefix. See [R2 object lifecycles](https://developers.cloudflare.com/r2/buckets/object-lifecycles/).

Live rooms record activity on Hello and accepted Push, at most once per hour, and
expire after 90 idle days when no socket is open. Alarms reschedule active rooms and
sweep archives. Stop and idle expiry release both the Account quota slot and its
current-room index, remove private resources and identifiers, and retain only
`phase=Ended` and `ended_at`. This tombstone returns 410, including repeated Stop
requests (treat 410 as already stopped), for 30 days, then `deleteAll`
removes it and the room returns 404. Cleanup failures retain the identifiers needed
for retry and a one-minute retry alarm. Account deletion visits outstanding quota
slots; already cleaned Ended rooms have no account identifiers and need no revisit.

Only permanent socket refusals send Error then close 1008: protocol (code 1), app
mismatch (2), ended/unknown room (3), unauthorized (4), and malformed messages (5).
The additive epoch error (6) fences an old connection and requests checkpoint recovery;
it does not authorize merging the old epoch into the new one. Invalid capabilities are refused before upgrade. Storage and other transient message
failures close 1011 without Error, allowing shipped clients to reconnect.
Owner identity accepts any Firebase sign-in provider except `anonymous`; clients offer
only Sign in with Apple.

## Build and qualification

Use the normal workspace Rust, wasm-bindgen, LLVM and Binaryen setup. The sharing build
installs `worker-build 0.8.7` into `generated/core-tools` when missing, or set
`POP_WORKER_BUILD` to that version:

```sh
bun scripts/build/sharing.ts
wrangler deploy --dry-run --config apps/sharing/wrangler.jsonc
wrangler deploy --dry-run --config apps/sharing/host.wrangler.jsonc
bun run verify sharing
bun run verify
bun run verify --native
```

`verify sharing` runs the real Rust WASM relay with SQLite Durable Objects and R2 in
pinned Miniflare under Node (the Bun entry point bundles and launches the driver).
Node avoids Bun undici proxy resets during sustained socket/storage qualification.
Only the local runtime's certificate fetch uses a generated test key;
production has no identity bypass. It covers idempotent creation, seed replacement,
open, guest access, second-session owner recovery, authorization, Stop/upload cleanup,
re-share, stale-room refusal, account deletion of Live/Uploading rooms, socket revocation,
cold re-sign-in with restored quota and bursts against the guest budgets. Epoch checks
cover exact-frontier publication, old-socket fencing, staged blob quotas/expiry/release,
Uploading-only cancellation, creator-only frozen backfill and finalization. Compaction checks cover bounds/conflicts, partial coverage, archival and expiry,
base-first backfill, quotas and both socket budgets, transient/permanent refusals,
per-IP socket admission, idle expiry, identifier removal and tombstone purge.

The private R2 bucket is `hitslop-sharing`. The `dotslop.com` zone needs a
proxied wildcard DNS record (`AAAA * → 100::`); the host config declares its wildcard
route and `share.dotpop.click` custom domain. Deployment is a separate operator action.

The optional deployed campaign uses disposable infrastructure fixtures:

```sh
cargo build --locked -p pop-sharing-client --example sharing
POP_SHARING_INVITATION_FILE=/private/path/to/link bun scripts/sharing/check-live.ts
```

Create the fixture with the signed owner app and save its edit link in a private file.
The driver joins as a signed-out native guest and opens signed-out browser profiles.
When prompted, Stop in the owner app. Alternatively supply `POP_SHARING_TOKEN_FILE`
with a refreshed Firebase token in an owner-readable file. Never put secrets in arguments
or reports. The campaign checks edits, offline/reconnect, private resources, attachment
barriers, download/native reopen and revocation. Ignored reports stay under
`.pop/sharing/check-*`.

## Limits and follow-ups

Seeds, retained updates (excluding the base), and the full-history base each have a
16 MiB limit. Retained updates also have a 32,768-row cap. Rooms admit 32 connections. App and
attachment limits use shared acceptance constants. While Relay owns a managed document,
CloudKit carries catalog/discovery metadata rather than mirrored shared snapshots. Private
checkpoint candidates and handoff snapshots use conditional catalog publication, with dirty
recovery before canonical replacement. A guest that observes Ended keeps an unshared local
copy with the same UUID. Reliable fully offline hosted-browser reopening remains deferred.

`POP_SHARING_TRACE=1` logs fixed transport event names and timestamps without tokens,
capabilities, room IDs or payloads. CloudKit run, phase, token and retry diagnostics use
Unified Logging subsystem `com.hitslop.library`. An isolated Debug
`POP_LIBRARY_DIAGNOSTIC_NAMESPACE` supports diagnosis without deleting cursors.

See the [architecture](../../docs/architecture.md#relay), [roadmap](../../docs/roadmap.md#later),
and historical [account](../../docs/evidence/live-sharing-accounts-2026-10-19.md),
[device](../../docs/evidence/live-sharing-phone-2026-10-19.md) and
[native/browser](../../docs/evidence/authoring-environment-2026-10-19.md) evidence.

The production Worker names are `hitslop-sharing-relay` and `hitslop-sharing-host`.
The `sharing-rename-v1` migration transfers the existing room and account namespaces;
keep this applied migration in the configuration. Its source Worker names are historical.
The public domains and frozen v1 frame layouts are unchanged; epoch and handoff controls
are additive routes and metadata.
