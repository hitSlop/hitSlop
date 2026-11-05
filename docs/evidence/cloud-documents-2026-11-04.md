# Persistent cloud documents deployment — 2026-11-04

The client-written Firestore catalog architecture was deployed from
`cloud-document-sync`, stacked over `rust-foundation-redesign`.

## Active services

- `dotpop-documents`: version `b0292778-e900-466c-8fa9-a3e12cc1670c`.
  New SQLite Durable Object classes `CloudDocument` and `DocumentAccount`.
- `hitslop-sharing-host`: version `6f10631e-af05-4636-add7-cad9ead2ca7f`.
  Its `RELAY` binding points to `dotpop-documents`; public routes remain
  `share.dotpop.click` and `*.dotslop.com/*`.
- Firestore project `hitslopapp`: own-user catalog rules deployed. Existing
  template, publisher and other project collection rules were preserved.
  No databases or indexes were deleted.
- R2 bucket `hitslop-sharing` remains the active document asset store.

The fresh Worker avoids applying incompatible class migrations to the retired
relay. All 31 Worker service bindings were inspected before retirement. No service
referenced `hitslop-sharing-relay`; the old resource-test pair referenced only
itself. The retired production relay and resource-test Workers were deleted.
Unrelated catalog and other project services were retained. The retired test bucket
`hitslop-resource-d7ef031304-files` remains: Cloudflare refused deletion because it
is nonempty. No old objects were purged from either bucket.

## Checks and limits

Live health returned 200. OpenAPI exposes 18 document routes and no old room routes.
The account document registry returns 401 without a Firebase identity. Apple App
Site Association returned 200 with the existing app identities.

Before deployment, the full native verification passed: 477 Rust tests,
266 Swift tests and 47 native integration tests; browser and sharing integration
also passed. Firestore ownership/isolation passed against the real Auth and
Firestore emulators again after preserving existing project rules.

## Physical Mac–iPhone proof

Device Rust frameworks, the arm64 iOS simulator app, the signed device app and
signed Mac app built successfully with two build jobs, serially. The iPhone app
was installed as an update; existing documents were retained. Both apps restored
existing Firebase sign-in. Production deployment alone was not treated as proof.

A new Quick Checklist was created on the physical iPhone with sync enabled.
The Mac discovered it in Synced and downloaded it on open. The document UUID was
`5afef0a6-f7d9-45e4-9cf9-b67f743a43fb`.

- The user added “Created on iPhone”; the live Mac owner's CLI read returned it.
- The Mac inserted “Created on Mac” through the live owner; the user confirmed it
  appeared on the iPhone without reopening.
- With Airplane Mode enabled and Wi-Fi disabled, the user added “iPhone offline edit”.
  The Mac read confirmed that task was absent while the phone was disconnected.
- The live Mac owner accepted “Mac edit while iPhone offline”. After reconnection,
  the user confirmed both edits on the phone. The Mac owner read returned both,
  preserving the earlier tasks and row IDs.
- The user returned to Recents and reopened the phone document; all four test tasks
  remained. The Mac reopened its saved copy and the live owner returned all four too.

Phone UI observations above were reported by the user during the test; they are not
claims of an automated XCTest pass. The normal hosted iOS test graph failed to link
an Xcode-generated Firebase Firestore dynamic package product because of missing
Abseil symbols. A temporary UI-only scheme attempt was stopped; no production
package changes or test bypasses were committed. The signed applications themselves
built successfully. Full TestFlight, account-switch and revocation qualification
remain separate release work. Earlier user documents were not edited as fixtures.

## Live local-file promotion regression

The extra Mac-local-to-sync check exposed a missing owner registration after adopting
an already-open external file. Promotion attempted a second writer and correctly
received `busy`; it did not bypass the file lock. The Swift façade now re-registers
existing owners after adoption, guarded by the account generation so an interrupted
adoption cannot reactivate an old account.

A focused regression failed before the fix with “document has a live writer”. After
the fix, all five library-owner tests passed. It checks accepted edits, an independent
copy after interrupted promotion, and retention of the original UUID, path, inode
and ability to accept later edits. No production network is needed by the test.

The signed Mac app was rebuilt. A fresh local document, “Mac promotion verified”,
was edited while open and then enabled for sync through the native menu. The physical
file inode remained unchanged. The user confirmed the document appeared on the iPhone
and opened with “Fresh Mac owner promotion”. The earlier pending test document also
became discoverable. No server redeployment was required for this client fix.

The cold Mac packaging pass built 77 templates in 338.5 seconds (76 were bundled).
The fix rebuild reused all 77 cache entries and took 0.6 seconds for that phase.

Final `bun run verify --native` passed after the promotion fix: Swift and native
integration ran again, including frozen compatibility replay. Other tiers reused
their prior exact-input passes. The affected verification took 232.4 seconds.
