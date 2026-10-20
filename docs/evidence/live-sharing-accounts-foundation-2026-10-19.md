# Sharing beta foundation — 2026-10-19

This extends the gated developer preview. Account-backed room ownership is not enabled.
The existing document format, exact Loro pin, released corpora, CloudKit library and
self-contained SQLite attachments are unchanged. No phone was used in this pass.

## Attachment transfer and deployed checks

Native and WASM use one Rust inventory-difference function. Both inventories are
validated; matching hashes with conflicting size or media type are refused. Native
connection/publication retries and browser reconnections read server inventory before
choosing uploads. Existing immutable blobs need neither payload reads nor PUTs.

The extended deployed campaign passed on the updated host. Local private report:
`.hitslop/sharing-preview/check-ce6b3a20-ba36-403c-b37b-2dbe345506e0/report.json`.
Do not publish that directory: fixture credentials are stored beside the report.
The disposable room was stopped.

- Private app/CSS/font/PNG loading worked with third-party cookies restricted;
  anonymous and sibling-host resource reads were refused.
- Native and two browser owners converged, including a late join.
- An interrupted attachment receive fenced queued CaughtUp until the image was installed.
- Download preserved the canonical UUID, edits and attachment and reopened natively.
- Browser process reopen followed by another edit produced **zero blob PUTs** and
  returned to Synced.
- Stop revoked remote access while the open browser copy remained editable.
- The auth probe returned `401 sign_in_required` for anonymous, malformed and
  unknown-key requests. This does not prove a valid Firebase session.

The new reconnect assertion first failed after the edit had already reached the native
owner: transport statuses bypassed the cached core status, leaving the UI at a file
transfer message. All sync statuses now share one publication cache, and active incoming
file work fences publication of the core status. The same deployed check passed after
the fix, retaining the interrupted-download barrier check.

Qualified preview relay: `d29378ce-c24c-435d-b840-64f1d73bb97b`.
Qualified host: `6c5bc2d7-bc19-41b0-a02a-34f7f7bf3888`.
Relay/host startup reported 11 ms / 3 ms. These are deployment startup measurements,
not edit, paint or network latency measurements.

## Identity setup and spike

Apple Developer now has `com.hitslop` as the primary Sign in with Apple ID, with
`com.hitslop.app` and `com.hitslop.iospreview` grouped under it. The user approved this
identity grouping. Bundle IDs in the existing native targets were not renamed.
Both current native IDs have Firebase registrations in the existing `hitslopapp`
project; its Apple provider is enabled. A production Mac build in the same App Store
record as production iOS must use that record's `com.hitslop` ID.

The separate `HitSlopAccount` target initializes a named Firebase app, requests Apple
sign-in with a secure random nonce, refreshes the Firebase token, and asks the Rust
client to verify it through `GET /v1/auth/probe`. It compares the verified UID and shows
only a short account fingerprint. Only a transient refreshed ID token crosses into
Rust; Firebase manages its own local login persistence. Tokens are not logged, stored
in documents or sent to authored pages. Default Analytics/Crashlytics behavior
is not changed by the auth target.

The Rust/WASM verifier pins Google's certificate endpoint and RS256, checks project
issuer/audience, signature, subject and timestamps, and refreshes the cached signing
keys on expiry/rotation with an unknown-key cooldown. Native tests cover valid signed
claims, forged/missing/wrong claims, algorithm confusion and cache replacement. These
tests and the attachment/client tests passed together (10 tests). The Worker WASM
check, sharing build and both deployment dry runs passed.

An optional IAM signing probe was denied (`iam.serviceAccounts.signJwt`); no permission
was granted and no custom-token test user was created. Real native Apple sign-in is
the intended positive-token proof.

The actual `hitSlop-macOS` Debug target subsequently built with automatic development
signing and passed deep/strict signature verification. The developer-only Sharing
Account window completed Apple sign-in with the user's Touch ID, signed into Firebase,
and successfully verified a refreshed real Firebase ID token through the deployed
Rust/WASM Worker. The returned UID matched the SDK UID. The visible account fingerprint
is reserved for device comparison and is not published here. This proves the Mac path;
the matching iPhone identity is still a gate.
Cold-process account-panel confirmation was not completed: native menu automation
retained stale accessibility elements after relaunch. This is not a demonstrated
Firebase persistence failure; keep the restart check in hands-on qualification.

## CloudKit diagnosis

Debug-only `HITSLOP_LIBRARY_DIAGNOSTIC_NAMESPACE` isolates local metadata and a fresh
CloudKit zone. Opt-in `HITSLOP_LIBRARY_TRACE=1` records lifecycle, fetch/cursor events
and counts without document payloads, paths or record IDs. Cursor/version values are
hashed. The ordinary library and saved cursor are retained. This is instrumentation,
not a demonstrated fix for the earlier initial/post-Stop discovery misses.

## Verification and remaining gates

The full verifier initially found a Rust helper placed after a test module; that lint
was fixed. A subsequent run passed compilation/lints and 388 Rust tests before two
existing command-runner tests hit their three-second execution deadlines while native
and WASM builds were also running. The run stopped early; it is not a full pass.
Repeat verification after those builds finish before treating this slice as complete.

The two affected runner tests passed on a focused repeat. Subsequently,
`NEXTEST_TEST_THREADS=1 bun run verify --native` passed in 1147.1 seconds: type checks,
161 Bun tests, 102 CLI tests, native render checks, all 431 Rust tests, packed-package
checks, Swift integration and 32 native replay/crash tests (one skip). Both published
corpora (1.0.0 and 1.0.1) passed. Concurrency was bounded; deadlines were unchanged.
The earlier timeout failure is retained above rather than treated as a source fix.

`bun scripts/build/ios.ts` prepared current device/simulator Rust libraries and the
arm64 simulator app built successfully with `CODE_SIGNING_ALLOWED=NO`. No simulator
launch, phone installation or phone launch was performed. No production bundle ID
was renamed. The signed Mac build initially used stale generated bindings; regenerating
the native binding before the successful rebuild corrected that build order.

Still required: matching Mac/iPhone UID, fresh two-device CloudKit diagnosis and
remaining hands-on qualification. Keep account registry/owner-admin cutover gated on the
identity proof. Server reservations/recovery, preview credential removal, re-share,
unshared-copy recovery, deletion, beta limits and protocol freeze remain subsequent
implementation slices.
