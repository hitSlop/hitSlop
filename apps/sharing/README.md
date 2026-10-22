# Live sharing

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

Mac and iPhone builds enable live collaboration without a launch flag. The v1 wire
contract is frozen for shipped TestFlight clients.

The host serves a static, cacheable Apple App Site Association response at
`https://share.hitslop.com/.well-known/apple-app-site-association` without a redirect.
It associates `/join` with team `78UAXU8QG8`'s `com.hitslop.app` and reserves the
`com.hitslop.app.Clip` App Clip ID. iOS and macOS entitlements include
`applinks:share.hitslop.com`; sharing-enabled apps pass the complete URL (including
its fragment) to the existing join flow. Mac Release entitlements also include the sharing
Keychain group; the Developer ID profile must authorize these capabilities. Mac builds
use Apple's web sign-in because Developer ID does not support native Sign in with Apple.

`GET /auth/apple` on the trusted host serves a static page that forwards `location.hash`
unchanged to `hitslop://auth/apple`. It requests no assets or relay resources, has its
own script-hash CSP, `Cache-Control: no-store` and `Referrer-Policy: no-referrer`.
The app requests `code id_token` with `response_mode=fragment` and no scopes, so the
credentials never reach this server. The active Mac authentication session validates
state and sends the ID token and raw nonce to Firebase. iPhone keeps native sign-in.

Operator setup: Apple Services ID `com.hitslop.app.signin` must use primary App ID
`com.hitslop` (the same group as `com.hitslop.app`), domain `share.hitslop.com` and return
URL `https://share.hitslop.com/auth/apple`. Firebase Authentication's Apple provider must
have this Services ID plus the Apple team ID, key ID and private key. This grouping
preserves the Apple subject and Firebase account across Mac and iPhone.

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
missing IPs share a fallback budget. Compaction POST has its own 6/minute budget.
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
only for a `v*` release tag or a manual run, then smoke-checks `/health` and the app-site
association, so browser guests run the same core as released apps. Roll back with
`wrangler rollback --config apps/sharing/wrangler.jsonc` (and the host config); a rollback
never undoes Durable Object migrations or stored data. `wrangler.jsonc` is the production config; its migration
transfers the Durable Object classes from the retired preview script, which exists only on the
production account. Staging, fresh accounts and local `wrangler dev` use `wrangler.fresh.jsonc`
(worker `hitslop-sharing-relay-staging`, bucket `hitslop-sharing-staging`), which creates the classes. Analytics Engine must be enabled on the account (the
dataset is created on first write). The rate-limit namespaces (`ACCOUNT_CREATE_LIMIT`
`27924743`, `GUEST_METADATA_LIMIT` `27924744`, `GUEST_SEED_LIMIT` `27924745`,
`GUEST_CONNECT_LIMIT` `27924746`, `GUEST_BLOB_LIMIT` `27924747`, `GUEST_COMPACT_LIMIT` `27924748`) need no separate provisioning.

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

Clients check this hint on connection and every ten minutes, wait a random 0–30
seconds, then re-fetch it and request an eligible export from their serial owner.
The base is Loro **updates** from the retained-history start through confirmed relay
heads, self-checked before upload; a shallow snapshot is not a usable room base.
Quota waits as `RoomFull` while clients compact confirmed history and reconnect to
retry the rejected local push. A repeated Quota with no compaction due, or a base
above 16 MiB, becomes `Outgrown` and offers a clean unshared copy/new link. HTTP 409
is a race/retry; 429, 500 and network failures back off. Attachment upload 413 is a
terminal `FilesFull` pause. Clients send text `ping` every twenty seconds, ignore
text/Pong controls, and reconnect on 1011/1013 without pausing. Typed pauses also
carry `NeedsUpdate`, `Refused`, `MissingHistory` and `Ended` recovery choices.


