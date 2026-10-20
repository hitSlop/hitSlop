# Signed Mac/iPhone live-sharing spike — 2026-10-18

## Result

The main signed-device campaign passed. The current Rust owner ran on a physical iPhone
and this Mac, connected over native Rust TLS sockets to the actual Rust Cloudflare Worker,
and converged while CloudKit was paused. Shared Keychain delivery, seed merge into an
existing document, real checkpoint throttling and phone background/kill/relaunch passed.

A supplementary private-CloudKit fallback check failed: the phone missed a later Mac
checkpoint despite repeated successful refresh calls. Recreating only the diagnostic
CKSyncEngine without its saved state recovered it. The same intervention was needed
during initial setup. This is an integration blocker to investigate, not a production
cursor-reset policy. The relay feasibility result does not qualify CloudKit reliability.

Use the existing owner, private full-file CloudKit library and synchronizable Keychain
binding for the next iteration. No evidence calls for another document engine, accounts,
a separate same-account relay mode or a durable CRDT outbox. Finish the native seams
before connecting the shipping editor UI. Hosted browser isolation/storage and relay
latency can be explored independently as the next discovery spike.

## Scope and implementation

These are disposable signed diagnostic apps using the current core and an adapted copy
of Swift LibraryTransport. Shipping app bundles and user documents were not replaced.
The probe has a minimal Rust-packed app with a list of text rows, not an authored WebView
or a live slop dependency. Swift receives typed diagnostics and opaque credential strings;
Rust owns edits, validation, SQLite, full-file merging, credentials and relay networking.

The device was a physical wired iPhone 13 mini, iOS 26.6.2, with Developer Mode enabled.
Both apps reported the same hashed CloudKit account identity. Distinct signed bundle IDs
`com.hitslop.livesharingprobe.mac` and `com.hitslop.livesharingprobe.phone` carried the
common Keychain group `78UAXU8QG8.com.hitslop.live-sharing-probe` in their actual signatures.
Both had Development CloudKit access to `iCloud.com.hitslop.syncspike`.

The isolated Rust facade uses the real Library/Owner. Its supervisor translates the
preferred serde/postcard envelope into the current core frames entirely within Rust.
Native HTTPS/WebSockets use Rust TLS with bundled trust roots on iOS. Socket sink queues
have byte/count limits; reconnect and lifecycle detachment reuse the owner hooks.
The server was the previously qualified opaque Rust Worker, not a new TypeScript codec.

CloudKit adaptation adds Rust eligibility checks at enqueue and actual staging, distinguishes
manual intent from foreground refresh, records staging observations, scopes fetch options
to the test zone, and provides explicit diagnostic reset/cleanup commands. It is not an
unmodified production-transport qualification. The 60-second clock is process-local.

## Observed checks

- Synchronizable after-first-unlock Keychain items arrived Mac to phone and phone to Mac,
  and survived phone process restart. Sequential conflicting insertion refused and retained
  the original fingerprint. This does not establish distributed compare-and-swap.
- **Document first:** CloudKit installed a complete `.slop` before the actual room key.
  The phone made an unsent local edit while credential access and CloudKit were withheld.
  The Mac edited and seeded the room. After allowing credentials, the phone's Rust owner
  validated and merged that seed into its existing file and sent its unsent edit through
  the relay. Both edits survived; both sides reached Synced with identical fingerprints.
- Both devices then disconnected relay and CloudKit, edited independently and reconnected
  only the relay. All four initial edits converged with CloudKit still paused.
- **Credentials first:** a second UUID's actual opaque room binding arrived by Keychain
  while CloudKit was paused and no local document existed on the phone. After resuming
  CloudKit, the document installed and joined without another cursor reset. No empty local
  document was manufactured from the credential.
- Twenty-five rounds of one edit per device produced fifty further edits. Both owners
  reached 54 rows with matching fingerprints. Rust verified the expected labels.
- Automatic staging, explicit bypass and quiescence passed as measured below.
- Settings was opened without changing system settings to background the phone. The probe
  reported background/inactive, detached its relay, and the Mac edited. Terminating and
  relaunching the phone recovered 57 rows and all expected labels. UUIDs and original file
  paths survived. This is a short background/process-death check, not prolonged suspension.
- With phone credential access withheld and the Mac relay paused, the phone saved another
  local edit and delivered it through private CloudKit. Both reached 58 rows. Restoring
  credential access rejoined the same documents and relay rooms.
- Ending both rooms paused all four native sockets with `Sharing ended`. A further Mac
  edit saved locally and received a CloudKit upload receipt. The phone stayed at 58 rows
  during repeated refreshes; after the explicit diagnostic reset it reached 59 rows with
  the same fingerprint/path, and Rust verified the retained labels. Ordinary post-end
  private-CloudKit delivery therefore **failed** this campaign.

### Actual checkpoint measurements

The observation window lasted 196.7 seconds, including edits, convergence, checkpoint
receipts and a final 20-second idle interval. Controller/device overhead means this is
not a precise five-second workload or an end-to-end latency benchmark.

| Device | Automatic attempts for the edited document | Gaps between attempts | Staged file bytes |
|---|---:|---|---:|
| Mac | 3 | 62.17 s, 60.96 s | 57,344 + 61,440 + 65,536 |
| iPhone | 3 | 60.23 s, 61.07 s | 57,344 + 61,440 + 65,536 |

