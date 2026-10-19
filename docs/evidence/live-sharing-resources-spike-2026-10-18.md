# Private resources and attachment sync — 2026-10-18

The isolated private-resource experiment passed on real Cloudflare HTTPS hosts, Chrome,
Mac and the physical iPhone before phone testing stopped at the user's request. Follow-up
Mac/browser checks continued on October 19. This is a transport feasibility result;
the shipping editor has no Share/Join UI or production relay connection.

## Scope and implementation

Source/artifacts: ignored `spikes/live-sharing-resources/`, run `d7ef031304`, cloned from
the [public-fixture web experiment](live-sharing-web-spike-2026-10-18.md). The existing
Rust owner, SQLite file and pinned Loro revision remain authoritative. Browser SQLite
uses the dedicated WASM Worker and OPFS; IndexedDB contains the browser catalog only.
No production formats, markers, wire or compatibility expectations were changed here.

Private fixture JS/CSS, PNG and font are immutable R2 objects. The native core's accepted
app/resource reader produces their inventory; browser Rust verifies the inventory against
its accepted embedded SQLite app. Preview/icon/command programs are not exposed by the
authored resource route. Rust validates inventories and bounds and uses the existing
resource range/response planner, copied reproducibly by `prepare-policy.py` for this probe.
The static host Worker delegates resource routes through a service binding to the Rust
relay, retaining the original authored URL for exact-host checks. Public static files
are the host/frame/runtime shell, never fixture app bytes or document data.

The trusted host establishes a resource session before mounting authored code. A ten-minute
`__Host-` cookie is Secure, HttpOnly, SameSite=None and Partitioned; its server record binds
the hashed opaque token to the exact authored host, app and room. Renewal is scheduled at
five minutes. This read grant cannot authorize room operations. Authored code receives
ordinary resource URLs, not room capabilities. All tests use Chrome's third-party-cookie
restriction with exceptions disabled; embedded Service Worker registration remains denied.
This establishes the tested Chrome path, not universal browser or WKWebView behavior.

For the simplest attachment proof, before sending a Push each client uploads retained
blobs absent from its in-memory uploaded set. Before applying Records it reads the bounded
room inventory, downloads missing blobs, checks SHA-256/size/MIME and installs through
the owner. Only then does it submit the record. The document wire is unchanged. Browser
network pumps are separate from local page requests; imports and ordinary edits save
locally while an upload is held. There is no durable CRDT outbox or second semantic engine.
Seed attachments are uploaded before room credentials are published.

Initial web opening requires a connection. Offline editing/import once open works;
reliable offline reopening and offline resource URLs are outside this slice. After a
process restart the upload cache is empty and retained files can be uploaded again.
The prototype buffers one bounded blob; it is not a global memory/load qualification.
The CloudKit preview's complete-file limit remains 16 MiB and is independent of the
core's per-file/aggregate attachment bounds and saved-Loro ceiling.

## Observed outcomes

`out/resource-checks.json` reports nine passing checks:

- Private SDK UI, stylesheet, font and image boot despite denied embedded Service Workers.
  An unauthenticated app fetch returns 403; the command program route returns 404.
  The cookie's partition key is `https://hitslop.com`.
- Wrong app/hash manifests are refused against the browser's accepted SQLite app.
- A new native PNG reaches browser and phone. A browser SDK import saves and a local
  edit resolves while its upload is held; losing the successful upload's HTTP reply
  recovers through reconnect/retry.
- An offline browser import and concurrent native 10 MiB attachment merge after reconnect.
  Mac/iPhone/browser state fingerprints and attachment inventories match, with CloudKit
  paused from startup and zero checkpoint sends/fetches in the final campaign.
- Attachment ranges return correct 206 bytes and an unsatisfiable range returns 416.
  Injected OPFS write failure refuses export before emitting bytes; recovery succeeds.
- Another browser copy backfills files, while a copied cookie on the sibling authored
  origin returns 403 and sibling fetch is unreadable. The final rerun reopened that copy;
  the earlier fresh-copy case also passed before the native exact-limit fix.

`out/resource-refusals.json` also passes: wrong upload checksum 422; active HTML MIME 400;
WASM receiver corrupt-byte/size refusal without inventory changes; cancelling a resource
stream after 8192 bytes followed by a successful new range; cookie-as-room-Bearer 403;
missing cookie 403; connected reopening renews access without changing canonical identity.
This targeted evidence is not a general hostile-app or abuse assessment.

