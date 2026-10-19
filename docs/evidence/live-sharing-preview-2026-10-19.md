# Integrated sharing preview — 2026-10-19

The preserved spike is integrated into the existing owner, native façade and browser
runtime. This is a developer-flagged preview, not a production sharing rollout. No
public document/storage/runtime marker, frozen corpus or exact Loro pin changed.

## Deployed qualification

Command: `bun scripts/sharing/check-preview.ts`, after building the workspace browser
core, Rust relay and native `preview` fixture driver.

The final deployed check passed on Chrome 155.0.8059.82 with third-party cookie
restriction enabled. Its local report is
`.hitslop/sharing-preview/check-321e400f-79dd-4fed-8126-7002d33d5e0f/report.json`.
Earlier passes are retained at `check-268d4f7a-f60f-43d5-9d5b-48bc7719ad1a` and
`check-21962421-f9af-4204-824c-c7e2a34c7d0b`. These directories contain private
credentials; never publish them or their invitations. Disposable rooms were stopped.

- A real SDK app loaded private JS, CSS, a font and a PNG from its dotslop host.
  Anonymous reads failed; the cookie was HttpOnly; a sibling host could not reuse it.
- Native Rust and browser owners exchanged edits through the deployed Durable Object.
  A second browser profile joined the canonical document and received existing edits.
- A deliberately failed attachment GET forced reconnect. While later downloads were
  held, queued CaughtUp could not publish the missing file reference or report Synced.
  Releasing the transfer installed the verified image and the app rendered it.
- Browser download preserved canonical UUID, edits and the attachment, and passed
  `slop-engine get` and attachment inspection on the downloaded SQLite file.
- Closing and reopening Chrome retained the copy and edits with a connection available.
- Stop reached the open browser, revoked resources and synchronization, and kept the
  local editor usable.

A repeat initially asserted image dimensions before decoding completed. The check now
awaits `HTMLImageElement.decode()` before asserting the image rendered; subsequent
runs passed. This was a verification timing issue, not a claimed product fix.

Preview services: `hitslop-sharing-relay-preview` and `hitslop-sharing-host-preview`;
private bucket: `hitslop-sharing-preview`; trusted host: `share.hitslop.com`; authored
hosts: `*.dotslop.com`. Final qualified relay version:
`04bef358-0421-47e5-863b-3b4cdec9fe9b`; host version:
`3cf21210-06f2-4c88-abba-5ca54db1a8a7`. The old exact spike custom domains remain intact.
The landing apex was not changed.

## Signed Mac app

Built the actual Debug `hitSlop-macOS` target with automatic development signing. A
renamed disposable copy, retaining the app's bundle ID, profile and entitlements, kept
automation separate from other running hitSlop instances. It used the developer flag
and creation key supplied only through the launch environment.

The app's actual native controls passed:

- File → Live Sharing opened the full history warning and Create control.
- Create saved credentials to the shared Keychain and reached Synced. Copy Edit Link
  produced a valid invitation in the native clipboard.
- Quit/reopen restored credentials and reached Synced at the same path and UUID.
- An edit in the native authored form reached the browser; a browser form edit appeared
  in the native document. Both used their existing Rust owners.
- Native Stop reached the browser. Both local editors remained usable afterward.

Local evidence: `.hitslop/sharing-preview/mac-ui-report.json`. Fixture UUID:
`badee93c-2016-4989-b309-9a50a023487d`; its path remained
`.hitslop/sharing-preview/mac-ui.slop`. The disposable room was stopped and the test app
quit. Native Join opened, but automation lost menu focus before submission, so its
hands-on success is not claimed. No phone was used in this integration pass.

## Owner and build checks

Owner coverage passed for aggregate byte and frame-count overflow, recovery and refusing
frames/detach from a replaced connection. Extending the blocked-owner case to tiny frames
failed before adding the admission count bound and passed afterward. Library coverage
passed for process-local automatic relay
checkpoint throttling, manual bypass, ordinary-document behavior and restart eligibility.
Client tests cover stable staged seed identity after edits, preserving creator authority,
refusing staging on another device and rejecting untrusted invitations before installing
a file.

`bun run verify` passed all applicable portable tiers, including two published frozen
compatibility corpora and 426 Rust tests (15 skips). `bun run verify --native` also
passed portable and Swift/native integration checks. The integration run completed in
821.8 seconds; native replay passed 32 tests with one skip, including both released
corpora (1.0.0 and 1.0.1).

A later frozen-source run intermittently failed the existing
`widgetOnlyCloseDoesNotRewriteDocumentArtwork` test: an unchanged close published
another widget generation. Its focused repeat and the full Swift/native repeat passed
without source changes (173.6 seconds for the full repeat). The cause remains
unconfirmed; this failure is retained in the PR notes rather than treated as a fix.

GitHub's Swift compiler also refused an unqualified `native` property read in the
CloudKit resume closure because a local variable with that name is declared later.
The read now explicitly uses `self.native`; the CI failure was compilation, distinct
from the intermittent widget expectation.

The arm64 iOS simulator target built with `ARCHS=arm64 ONLY_ACTIVE_ARCH=YES` and
`CODE_SIGNING_ALLOWED=NO`. A generic simulator build initially attempted x86_64 too,
which this Apple-silicon-only FFI artifact does not contain. No device installation or app launch was performed; the phone was not used.

## Remaining gates

Qualify native Join submission and the integrated phone controls, same-account Keychain
delivery, document/key arrival ordering, background/relaunch, WKWebView resource delivery
and real CloudKit checkpoints without cursor resets. Earlier isolated probes do not
qualify these integrated paths. Investigate the previously observed CloudKit discovery
and post-stop checkpoint misses independently of the relay.

Offline web reopening, room compaction, accounts, person-level permissions, full-history
save-limit recovery and production rollout remain deferred. An already-open browser
copy stays editable when sharing ends; reopening that ended room is not qualified.

This run did not measure latency. The earlier
[430–439 ms resource-probe echo RTT](live-sharing-resources-spike-2026-10-18.md)
remains historical application round-trip evidence, not one-way or paint latency.