These are full-file staging attempts, including conflicting attempts, not successful
uploads on both devices or billed network bytes. Both dirty flags cleared and there were
no further stages during the idle interval. A fresh automatic refresh inside the cooldown
did not stage; explicit Sync Now did. Ordinary library documents retain their normal policy.
The experiment does not establish a durable rate bound across process restart.

## CloudKit finding and next work

Two misses occurred in this run: initial phone discovery and a later Mac checkpoint after
room ending. Initial direct record reads succeeded on both devices while normal phone
fetches delivered nothing. Repeated broad and zone-scoped engine fetches did not recover
the misses. Explicitly stopping the probe engine and constructing a fresh one with nil
state did recover them. Other deliveries, including credential-first discovery and the
58-row private-cloud check, succeeded without resetting.

The initial miss preceded actual relay room creation, so the issue is not established as
collaboration-specific. It was observed in adapted diagnostic apps; this does not prove
that ordinary documents in the unmodified shipping app exhibit it. The 60-second candidate
upload throttle is a separate policy applying only to relay-bound documents.

This narrows the next investigation toward fetch/subscription/cursor/engine lifecycle;
it does not identify the cause or prove an Apple SDK defect. Repeat on a fresh namespace
with uncapped, structured per-generation callback diagnostics, persisted-cursor ordering
and direct record version observations. Do not silently reset or bypass the production
cursor, and do not count the recovered post-end check as a normal-sync pass.

Other remaining native gates: crash-safe room init/seed/credential publication after an
unknown reply; the owner's aggregate byte budget while commands hold frames; socket
cancellation/lifetime cleanup; restart checkpoint policy; simultaneous conflicting room
creation and missing-key invitation UX. New attachments, actual airplane mode, first
unlock, account switching, prolonged suspension, production CloudKit, visible editor
publications and hosted browser origins were not qualified here.

## Provenance and verification

Root HEAD: `67bf4de4a83ab8bb6fdf2e4baf2cf7f2222e188c`, with the staged/unstaged iOS and
library work present. Loro remains pinned to `c00c9fa501f8d32f68d6255eacb7035a67fb6ab6`.
The local `spikes/live-sharing-native/` contains the XcodeGen project, source, generated
UniFFI bindings, Cargo.lock, controller scripts and per-command evidence. These ignored
artifacts are not present in a fresh clone. `out/source-hashes.json` identifies 23 files,
whose tested bytes are frozen under `out/tested-source/`;
its SHA-256 is `f360a8fe04eb03f7e8b5fe5787a402559089d08dce56568fc57db36c0a3c315b`.

| Probe source | SHA-256 |
|---|---|
| rust/src/lib.rs | ee9a691c6e640fea18a41497890db8ce061d30848452d4e272022c04982d518c |
| rust/src/transport.rs | 9a4e2916964a9bc8e919832e1ce0b06b2fb2923485edfc5a56cad07505dbcdff |
| App/LibraryTransport.swift | 673f2db168baea0eabff9fb131647605b66a710d57b0554c412705df052ac562 |

Rust release builds passed for native arm64 macOS and `aarch64-apple-ios`; native clippy
with `-D warnings` passed. Both signed Release Xcode builds passed and ran on the devices.
The main controller result is `out/device-campaign.json` (`passed: true`); the supplementary
result is `out/fallback.json` (`passed: false`). `out/ended-room-cursor-recovery.json`
separately records recovery after intervention, preserving the failed normal-sync result.
The earlier Keychain-only phase is retained in `out/keychain-phase.json`.

`bun run verify --native` reused the runner's passing evidence for the exact current
inputs; it reported nothing to verify. This is cache reuse, not another full test replay.
Isolated signed builds and the actual device campaign are the new checks in this step.
A subsequent cleanup-only Swift addition removes the two exact probe subscriptions;
the Mac Release build and signed cleanup invocation passed. Its separate hash is recorded
in `out/cleanup.json`; it does not change the frozen campaign source hashes above.

## Cleanup

Run `cb1a06ba32ee` used Development CloudKit zone `spike-live-cb1a06ba32ee`, Worker
`hitslop-device-sync-cb1a06ba32ee` (deployment `90aff4b6-1897-4392-b8d6-b760d61d38cf`)
and R2 bucket `hitslop-device-sync-cb1a06ba32ee-seeds`. Both actual rooms were ended through
the creator capability before deleting credentials. The isolated CloudKit zone and the
two exact disposable bundle subscription IDs were deleted successfully (already-absent
subscriptions are accepted). Both probes reported zero items in their namespaced Keychain
service after cleanup. Wrangler confirmed Worker and empty-bucket deletion.

Private gate/deployment-secret files were deleted. Both probe apps were closed; their
local diagnostic files and source remain for review. No production routes, shipping
bundles, the shipping Library zone or user documents were changed by this device probe.
Credentials were not placed in documents, authored channels, query strings or diagnostic
reports. There are no active cloud resources from this run.

## Primary references

- [Apple: synchronizable items](https://developer.apple.com/documentation/security/ksecattrsynchronizable)
- [Apple: shared Keychain groups](https://developer.apple.com/documentation/security/sharing-access-to-keychain-items-among-a-collection-of-apps)
- [Apple: fetch completion](https://developer.apple.com/documentation/cloudkit/cksyncengine-5sie5/fetchchanges(_:))
- [Apple: fetch options callback](https://developer.apple.com/documentation/cloudkit/cksyncenginedelegate-1q7g8/nextfetchchangesoptions(_:syncengine:))