**Operator:** add a 14-day object expiration lifecycle rule with prefix `rooms/` to
both production and staging R2 buckets. Only compaction archives use this prefix
(`rooms/{room}/compacted/`); existing seed/resource keys begin with the room ID.
R2 lifecycle filters are prefixes, so a wildcard suffix `/compacted/` is unnecessary.
The relay also sweeps expired archives during compaction and Live alarms; Stop purges
the entire archive prefix. See [R2 object lifecycles](https://developers.cloudflare.com/r2/buckets/object-lifecycles/).

Live rooms record activity on Hello and accepted Push, at most once per hour, and
expire after 90 idle days when no socket is open. Existing Live rooms initialize this
clock on their first request after upgrade; touch existing room URLs during rollout
to start every legacy deadline. Alarms reschedule active rooms and
sweep archives. Stop and idle expiry release both the Account quota slot and its
current-room index, remove private resources and identifiers, and retain only
`phase=Ended` and `ended_at`. This tombstone returns 410, including repeated Stop
requests (treat 410 as already stopped), for 30 days, then `deleteAll`
removes it and the room returns 404. Cleanup failures retain the identifiers needed
for retry and a one-minute retry alarm. Account deletion visits outstanding quota
slots; already cleaned Ended rooms have no account identifiers and need no revisit.

Only permanent socket refusals send Error then close 1008: protocol (code 1), app
mismatch (2), ended/unknown room (3), unauthorized (4), and malformed messages (5).
Invalid capabilities are refused before upgrade. Storage and other transient message
failures close 1011 without Error, allowing shipped clients to reconnect.
Owner identity accepts any Firebase sign-in provider except `anonymous`.

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
pinned Miniflare under Node (the Bun entry point bundles and launches the driver).
Node avoids Bun undici proxy resets during sustained socket/storage qualification.
Only the local runtime's certificate fetch uses a generated test key;
production has no identity bypass. It covers idempotent creation, seed replacement,
open, guest access, second-session owner recovery, authorization, Stop/upload cleanup,
re-share, stale-room refusal, account deletion of Live/Uploading rooms, socket revocation,
cold re-sign-in with restored quota and bursts against all five guest budgets.
Compaction checks cover bounds/conflicts, partial coverage, archival and expiry,
base-first backfill, quotas and both socket budgets, transient/permanent refusals,
per-IP socket admission, idle expiry, identifier removal and tombstone purge.

The private R2 bucket is `hitslop-sharing`. The `dotslop.com` zone needs a
proxied wildcard DNS record (`AAAA * → 100::`); the host config declares its wildcard
route and `share.hitslop.com` custom domain. Deployment is a separate operator action.

The optional deployed campaign uses disposable infrastructure fixtures:

```sh
cargo build --locked -p hitslop-sharing-client --example sharing
HITSLOP_SHARING_INVITATION_FILE=/private/path/to/link bun scripts/sharing/check-live.ts
```

Create the fixture with the signed owner app and save its edit link in a private file.
The driver joins as a signed-out native guest and opens signed-out browser profiles.
When prompted, Stop in the owner app. Alternatively supply `HITSLOP_SHARING_TOKEN_FILE`
with a refreshed Firebase token in an owner-readable file. Never put secrets in arguments
or reports. The campaign checks edits, offline/reconnect, private resources, attachment
barriers, download/native reopen and revocation. Ignored reports stay under
`.hitslop/sharing/check-*`.

## Limits and follow-ups

Seeds, retained updates (excluding the base), and the full-history base each have a
16 MiB limit. Retained updates also have a 32,768-row cap. Rooms admit 32 connections. App and
attachment limits use core acceptance. Automatic CloudKit checkpoints for relay documents
run at most once a minute per process; manual/background saves bypass the interval.
Public rollout needs privacy disclosures and broader device qualification.
Reliable offline web reopening remains deferred.

`HITSLOP_SHARING_TRACE=1` logs fixed transport event names and timestamps without tokens,
capabilities, room IDs or payloads. CloudKit run, phase, token and retry diagnostics use
Unified Logging subsystem `com.hitslop.library`. An isolated Debug
`HITSLOP_LIBRARY_DIAGNOSTIC_NAMESPACE` supports diagnosis without deleting cursors.

See the [architecture](../../docs/architecture.md#relay), [roadmap](../../docs/roadmap.md#later),
and historical [account](../../docs/evidence/live-sharing-accounts-2026-10-19.md),
[device](../../docs/evidence/live-sharing-phone-2026-10-19.md) and
[native/browser](../../docs/evidence/authoring-environment-2026-10-19.md) evidence.

The production Worker names are `hitslop-sharing-relay` and `hitslop-sharing-host`.
The `sharing-rename-v1` migration transfers the existing room and account namespaces;
keep this applied migration in the configuration. Its source Worker names are historical.
The public domains and v1 sharing contract are unchanged.
