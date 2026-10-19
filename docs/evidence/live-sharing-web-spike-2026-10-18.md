# Hosted browser and native relay latency — 2026-10-18

## Result

A disposable public fixture ran on real HTTPS origins with Chrome's third-party cookie
restriction enabled: trusted host/SQLite on a temporary hitslop.com hostname, authored
SDK code on two distinct UUID.dotslop.com hostnames. The browser, physical iPhone and Mac
exchanged updates through the actual Rust Worker/Durable Object with CloudKit paused.
Four owners converged, including a second browser origin joining from the complete seed.
Offline edits, reload, full Chrome close/reopen and native reopening of a browser SQLite
download passed. No production sharing UI, format or wire was installed.

The important architecture finding: **the local browser host's embedded Service Worker
does not work under this cookie restriction.** Chrome denied its registration. The Rust
owner had successfully imported and saved the document, but the authored UI could not
boot because its assets depended on that worker. This happened locally and the denial
was confirmed again on the real HTTPS dotslop origin.

The passing alternative serves this fixture's immutable UI JavaScript/CSS directly from
its dotslop origin. The trusted first-party worker retains the Rust owner, OPFS and room
credentials. Only the public infrastructure fixture's UI files were published; private
document state, SQLite seeds and credentials were not public assets. General private app,
media and attachment delivery is a separate gate. Do not promote this fixed-fixture asset
dispatcher as a general hosted document service.

## Tested path

- Chrome 155.0.8059.82, an isolated persistent profile, normal certificate verification
  and Network.setCookieControls with restriction enabled and metadata/heuristic exceptions
  disabled. Embedded Service Worker registration was explicitly refused while the UI ran.
- The same physical iPhone 13 mini and Mac from the
  [signed-device spike](live-sharing-native-spike-2026-10-18.md), with new disposable
  signed builds linking the current Rust owner/library. Native probes perform owner edits
  and echo replies without an authored native WebView; native paint latency is not measured.
- The existing BrowserDriver, serial owner and SQLite sahpool OPFS store, through an isolated
  copy of BrowserCopy. Joining verifies seed SHA-256, canonical UUID and room binding. It
  preserves identity; ordinary local imports still use their existing independent-copy path.
- The same opaque serde/postcard relay. All frame conversion/validation remains in Rust.
  Browser JavaScript supplies WebSocket IO, timers and opaque byte transfer, not a codec or
  another CRDT. Browser credentials stay in the trusted host's private worker/storage channel.
- A browser handshake adapter uses a public selected WebSocket subprotocol plus capability
  and gate offers. The response selects only the public protocol. Credentials are not URL
  query parameters or selected response protocols. The relay checks the exact trusted
  Origin and serves seed CORS only to that origin. This is experimental authentication,
  not a released invitation/ticket protocol.
- Rust-generated hosted CSPs replace the isolated adapters' localhost assumptions. The
  local host and its policies were not changed. Initial fresh joins wait for catch-up before
  mounting authored code, so startup effects see the caught-up state. Cached reopening
  does not use that fresh-join gate; offline browser reopening itself was not exercised.

## Outcomes

1. The real SDK fixture booted on dotslop despite denied embedded Service Workers.
2. A canonical full seed joined the hosted browser, Mac and iPhone. All reached Synced
   with identical Rust state fingerprints. CloudKit remained paused throughout the campaign.
3. Browser edits survived reload with the same document UUID. A competing tab for the same
   browser copy was refused by its Web Lock.
4. Authored code could not read parent storage, access the trusted diagnostic control
   object/WASM owner or read the trusted host by cross-origin fetch. The relay refused an
   authored Origin, a wrong bearer and a missing gate with HTTP 403. This is targeted
   isolation evidence, not a general hostile-app penetration test.
5. Native and browser application echo campaigns passed in both directions, as below.
6. With the browser network/relay paused, the Mac and browser edited independently. The
   browser saved locally, reconnected and merged both edits. All three fingerprints matched
   at 245 rows; reload retained the union and canonical identity.
7. A second browser copy on another dotslop origin installed the full original seed,
   backfilled retained history and joined. One further edit reached all four owners at
   246 rows with equal fingerprints; both browser UI counters also showed 246.
8. Browser download used the existing draft drain, Rust flush barrier and SQLite page
   export. The resulting .slop reopened in the native Rust core with the same UUID and
   246 rows. The picker destination was a controlled test adapter, not a human Save As.
9. The Chrome process closed completely, then reopened the same isolated profile. Its
   existing hosted copy reopened with retained state, made another edit, and converged
   with both physical native devices at 247 rows and equal fingerprints. No reimport or
   new UUID was required.

## Latency observations

Each row contains 30 connected-edit samples, with all samples retained, including tails.
p95 uses nearest rank (ceil(0.95 * n)). Every edit round trip is measured on the origin's
single monotonic clock: append a unique probe row, receive an application-level echo row
from the remote owner/SDK, then observe it locally. Native responders read through Rust;
the browser responder uses the actual authored SDK/reactivity path.

| Origin → responder → origin | Median echo RTT | p95 echo RTT | Maximum |
|---|---:|---:|---:|
| Mac → iPhone → Mac | 168.04 ms | 389.95 ms | 749.80 ms |
| iPhone → Mac → iPhone | 161.45 ms | 442.73 ms | 853.07 ms |
| Mac → browser SDK → Mac | 144.23 ms | 257.20 ms | 693.20 ms |
| Browser SDK → Mac → browser SDK | 148.40 ms | 167.50 ms | 500.20 ms |

