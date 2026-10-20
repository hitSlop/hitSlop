# Authoring environment and tester sharing — 2026-10-19

## Implemented API

`getEnvironment()` in `hitslop/svelte` exposes a read-only reactive `current` snapshot:
platform, device category and compact/regular viewport layout. Rust owns identity wire
types and the 640 CSS-pixel breakpoint. Native hosts answer a typed host action; browser
identity is unknown. The shell installs matching editor CSS attributes before mounting.
A narrow Mac window stays desktop/macOS. Environment changes never write document state.

The optional `ctx.environment` member has a genuine older-host SDK fallback: unknown
identity and live responsive layout. Runtime ABI, file/storage markers and Loro remain
unchanged. Static capture props and output remain independent of editor environment.
Dev presets retain accepted state while changing identity and viewport size.

## Independent review

The separate #46 diff and #47 → #48 → #49 stack received a read-only review of their
principal authorization, lifecycle, CloudKit merge/deletion and relocation paths.
No P0/P1 blocker was found for the gated tester-preview scope. This was not exhaustive
security or stress qualification. A new P2 follow-up is recorded in the roadmap:
native guests cannot yet replace ended membership to join a new generation after
Stop/re-share. Creator re-share recovery does not prove that guest flow.

The API implementation review caught an outer dev-preview runtime import blocked by
its narrow CSP, plus a startup-order race in the required environment handshake. The
server now supplies the escaped preset table without an outer runtime dependency, and
both startup orders send the initial preset. The follow-up review found no remaining
blocker in these fixes, native wiring, fallback, subscription cleanup or static capture
isolation. The optional invitation-based guest campaign was also reviewed.

## Verification so far

- Type/Svelte/lint checks passed; the existing sharing-fixture image-alt warning remains.
- SDK/shell portable tests passed: 162 tests.
- Targeted WebKit qualification passed: 9 tests, including preset JS/CSS agreement,
  639/640-pixel resize, retained row identity, capture switching, HMR, remount cleanup,
  host fallback and connected dev-frame reload. The initial failed startup campaign
  is retained in local verification evidence; the corrected repeat passed.
- Rust verification passed: 434 tests, including released compatibility replay and the
  typed environment request/reply plus owner-view fence checks.
- The browser host deployment passed its dry run and deployment:
  `e1fdaf91-961f-4fe1-a4ff-9e30fa5833da`. Relay ownership/auth behavior was unchanged.

The signed macOS Debug app built and passed deep/strict code-signature verification.
The installed app was updated with its previous version preserved under ignored
`.hitslop/installed-app-before-environment-20261019-2254.app`.

## Deployed guest and offline campaign

The signed Mac app restored its existing Firebase session and created a disposable
account-backed room. The native fixture driver joined through the real invitation
without a Firebase session; both Chrome profiles were also signed out. No Firebase
token was exported. The private campaign report is
`.hitslop/sharing-preview/check-79b2ed2f-c98f-4f87-b10e-1d2fdfb07af3/report.json`.
Do not publish the directory: it holds fixture invitations and browser profiles.

The deployed campaign passed:

- Initial private app/CSS/font/image loading with restricted third-party cookies;
  anonymous and sibling-host resource reads were refused.
- Native/browser edits in both directions and canonical late joining.
- Native transport pause/edit/reconnect and open-browser offline edits, preserving
  local state and merging after reconnect.
- Connected browser refresh and cold process reopening with accepted edits retained.
- Interrupted attachment reception preserving the file-before-reference barrier.
- SQLite download preserving UUID, edits and attachment, reopening in the native engine.
- Reconnection plus a new edit produced zero attachment PUTs.
- Stop from the signed owner app revoked reads and syncing; the guest local copy
  remained editable. The disposable room was stopped at the end.

These are happy-path and selected existing fault checks, not latency percentiles,
reliable offline browser reopening or broad resource/concurrency qualification.
`NEXTEST_TEST_THREADS=1 bun run verify --native` passed in 1,348.8 seconds, including
the new native before-mount identity/viewport test, Swift integration, packed SDK/CLI
checks and 32 native checks replaying both immutable 1.0.0 and 1.0.1 corpora.
Report: `.hitslop/evidence/runs/2026-10-19T22-38-38-482Z-23629/report.json`.
The Swift shards executed 236 tests, including the new native environment test.
The final incremental `verify --native` also passed, including the explicit static
capture rejection case added afterward (102 CLI tests, 707 assertions):
`.hitslop/evidence/runs/2026-10-19T23-01-46-843Z-48122/report.json`.

`bun run verify sharing` passed with the real Rust WASM, SQLite Durable Objects and R2
in pinned Miniflare, covering two creators, ownership isolation, reservations,
publication, cold recovery and Stop/re-share races:
`.hitslop/evidence/runs/2026-10-19T23-06-19-601Z-53286/report.json`.

The iOS device and simulator Rust libraries rebuilt successfully. The iOS app's
arm64 simulator build passed with `ARCHS=arm64 ONLY_ACTIVE_ARCH=YES` and code signing
disabled. An initial generic-simulator invocation also requested unsupported x86_64
Rust symbols and failed at link; its log is retained separately. This is compile
qualification, not new physical-phone testing.

## Remaining release work

Sharing stays disabled by default. Prioritize Save Unshared Copy for unsavable full
history, account deletion/Apple revocation and privacy disclosures, remaining abuse
controls/metrics and frozen network exchanges, guest rejoin, then broader resource,
concurrency and near-limit qualification. Reliable offline browser reopening and new
platform hosts remain deferred. No phone was used for this slice.
