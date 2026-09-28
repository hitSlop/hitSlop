# hitSlop v1

Read the slop’s `manifest.json` first. Accept only `hitslop-v1`; no legacy migration. Preserve shipped contracts, sealed runtime bytes and release records. Read [engineering contracts](docs/engineering-contract.md) for platform changes and [versioning](docs/versioning.md) for compatibility/release changes.

## Non-negotiable

- Loro in the WebView owns live state; Swift persists opaque bytes in `state/document.sqlite` (format 1). One OS writer lock owns a local package. Route CLI edits to its live session or acquire ownership when closed; never bypass a busy lock or unlink `writer.lock`.
- Preserve the existing macOS client and shared HitSlopWasm/HitSlopRuntime engine. No second engine, JavaScriptCore evaluator, compatibility lenses, JSON projection/reconciliation or `stores/data.json`. App bundles never embed the engine; host and helper runtimes match. Headless native editing never loads authored app code or requires Node/Bun.
- TypeBox owns platform contracts; run `bun run schema:generate`, never edit generated files. Author descriptors with `defineDocument`/`s`; `initial.json` is creation-only. Read immutable snapshots; write typed handles with synchronous `change`. Preserve `$id` application identity; the runtime owns the page and apps reach it only through `ctx` (`packages/document/src/abi.ts`). Merged anomalies are preserved and flagged, never repaired on read. Initialize absent composites; never replace existing identity-bearing collections, including through containing objects.
- Flush drafts/writes before close/export. Failed saves retain ownership and show native retry; successful close destroys WebViews. Native validates isolation, symlinks, envelopes and resource bounds, not app semantics. Attachments remain host-owned immutable blobs.
- Masters are immutable; edit copies. Build packages contain no mutable state, source, dependencies or caches. PNG/PDF export, local catalog/Recents, Analytics/Crashlytics and Sparkle remain active.

## Authoring

Immediate manifest-bearing directories under `examples/slops` are active; `bundled.json` selects shipped templates. Quick Checklist/Small Expenses remain black-box fixtures. Use plain CSS and `defineTheme`; read [product guidance](examples/slops/PRODUCT.md) and [authoring](docs/guides/authoring.md) for visual changes. `_vibe` is inspiration only.

## Testing

- Before adding a test, name the observable failure, independent expected result and gap in existing coverage. Prefer extending the contract’s existing test at the cheapest stable boundary. Bun owns document semantics and compatibility replay; Swift proves distinct native integration failures.
- Tautological tests and incidental change detectors are harmful. Literal CSS/HTML, private state and internal call sequences usually aren’t contracts. Frozen runtime bytes, bridge envelopes and save-before-close ordering are.
- Add bug regression cases only for genuine coverage gaps; demonstrate failure for the intended reason before the fix. When a behavior-preserving refactor breaks a test, rewrite it at the owning boundary or delete it when equivalent proof remains. Never contort production code to preserve test scaffolding.
- Slops are black boxes; test runtime semantics in dedicated fixtures. Prefer observable completion over sleeps/global switches. Keep fault injection narrow and at real I/O boundaries; moving test flags into a wrapper alone is no improvement.
- Before removing or replacing consequential coverage, break the protected behavior and verify the remaining owner test catches it. Record changed contracts in the five-column ledger; untouched tests need no audit paperwork.
- Everyday: `bun run check && bun run test`. Native: `bun run build && bun run swift:test && bun run test:native`. Release: `bun run release:check`. Follow [testing](docs/testing.md) and [releasing](docs/guides/releasing.md); publish matching npm packages manually after the compatible signed Mac app.

## Deferred

Collaboration, audio-library import, undo UI, schema evolution, history pruning, synced folders, hosted catalog/publishing, Registry, accounts/auth and sharing. Historical `_docs/`, `archive/`, `deferred/`, retired command-engine tests and backend source are not active contracts.