These are **round trips**, not one-way delivery, time to visible paint, a global network
estimate or an SLA. Do not divide by two and present that as measured one-way latency.
USB/controller commands start the campaigns but are outside these timing spans. Cold
seed download, browser startup and late-join catch-up are not mixed into this table and
were not timed separately. Only this session's physical devices/network were measured.

The native socket supervisor separately measured push write → matching Stored Ack:

| Native origin/campaign | Median durable Ack RTT | p95 | Samples |
|---|---:|---:|---:|
| Mac, iPhone responder | 72.83 ms | 155.05 ms | 30 |
| iPhone, Mac responder | 65.90 ms | 267.64 ms | 30 |
| Mac, browser responder | 73.14 ms | 176.19 ms | 30 |

Stored Ack follows the relay's durable SQL transaction/storage sync; it does not assert
remote-device persistence or paint. Browser-origin durable Ack timing was not instrumented.
The probe uses a 5 ms native socket read timeout, 5 ms native echo state polling, 2 ms
origin observation polling and 30 ms spacing between samples. These contribute overhead
and CPU use; they are not a shipping mobile networking design. The older supervisor's
100 ms timeout was not benchmarked. No before/after speedup is claimed. The 60-second
CloudKit checkpoint policy has no role in these paused-CloudKit measurements.

## Sources and checks

Root HEAD remained 67bf4de4a83ab8bb6fdf2e4baf2cf7f2222e188c with the existing dirty iOS/
library worktree. Loro remains exactly c00c9fa501f8d32f68d6255eacb7035a67fb6ab6.
Ignored sources and artifacts are under `spikes/live-sharing-web/`; they are absent in a
fresh clone. Its README describes the build and evidence. `out/source-hashes.json`
identifies 43 inputs, frozen under `out/tested-source/`. Manifest SHA-256:
`8c37679dcb38076342528a0e2dcbb829212f30d16b6143779a4156e493696181`.
Subsequent native edits only collapse nested conditions/remove needless borrows for lint;
the frozen files identify the binaries used for the device timing campaign.

- Rust WASM owner and opaque workers-rs builds passed. Mac and physical iPhone Release
  builds signed, installed and ran. Native and browser-WASM Clippy with -D warnings passed.
- Both Worker deploy dry runs passed before deployment. Actual custom-domain HTTPS
  certificate verification passed; no browser TLS, SOP or web-security bypass was used.
- `out/service-worker-refusal.json` retains the failed original approach. `browser-local.json`
  and `browser-hosted.json` record the passing alternative. `campaign.json` passed;
  `browser-process-reopen.json` and `process-reopen-convergence.json` passed.
- `latency-*.json` preserve raw samples; `latency-summary.json` gives the statistics above.
  `access-refusals.json` records the targeted HTTP refusals. The final screenshot is
  `out/hosted-browser-final.png`; `browser-download.slop` is the exported SQLite fixture.
- `bun run verify --native` reused passing evidence for the exact shared-repository inputs;
  it reported nothing to verify. The new checks were the isolated builds and actual hosted
  browser/device campaign, not another full repository test replay.

## Decisions and remaining gates

Proceed with the same Rust owner and opaque relay. Use a dedicated trusted top-level host
and per-copy authored origins. Do not require embedded Service Workers for hosted resources
when third-party cookies are blocked; retain the working local browser path unchanged.

Next prove general resource delivery: immutable app/media hosting and a private attachment
path that preserves the authored/trusted boundary and does not expose room credentials.
Distinguish public package artifacts from private document blobs. Experiment with scoped
resource grants/partitioned HttpOnly cookies if needed, rather than putting a secret in a
resource query string or asking the authored frame to hold room keys. No such grant/cookie
system was built or qualified in this run.

Native unknown-reply room creation, owner command-held byte budgets, production socket
lifetime/async IO, checkpoint restart policy and the prior CloudKit fetch misses remain
shipping gates. Hosted storage failure recovery, offline browser restart, private packages,
seed/new attachments, app-code integrity/version dispatch, invitation UX and a sustained
latency/load campaign also remain unqualified. The fixture's public resource dispatcher
only serves its fixed UI roles; it is intentionally not a catalog/publishing implementation.

## Cleanup

Run d0641ab982 used Worker hitslop-web-relay-d0641ab982 (initial deployment
2ec6225b-8f2e-4cf5-ba8d-20ea12c33c25), static host Worker hitslop-web-host-d0641ab982
(final deployment bac18631-d4a7-4286-a9f2-1bb2fdc0fa59) and R2 bucket
hitslop-web-d0641ab982-seeds. Temporary custom domains were
web-spike-d0641ab982.hitslop.com and the two UUID.dotslop.com names in run.json.

The room was ended while the creator key remained available. Both Workers and the empty
bucket were deleted; the Cloudflare API confirmed no custom-domain bindings remain for
the host Worker. The Development test zone spike-live-d0641ab982 and exact probe subscription
IDs were cleaned up. Both native probes reported zero namespaced Keychain items. Private
gate/invitation files and isolated Chrome profiles were removed; local servers stopped.
Probe apps were closed. Source, synthetic .slop files and nonsecret reports remain for
review. No apex/landing routes, shipping app bundles, shipping Library zone or user files
were changed. `out/cleanup.json` records the cleanup.

## Platform references

- [Cloudflare WebSockets](https://developers.cloudflare.com/workers/runtime-apis/websockets/)
- [Cloudflare custom domains](https://developers.cloudflare.com/workers/configuration/routing/custom-domains/)
- [Chrome partitioned cookies](https://privacysandbox.google.com/cookies/chips)

The Service Worker refusal is direct evidence from this Chrome configuration; the cookie
reference describes a possible follow-up resource mechanism, not one used by this probe.
