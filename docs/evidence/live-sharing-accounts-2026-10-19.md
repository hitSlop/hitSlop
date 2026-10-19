# Account-backed sharing lifecycle — 2026-10-19

This slice replaces preview ownership behind the existing, disabled-by-default native
developer flag. It follows the [identity/attachment foundation](live-sharing-accounts-foundation-2026-10-19.md).
It does not qualify a public beta or physical Mac/iPhone recovery.

## Implementation

- A SQLite Account Durable Object per verified Firebase UID reserves one room per
  document/generation, counts pending/stopping rooms toward the 10-room limit, and
  privately retains the plaintext edit capability for owner recovery. Stop clears
  that capability and the invitation before remote cleanup; failed cleanup is retryable.
- The Room DO independently verifies Firebase identity and its immutable owner UID.
  It checks the account reservation at init and requires stopping state before
  tombstoning even a room that never initialized. No forwarded UID assertion is trusted.
- Immutable seed identity wins at init. Publication waits for the seed and every
  declared app asset in private R2. Account finalization reloads current state after
  every remote await, so late publication cannot resurrect a stopped/replaced room.
- A stopped generation cannot be reused. Re-share explicitly increments it and
  reserves a fresh room/capability. Stale admin requests cannot target the replacement.
  An owner holding the ended predecessor's CloudKit checkpoint can recover or cancel
  its pending replacement; unrelated room/endpoint membership is refused.
- Native Rust/UniFFI create/recover/stop use transient Firebase ID tokens; Swift holds
  no Loro bytes or wire metadata. The immutable, room-named SQLite seed is the local
  retry journal, installed by the core's existing durable, non-replacing Backup
  operation in a private directory before HTTP and removed after publication.
- Credentials contain only invitation + ended. The admin key, preview creation key,
  `.staging` marker, synchronizable room credentials and two-second credential polling
  are removed. Device-local invitations let signed-out guests/creators keep syncing;
  create, owner recovery and Stop require sign-in. Transport callbacks report status;
  recovery runs on open, foreground, account change and opening sharing controls.
  Initial seed catch-up runs inside the Rust reconnect supervisor so an offline
  open can recover without a UI Retry; a local HTTP-service regression checks that
  the existing owner receives the seed state after the service comes back.
- HTTP metadata/file/socket routes use `/v1/`. Rust generates the new TypeScript and
  OpenAPI contracts. The existing `hitslop.sync.v1` WebSocket envelope is unchanged.
- The Mac sidebar no longer carries Join/New Synced Checklist buttons. File menu
  commands expose them; Sharing Account stays in the app menu and sharing sheet.
- Operator controls include the creation kill switch, 5 creates/minute/UID and
  20 reserve requests/minute/IP; rooms admit 32 connections. Existing document/seed/
  attachment acceptance and the Loro pin/released corpora are unchanged.

## Qualification and deployment

`bun run verify sharing` passed against the built production Rust WASM module in
pinned Miniflare with actual SQLite DO and R2 storage. The local runtime alone serves
a generated public signing key at the fixed Google certificate URL. Production has no
test key, identity bypass or configurable certificate URL. Checks cover:

- Two signed test identities with separate registries, no access to each other's rooms.
- Concurrent reservations returning one stable room/capability; immutable init retries
  and a conflicting seed refused.
- Partial publication refused until the seed, manifest and every app file exist.
- Guest capabilities refused on creator initialization, seed/asset administration
  and Stop; another verified UID also refused.
- Owner invitation recovery after disposing and recreating the entire local runtime
  over persisted SQLite/R2.
- Stop overlapping an unfinished upload, late write refused, and zero retained R2
  objects afterward; late publication, stale-generation Stop and delayed init refused.

The first runtime run exposed mutation of immutable headers on a forwarded DO error
response, returning 500 instead of the intended 409. Cloning response headers before
setting no-store fixed it; the unchanged partial-publication check then passed.
The local seed test caught a temporary filename lacking `.slop`; the core refused it.
The stable journal path preserves the required extension and reuses the core's atomic
Backup publication instead of adding manual temporary-file/hard-link handling. The
restart/edited retry test passes. No document acceptance rule was relaxed.

Final relay deployment: `b66268b4-f333-4b9b-b9b1-831bf5ca2401`; host deployment:
`8b65fc1b-e547-40aa-8190-6adf5e72d51c`. Both dry runs and binding generation pass.
Deployed anonymous `/v1/account/access/...` and auth probe return 401 with no-store;
old unversioned room URLs return 404. The obsolete deployed `PREVIEW_CREATE_KEY`
secret was deleted. Worker startup timing is not edit/paint latency.

The signed Mac target builds and passes deep/strict codesign verification. The arm64
iOS simulator app also compiles; no simulator or phone was launched. The Mac catalog
sidebar has no developer buttons; its File menu exposes New Synced Checklist and Join
Shared Slop. No real account-backed room was created through this UI in this pass.
All temporary app copies were quit through their normal Quit menus after review;
only the user's installed `/Applications/hitSlop.app` was left running.

The offline catch-up regression was also run with its bootstrap fix removed: it fails
at the intended assertion that opening must start seed catch-up. Restoring the fix
passes and updates the existing owner without a UI Retry. The full portable run hit
the pre-existing three-second `explicit_date_constructors_keep_their_arguments`
watchdog during concurrent template builds. A focused repeat passes unchanged after
build/app cleanup. Deadlines and runner code were not changed.

The pending replacement cancellation regression likewise failed against the old
membership rule: recovery accepted the ended predecessor, but Stop rejected it.
Creation, recovery and cancellation now share the same reservation membership rule.

The final `NEXTEST_TEST_THREADS=1 bun run verify --native` passes in 854.6 seconds:
434 Rust tests, 234 Swift integration tests, 161 SDK/shell tests, 102 CLI tests,
six render checks, package/landing checks and 32 native checks including replay of
both frozen 1.0.0/1.0.1 corpora. The date constructor test passes in the full run at
1.759 seconds under the unchanged three-second budget. The runner certifies the
default run, reusing unchanged passing type/tooling checks. Report and logs are retained
under `.hitslop/evidence/runs/2026-10-19T20-39-18-920Z-23450/`.
Final process inspection confirms only `/Applications/hitSlop.app` remains running.

## Remaining gates

- Qualify the updated account-backed signed Mac UI and deployed native/browser
  fixture campaign with an actual refreshed Firebase session; earlier preview UI
  success does not certify this replacement.
- Physical Mac/iPhone same UID, either-order CloudKit/access arrival, guest link on
  each device, owner recovery, native Join, background/kill/relaunch and offline edits.
  The phone was not used in this pass.
- Diagnose CloudKit discovery/post-Stop checkpoint delivery without resetting normal
  library cursors. Old unversioned preview rooms/resources still require operator
  retirement; local documents are retained and old ownership secrets are not migrated.
- Save Unshared Copy from accepted in-memory state when source history cannot save;
  account deletion including Apple revocation, remaining frame/resource throttles,
  privacy labels/policy, secret-free operational metrics and network fixture freeze.
- Automatic CI execution of the explicit Cloudflare runtime qualification tier.

Browser scope remains connected Chrome opening/reopening; reliable offline reopening,
permissions beyond edit links, presence, lazy attachments and room compaction stay deferred.