`out/seed-attachment.json` passes in a separate browser profile: a photo present before
sharing renders with zero relay records and zero data rows. It therefore does not depend
on a later edit triggering file backfill.

Mac/browser-only follow-up: the downloaded SQLite file reopened in the native Rust core
at 139 rows, with 13 distinct stored attachment hashes verified and 42,049,611 attachment
bytes. The document lists 16 file references because some blobs are reused. A final native
edit was flushed at 140 rows; Chrome received it. Full Chrome process reopening preserved
that copy and saved another ordinary edit at 141 rows. The exported file is the earlier
139-row cut, not a claim that later test edits were included. Screenshots are local in
`out/hosted-browser-final.png` and `out/seed-attachment.png`; the fixture image/font visibly
rendered. Native probes exercise owners/networking, not authored native WebViews.

## Timing

Warmed sequential text application echoes, CloudKit paused, 30 samples per direction:

| Origin → responder → origin | Median | Nearest-rank p95 | Max |
| --- | ---: | ---: | ---: |
| Mac → browser SDK → Mac | 439 ms | 652 ms | 755 ms |
| Browser SDK → Mac → browser SDK | 430 ms | 884 ms | 904 ms |

Mac's Stored-Ack median was 71 ms, measured after attachment preflight and socket send;
it excludes file upload time. Raw arrays: `out/latency-mac-browser.json` and
`out/latency-browser-mac.json`. Native socket reads use a 5 ms timeout, with 2/5 ms
diagnostic polling; browser origin polling is 2 ms. These are single-origin-clock echo
round trips, not one-way or paint latency. No new phone timing was taken after it was
released to the user.

The earlier fixture's Mac/browser medians were 144/148 ms. The additional inventory HTTP
coordination is a clear optimization candidate, but separate runs do not isolate every
cause of the difference. Keep this implementation as the conservative correctness baseline.

`out/file-latency.json` contains one browser-import-to-verified-Mac-echo sample at each
size: 16 KiB took 1377 ms; 10 MiB took 13,479 ms. This includes import, upload, R2,
recipient download/verification/owner admission and return echo. Two samples are not a
throughput distribution, startup metric or service objective.

## Failures that informed the implementation

1. A stale diagnostic command from an older run replayed cleanup in a fresh phone install.
   Commands now carry the run namespace; native nonces persist across relaunch. An early
   abandoned room lost its creator key and needs explicit run-wide cleanup.
2. The fixture initially read `ref.bytes` instead of the SDK's `ref.byteLength`. A corrected
   new master and fresh documents were used; no existing embedded app was replaced.
3. Browser retry used WebSocket close code 1011, which the browser API forbids. The exception
   left a lost-upload-reply case waiting. Application close codes 4011/4008 fixed the
   observed failure; the same fault then passed.
4. Native ureq's limit reader rejected a legal exact-10-MiB body when probing EOF. Native
   reads now reserve one byte for EOF and enforce the inclusive route limit afterwards.
   Signed Mac/phone builds passed the full campaign after this fix.
5. The later Mac restart exposed a second, separate limit issue: the adapter used the
   CloudKit snapshot inspector/merge for the local relay file. Once attachments grew the
   file beyond 16 MiB, local reopening succeeded but relay reconnection was refused.
   The isolated adapter now accepts/checks the local SQLite file through Store and merges
   the transport-bounded canonical seed through owner requests. The CloudKit policy stays
   unchanged. The follow-up build/result is recorded separately below; earlier cross-device
   and timing evidence used the pre-restart-fix binaries.

The October 19 follow-up passed: a newly signed Mac build reopened/reconnected the same
large file, then received pending browser edits. `out/mac-reconnect.json` records equal
Mac/browser fingerprints at 142 rows, all 13 hashes, 42,049,611 attachment bytes and zero
CloudKit sends/fetches. Reading the saved Mac SQLite file independently verified the same
row count and hashes. The matching iOS Rust/static and signed app builds also succeeded
with `IPHONEOS_DEPLOYMENT_TARGET=26.0`; the earlier newer-OS linker warnings disappeared.
Only the routine AppIntents metadata-skip warning remains. That prepared iPhone build
has not been installed or executed; its device qualification is deferred.

