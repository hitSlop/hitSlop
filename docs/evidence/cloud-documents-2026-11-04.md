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

Production deployment is not a physical-device synchronization proof. Device Rust
frameworks and the arm64 iOS simulator app built successfully. Signed device build
and disposable-document qualification remain in progress. Existing user documents are not test fixtures.