Failure reports remain local: `lost-reply-before-close-code-fix.json`,
`resource-checks-before-native-body-fix.json`, `phone-limit-failure.json`,
`mac-reopen-before-limit-fix.json` and `resource-checks-before-fixture-fix.json`.

## Reproduction and retained state

The relay deploy was `18972f89-5dc1-4cca-b64f-3deda67f23d2`; host deploy was
`4b2f4595-c53e-42bf-a16e-f2b351c035e0`. Only run-specific workers/custom domains and a
private bucket were added. No apex landing route was replaced.
Rust cache directories happen to be under the older web spike; use this experiment's
manifest paths, not its predecessor's source. The README records commands and test controls.

- WASM: offline release build for wasm32-unknown-unknown with LLVM clang/llvm-ar C tools;
  wasm-bindgen 0.2.129 web target. A later invocation without those C tools failed to link
  and was not deployed. The deployed successful build ran the campaigns above.
- Relay: worker-build 0.8.7 release/no-opt/offline, then Wrangler 4.46.0 dry run and deploy.
- Native: offline release Mac and aarch64-apple-ios Rust builds, UniFFI 0.32.2 generated
  bindings, signed Xcode Release probes. Initial iOS C objects emitted minimum-OS warnings
  (27.1 objects vs 26.0 app); execution passed on the connected device, but minimum-OS
  support was not established. The follow-up prepared build pins that C target; device
  execution and minimum-OS support still need qualification.
- Browser: `check-browser.ts --resume --keep --resources --extra`; separate seed check;
  Mac/browser timing controls and native setup `inspect` on the downloaded SQLite file.
- Repository `bun run verify` and `bun run verify --native` returned cached exact-input
  passes, not new full-suite replays. Ignored spike builds/campaigns qualify the experimental
  code directly. Preserve unrelated staged iOS/library work.

Initial campaign manifests: `out/source-hashes.json` (348 selected non-secret input files,
SHA-256 `e6117635f8852da23f3e0e82ac2a4a92abda60543c7ce12e0fb55e6b7ddf089f`) and
`out/artifact-hashes.json` (SHA-256
`f13aa6304069bad214e1ae20ce3f72b85f6cea4da58ed954e69b77c35446ae6b`).
`out/tested-source/` freezes that selected source; credentials/profiles are excluded.
Later native reconnect changes must have their own cohort manifest rather than rewriting
these campaign hashes.

Follow-up cohort: `out/cohorts/mac-reconnect-20261019/`, 349 selected input files,
source-manifest SHA-256
`c53f2a664f77bfe088c90f3431cd99b99252ad17f73d65fa80756b723c15b4f0`,
artifact-manifest SHA-256
`524ed0d5b3dac08d4d83ee8d0994fa47377df73aa2c7c105754c1bdf10926029`.
This records the executed Mac reconnect fix and the built-but-unexecuted phone app;
it does not replace the initial cross-device/timing cohort.

The bounded browser process-reopen run exited successfully at 141 rows. A subsequent
keep-open diagnostic session passed those checks again and participated in the verified
142-row Mac/browser convergence, then its page/browser closed unexpectedly before the
finish command returned. `out/browser-close-after-checks.json` records that later harness
failure; the cause is unqualified. Do not describe that final keep-open session as a clean
pass or graceful shutdown. Its profile remains available; no browser probe process remains.

Phone operations are deferred until the user is ready. Its probe was no longer running
when work continued. The run's private
profiles, namespaced credentials, test CloudKit zone and gated cloud services are retained
for resumption. Cleanup has **not** completed. Main document
`dda69ce8-289a-4822-b62b-c051ce89eb18`, main room
`4083bd31af90fb39ee37dc3d7de9e6a5`; seeded document/room are in local `run.json`.
Retire all rooms, including the early abandoned room, before removing credentials and
the run's DO class/R2 payloads/Workers/domains/bucket. Remove only the test zone,
subscriptions, keys and disposable profiles. Local documents must survive ending sharing.

Still unqualified: interrupted incoming-blob ordering against queued CaughtUp, grant
renewal and room ending during transfers, native resource UI, reliable offline reopening,
aggregate service memory/abuse/hibernation load, crash-safe credential publication,
restart checkpoint policy and the earlier CloudKit cursor misses. Next device checks and
the minimal three-host product slice are in `plans/live-sharing.md`; this experiment does
not settle those production gates.
