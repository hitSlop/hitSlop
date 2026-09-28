# Approved test disposition ledger

Jordan approved the initial dispositions before pruning. This ledger groups that approved inventory by contract instead of requiring a row and survival essay for every declaration. Tier 0 is Bun; tier 1 covers distinct native risks. Detailed evidence is retained below. This simplification authorizes no additional test deletions.

Bundled slop business logic is outside the platform suite by Jordan’s explicit decision. Its deletion does not claim equivalent formula, parser or decorative appearance coverage. Contractual byte seals, resource bounds, alpha hit testing and export bounds remain meaningful requirements. “—” in duplicate-of means no equivalent owner is claimed; tests at different layers retain distinct failure modes.

## Contract dispositions

### Runtime reset: host-owned boot, ctx ABI and collaboration-ready data

| Contract | Oracle | Tier | Duplicate-of | Verdict |
|---|---|---|---|---|
| Merged anomalies stay openable, are flagged and never repaired ([open](../packages/document/tests/open.test.ts), [convergence](../packages/document/tests/convergence.test.ts)) | Literal fallbacks and exact issues per anomaly; stored generation and checkpoint bytes unchanged after open; peers converge on state and issues | Bun | — | REWRITE the former "rejects invalid stored data" cases; the counter-overflow merge failed before the fix with "Expected finite number" |
| One outbound stream per accepted local commit ([convergence](../packages/document/tests/convergence.test.ts)) | Handle, change, CLI batch and JSON import each emit once; remote imports emit none; replaying the emitted bytes reproduces the state | Bun | — | ADD; failed before the fix (no stream; staged commits emitted nothing) |
| Application `$id` identity ([import](../packages/document/tests/import.test.ts), open) | 26-character random IDs, never container IDs; JSON round trip into a new document keeps IDs and references; missing, invalid and duplicate IDs read as null with identity issues | Bun | Existing move/reopen/same-transaction owners | ADD |
| Save-time capacity ([storage](../packages/document/tests/storage.test.ts)) | Actual multibyte snapshot exceeds limit; no storage write, live edit retained, CLI reports failed (not rejected), close blocked; creation checks bytes too | Bun | — | REWRITE pre-edit refusal/reserve contract per approved plan; original code saved the oversized snapshot regression |
| Counter projection types ([types](../packages/document/tests/types.ts)) | Reads require handling null; optional reads include undefined; inputs/increments reject null at compile time | TypeScript | Existing convergence test owns overflow and issues at runtime | EXTEND |
| Native discard ([LoroEngineTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopHostTests/LoroEngineTests.swift)) | Failed save blocks close; awaited discard restores saved get while writes still fail, retains lock/doc_id, permits later edits and reopen | Swift/WebKit | Bun owns byte restoration and binding remount semantics | ADD |
| Schema-key canonicalization ([open](../packages/document/tests/open.test.ts)) | Golden canonical string for a scrambled descriptor | Bun | — | ADD |
| App bundles cannot reach the engine, bridge or remote boot resources ([build](../packages/cli/tests/build.test.ts)) | Each forbidden import or pattern fails the build with its message | Bun | — | REWRITE the former engine-import case over the new boundary |
| Frozen ABI consumers keep working against the candidate host ([2-1, 2-1-svelte](../tests/compatibility/README.md)) | Self-checks inside mount/capture; render, `RuntimeCompatibilityTests` | Native | Bun replay owns data | ADD; breaking `ctx.bind.text` in a disposable runtime failed both consumers |
| Mixed-version collaboration ([compatibility](../scripts/v1/compatibility.ts)) | Peers edit, exchange, edit again, reopen and converge on state and issues | Bun | Historical readers | ADD |
| Duplicate renews `doc_id` ([LoroEngineTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopHostTests/LoroEngineTests.swift)) | Distinct IDs, identical checkpoint | Native | — | ADD; exposed `/var` realpath handling in the renewal |
| Duplicate returns the same package URL identity as Open/Recents ([SlopPackageTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopCoreTests/SlopPackageTests.swift)) | Returned URL equals the canonical existing directory URL; opening the live duplicate from Recents focuses its window | Swift/filesystem + manual UI | Existing duplication test checked bytes and permissions only | EXTEND; failed before the fix because `copy.slop` and `copy.slop/` were unequal, causing a second writer attempt |
| Unsealed bundled templates block release ([compatibility](../packages/cli/tests/compatibility.test.ts), `check:sealed-templates`) | Regenerated previews/guidance pass; changes to manifest, schema, initial data, runtime requirements, JS, CSS or assets fail; historical preview tampering still fails its full seal | Bun + Release/CI | — | EXTEND; regression failed on the old whole-package comparison after changing only ancillary files; frozen specimen bytes remain immutable |
| Contract-1 fixtures, 1/1 byte-preservation, `transaction`/`observeText` aliases | — | — | — | DELETE with the pre-launch reset; no contract-1 documents exist |
| Failed saves preserve drafts; explicit discard restores durable state ([storage](../packages/document/tests/storage.test.ts), [lifecycle](../packages/document/tests/view-lifecycle.test.ts)) | Composition stays visible, repeat close fails; discard preserves lost-reply commits and ownership, failed reload keeps edits; remounted bindings save new drafts | Bun | Existing queued-write and retry owners remain | REWRITE reserve case; original code erased the composition and retained unsaved Loro edits after discard (both reproduced before fixing) |
| Incremental projection equals full projection ([convergence](../packages/document/tests/convergence.test.ts)) | For nine anomaly positions, state and issues after import deep-equal those after reopen | Bun | — | ADD; numeric-record repro failed first |
| Derived IDs for broken rows ([open](../packages/document/tests/open.test.ts)) | Unique `x-` IDs stable across reopen, addressable by `at()`/`move`; ownership unchanged by moves; golden hash vector | Bun | Replaces the null-ID expectations | REWRITE |
| Native compatibility by fixture kind ([RuntimeCompatibilityTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopHostTests/RuntimeCompatibilityTests.swift)) | Conformance edits only on the conformance schema; every fixture's background render leaves `get` unchanged; probe reopen equality ([LoroWindowTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopHostTests/LoroWindowTests.swift)) | Native | — | REWRITE; passed with a temporary template specimen |
| Every recorded fixture file is history-guarded ([compatibility](../packages/cli/tests/compatibility.test.ts)) | Rewrites of `issues.json`/`collaboration.json` against the base fail | Bun | — | EXTEND |
| Concurrent collaboration matches an independent model (`2-1/collaboration.json`) | Offline round plus synced edits; peers equal and equal to `expected` | Bun | Historical readers | EXTEND; fails for divergence and for equal-but-wrong peers |

### Apple lifecycle and catalog cleanup

| Contract | Oracle | Tier | Duplicate-of | Verdict |
|---|---|---|---|---|
| Partial quit keeps completed closes final ([FeatureTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopFeaturesTests/FeatureTests.swift)) | A closes, B fails, C remains open; only B/C regain commands and are retried; AppKit receives false then true | Swift reducer | Existing quit cases cover preparation failure and waiting, not failure after a completed close | ADD; baseline retained A, cancelled it, and finished it again on retry |
| Controller finalization closes native chrome and releases the document ([LoroWindowTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopHostTests/LoroWindowTests.swift)) | Window becomes invisible, weak WebView disappears, and a closed CLI edit succeeds with persisted data intact | Native | Existing owner already proves live export and subsequent closed editing | EXTEND existing owner through awaited controller finalization; retain save-failure/ownership and renderer-death coverage in LoroEngineTests |
| Same-path preview replacement updates a mounted catalog ([LocalTemplateStoreTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopCatalogTests/LocalTemplateStoreTests.swift)) | Rendered preview changes from a red fixture to a blue fixture after the preview notification, with unchanged selection | Native SwiftUI | Existing preview tests only checked bytes on disk | ADD; baseline displayed the initial red preview but never displayed its blue replacement |
| Command errors remain visible, independent, repeatable, and compatible with runtime retry ([FeatureTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopFeaturesTests/FeatureTests.swift)) | Real failing command effects produce alerts; acknowledgement clears only the intended alert; repeated errors get fresh identities and retry retains newer failures | Swift reducer | Same existing alert owners | REWRITE test-only saveFailed triggers as command failures; remove the unused production action |
| Closing one document preserves the other document ([FeatureTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopFeaturesTests/FeatureTests.swift)) | The other document's complete state is unchanged; reopening an existing document still invokes focus | Swift reducer | Existing close isolation and existing-document focus tests | KEEP behavioral assertions; remove the assertion on redundant activeDocumentID bookkeeping, which production never consumed |

The focused run passed 26 feature/identity cases, 7 catalog cases and 2 native
export/close/save-failure cases. In a disposable package copy, suppressing alert
creation in `operationFailed` made all three rewritten alert owners fail (13
issues), proving they still detect broken production error presentation. The
production source was not faulted. Regression and sensitivity logs are retained
under `.hitslop/v1-evidence/apple-cleanup/`.

Final validation passed `bun run check`, `bun run test` (126 tests, 161 historical
replay cases and 51 bundled template checks), `bun run build`, `bun run swift:test`
(the three opt-in benchmarks stayed skipped), and `bun run test:native` (8 tests).
Runtime bytes, bridge contracts and release records are unchanged.

### Cache and compiler corrections after the Nx pilot

| Contract | Oracle | Tier | Duplicate-of | Verdict |
|---|---|---|---|---|
| Shared compiler inputs invalidate every template | Skill/config edits cause rebuilds with named miss reasons; restored guidance equals current source; CLI help changes remain hits | Bun cache | Existing sibling isolation test retains one-template coverage | EXTEND; both missing inputs demonstrably returned stale hits before the fix |
| Portable Svelte compilation | Identical component sources built in two checkout locations produce identical JavaScript | Bun build | Existing build checks used one source location | ADD; emitted style hashes differed before the fix |

Focused validation passed all 11 build/cache tests after the fixes. No render cases
or other consequential coverage were removed. Stage timings are observational;
PNG/PDF, separate reopen, and immutable-master assertions remain in place.

### Existing dispositions

| Contract | Oracle | Tier | Duplicate-of | Verdict |
|---|---|---|---|---|
| Browser-normalized text never becomes a draft deletion ([binding tests](../packages/document/tests/bindings.test.ts), [LoroCLITests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopHostTests/LoroCLITests.swift)) | Full text survives flush/reopen when the DOM caps its rendered value; editing the visible prefix preserves the undisplayed suffix | 0 / 1 | Unit owns rebasing; native proves actual WebKit truncation | EXTEND; baseline truncates imported 1.15 MiB text to the input's 524,288-character limit on flush |
| Whole-document JSON import ([import tests](../packages/document/tests/import.test.ts)) | Literal expected values, retained/new IDs, cyclic references, Unicode text, rich text, counter values after reopen, and unchanged state/version after rejection | 0 | Existing batch tests own generic staging; these cases own JSON translation | ADD; regressions exposed slow large-text diff and counter replay rounding (accepted 1 reopened as 0); Unicode-safe splices and import checkpoints preserve the intended results |
| Version-checked snapshots and replacement ([session tests](../packages/document/tests/session.test.ts)) | Stable token across reopen/checkpoint, rejection after another edit or a draft during an awaited save, accepted data retained after failed persistence | 0 | — | EXTEND |
| Imported state readable by historical runtimes ([compatibility tests](../packages/cli/tests/compatibility.test.ts)) | Independent field/reference expectations followed by sealed readers in isolated processes, for both updates and checkpoints; the current reader uses candidate bytes checked against its seal | 0 | Existing corpus covers old authored commands, not the new importer | EXTEND without changing historical fixtures or release records; reproduced and fixed fresh-runner failure when only older readers have restored archives |
| Toolbar callbacks remain safe during asynchronous close ([LoroEngineTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopHostTests/LoroEngineTests.swift)) | Pointer sampling during close completes without a crash; persisted theme remains readable and editable afterward | 1 | Existing close tests checked completion, not native callbacks during storage release | EXTEND existing theme round trip; pre-fix test crashes accessing a destroyed WebView while the session still reports ready |
| JSON import native creation and live editing ([CLI tests](../packages/document/tests/cli.native.test.ts), [LoroCLITests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopHostTests/LoroCLITests.swift)) | Atomic publication, unchanged templates, file/symlink validation, large requests, live input refresh, draft conflict and reopen | 1 | —; native filesystem, transport and WebView boundaries | EXTEND; existing large-attachment round trip also guards linear socket framing after repeated baseline timeouts |
| Catalog discovery, master duplication, source identity and refresh ([LocalTemplateStoreTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopCatalogTests/LocalTemplateStoreTests.swift)) | Constructed catalog inputs, expected discovered packages, unchanged masters and observed refresh | 1 | — | KEEP |
| Decorative catalog emoji mapping (former CatalogEmojiTests) | Exact cosmetic inventory; no platform requirement | — | None needed | DELETE |
| Package paths, symlinks, resource bounds, metadata and supported locations ([Core tests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopCoreTests)) | Valid/invalid packages, traversal and symlink attempts, size limits, immutable master bytes | 1 | —; actual native filesystem validation | KEEP |
| Catalog search/identity and application open, close, quit, retry and alert orchestration ([Features tests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopFeaturesTests)) | Explicit action sequences, expected document/effect state and retained user data | 1 | —; orchestration decisions | KEEP |
| Startup timing measurements ([BenchmarkTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopHostTests/BenchmarkTests.swift), [DocumentStartupTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopHostTests/DocumentStartupTests.swift)) | Report measurements; no hardware-dependent correctness threshold | Opt-in | — | MOVE outside default gate |
| Hidden opening, readiness, focus, cancellation and delayed progress ([DocumentStartupTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopHostTests/DocumentStartupTests.swift)) | Visible/hidden outcome, released ownership and cancelled presentation; controllable deadline | 1 | — | REWRITE scheduling-dependent setup; KEEP native failure/hidden-open cases |
| Live/closed CLI routing, unsupported runtime rejection and headless isolation ([LoroCLITests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopHostTests/LoroCLITests.swift)) | Command results, unchanged packages and authored code that must never execute | 1 | —; real native socket/WebKit | KEEP |
| Theme persistence, committed writes without acknowledgement, save retry and ownership ([LoroEngineTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopHostTests/LoroEngineTests.swift)) | Known saved values after renderer death, independent ownership attempt, retry and reopen | 1 | —; native storage/renderer failure | KEEP; fault injection moved to storage acknowledgement boundary |
| PNG/PDF capture, restoration, publication deadline and export telemetry ([LoroExportTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopHostTests/LoroExportTests.swift)) | Decoded export data, requested bounds/text, retained editor, no expired publication or document values in telemetry | 1 | — | KEEP |
| Native window, toolbar and live CLI integration ([LoroWindowTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopHostTests/LoroWindowTests.swift)); all-template coverage | Minimal contract fixture plus every bundled/preserved package rendering and reopening unchanged | 1 | Shared [native smoke](../scripts/v1/native-smoke.ts) owns corpus breadth | SPLIT template loop from native interaction |
| Frozen slops reopen and accept edits; historical readers understand candidate writes ([compatibility runner](../scripts/v1/compatibility.ts)) | Sealed specimens, literal expected state and scripted outcomes, SQLite close/reopen | 0 | Former RuntimeCompatibilityTests data replay | MOVE to Bun; equivalent artifact hashes are reported, not replayed twice |
| Old authored app works with current native bindings, theme and export ([RuntimeCompatibilityTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopHostTests/RuntimeCompatibilityTests.swift)) | Frozen compiled app, visible edits, persisted theme and completed export | 1 | —; actual authored app/native integration | KEEP native half of SPLIT |
| Habit Heatmap/Pocket Sheet walkthroughs and Soma Amp audio/skin walkthrough | Application formulas, import parser and decorative rendering are outside platform contract | — | Shared corpus smoke covers only build/open/render/reopen | DELETE application-specific assertions |
| Window miniaturization, icon behavior and usable screen geometry ([SlopDocumentWindowTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopHostTests/SlopDocumentWindowTests.swift), [SlopDynamicWindowTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopHostTests/SlopDynamicWindowTests.swift)) | Native behavior and reachable bounds, including document extending above the screen | 1 | — | KEEP behavior; REWRITE duplicated toolbar sizing formula |
| Style-mask constants, editor/menu inventory, fallback-icon plumbing, drag threshold and forwarding call shape | Implementation assertions without additional observable protection | — | Native window/startup/toolbar behavior | DELETE reviewed incidental assertions and drag-forwarding file |
| PNG fidelity and alpha hit testing ([SlopPresentationTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopHostTests/SlopPresentationTests.swift), [SlopWindowMaskTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopHostTests/SlopWindowMaskTests.swift)) | Decoded fixture color/transparency, synthetic alpha masks, click-through holes and cancelled output | 1 | —; native decoding/coordinates | KEEP native fidelity/masks; optimizer-specific coverage retired in the simplification below |
| Editor remains usable after capture, failed render permits retry ([SlopPresentationTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopHostTests/SlopPresentationTests.swift)) | Restored editor behavior and successful capture; explicit failed rendering | 1 | — | REWRITE incidental selectors/styles; KEEP failure cases |
| Toolbar hover and hide deadline allow interaction ([SlopToolbarInteractionTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopHostTests/SlopToolbarInteractionTests.swift)) | Hover/gap-crossing and renewed interaction retain usable controls | 1 | — | KEEP |
| Guest hover controls follow native chrome ([SlopToolbarInteractionTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopHostTests/SlopToolbarInteractionTests.swift)) | Real WebView computed visibility, focus exclusion while hidden, disabled controls, window hiding, export and renderer recovery | 1 | Existing deadline tests cover timing; this case covers native-to-guest delivery | ADD; missing signal makes the pre-fix case fail |
| Only managed templates use working-copy creation ([SlopWorkingCopyTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopRuntimeTests/SlopWorkingCopyTests.swift)) | Managed/unmanaged package inputs and factory outcome | 1 | — | KEEP |
| Attachment bytes, quotas, staging, path safety and native duplication ([AttachmentTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopWasmTests/AttachmentTests.swift)) | Known blob bytes and hashes, malicious paths, interrupted staging, live/closed CLI reads | 1 | —; actual native blob store | KEEP |
| Native download confirmation, destination safety and filenames ([DocumentFileSaverTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopWasmTests/DocumentFileSaverTests.swift)) | Confirm/cancel outcome, unchanged destination after rejection, literal safe names | 1 | — | KEEP |
| Picker delivery, cancellation on close/reload and exactly-once completion ([DocumentFilePickerTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopWasmTests/DocumentFilePickerTests.swift)) | Real temporary-file bytes reach HTML input; cancellation/completion and concurrent rejection | 1 | Former SomaAmp picker cases | MOVE/REWRITE at platform boundary; remove app fixture |
| Runtime selection, busy ownership and rejection before creating state ([RuntimeCatalogTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopWasmTests/RuntimeCatalogTests.swift)) | Declared contract/minimum revision, unchanged package, competing writer | 1 | —; native runtime selection | KEEP |
| Cancelled open releases acquired lease; prewarm releases WebView ([RuntimeCatalogTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopWasmTests/RuntimeCatalogTests.swift)) | Acquired lease then cancellation and successful reopen; completion and released view | 1 | — | REWRITE shared-queue semaphore and process-global polling |
| Bridge envelopes, bounded clients, snapshot isolation and scheme safety ([WasmTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopWasmTests/WasmTests.swift)) | Malformed requests, independent writer, unchanged masters, frozen rows/theme, symlink replacement | 1 | —; real native transport/storage | KEEP |
| Build package excludes engine/source and handles fresh authoring inputs ([build.test.ts](../packages/cli/tests/build.test.ts)); native rendering/registration ([build.native.test.ts](../packages/cli/tests/build.native.test.ts)) | Built artifact contents, rejected direct engine imports, valid rendered output and intact registered master; a fixture token resolves to its independently specified color at mount | 0 / 1 | —; native rendering and theme readiness have distinct risks from controller persistence | KEEP Bun checks; MOVE native cases; EXTEND plain DOM case to cover theme readiness without a generated theme stylesheet |
| CLI parsing/help, native forwarding, relocated helper discovery and native document commands ([CLI tests](../packages/cli/tests), [cli.native.test.ts](../packages/document/tests/cli.native.test.ts)) | Literal argv, JSON, exit status and rejected malformed/unsupported input before mutation | 0 / 1 | — | KEEP pure commands; MOVE native-dependent commands |
| Preview title cannot inject executable markup ([preview.test.ts](../packages/cli/tests/preview.test.ts)) | Parsed output with malicious title has no injected executable element | 0 | — | REWRITE escaping check; DELETE literal CSS/shape strings |
| Template discovery, selection, hygiene and skills installation ([repository.test.ts](../packages/cli/tests/repository.test.ts), [skills.test.ts](../packages/cli/tests/skills.test.ts)) | Controlled manifests/packages, missing links, conflicting directories, unchanged user files | 0 | — | KEEP |
| Runtime catalogs, sealed release history and capabilities ([runtime-artifacts.test.ts](../packages/cli/tests/runtime-artifacts.test.ts), [runtime-capabilities.test.ts](../packages/cli/tests/runtime-capabilities.test.ts), [compatibility.test.ts](../packages/cli/tests/compatibility.test.ts)) | Independent base commit, altered bytes/requirements, incorrect expected data and no-op compiled handle | 0 | —; guards release integrity and false-green replay | KEEP; history guard rejects candidate rewrites |
| Cache invalidation, isolation and safe publication ([template-cache.test.ts](../packages/cli/tests/template-cache.test.ts)) | Changed/damaged input, unaffected sibling cache, failed builder leaves no published entry | 0 | — | REWRITE generated-template dependency with minimal valid package |
| Template cache keys follow compiler inputs and name miss causes ([template-cache.test.ts](../packages/cli/tests/template-cache.test.ts)) | Template/toolchain edits report the changed input; compiler files keyed, CLI routing/help/skills/scripts not; old-format entries pruned | 0 | — | EXTEND; CLI help edits no longer rebuild the corpus |
| Full-corpus build, `check:built` and bundled/preserved-template render ([native smoke](../scripts/v1/native-smoke.ts)) | Every bundled and preserved package opens, renders PNG/PDF and reopens unchanged | 1 | Everyday `native` keeps fixtures + contract specimens (`test:render --fixtures`) | MOVE from every push to the release workflow; coverage unchanged per release |
| Host death after acknowledged write ([crash matrix](../scripts/v1/crash-matrix.ts)) | Acknowledged CLI edit survives killing the host | 1 | — | MOVE on tags from the Debug app to the signed Release candidate, before notarization |
| Attachment references follow durable blobs, close waits, retries preserve intent ([attachments.test.ts](../packages/document/tests/attachments.test.ts)) | Known bytes, explicit faults, retained/rejected operations and expected MIME/session behavior | 0 | —; document attachment lifecycle | KEEP |
| Offline convergence, identity and invalid remote state ([convergence.test.ts](../packages/document/tests/convergence.test.ts)) | Literal merged outcomes, stable IDs, rejected malformed updates and reopened state | 0 | — | KEEP |
| Typed handles, transaction atomicity and identity-bearing composites ([handles.test.ts](../packages/document/tests/handles.test.ts)) | Known values/IDs; rejected writes leave state and selection unchanged | 0 | Former DSL and slop identity assertions | KEEP owner; FOLD optional/nested identity cases |
| Binding composition, Unicode edits, unfinished gestures and failed-save retry ([bindings.test.ts](../packages/document/tests/bindings.test.ts)) | Literal Unicode/caret results, flushed values after reopen, retry retains edits | 0 | Former dsl-regressions and template gesture tests | FOLD into shared owner; DELETE private UndoManager assertion |
| Malformed saved state and byte decoding ([open.test.ts](../packages/document/tests/open.test.ts)) | Constructed invalid checkpoint fields, known empty/odd/large byte sequences | 0 | — | KEEP |
| Session epochs, flush-on-get and cancellable/concurrent close ([session.test.ts](../packages/document/tests/session.test.ts)) | Stable lifetime identity, rejected stale commands, retained writes before final close | 0 | — | KEEP |
| Storage durability, ownership, compaction, lost replies and package relocation ([storage.test.ts](../packages/document/tests/storage.test.ts)) | Known state/IDs after reopen, explicit I/O failures, independent writer attempt | 0 | Former slop persistence cases | KEEP owner; FOLD generic persistence invariants |
| Theme writes and rejection ([theme.test.ts](../packages/document/tests/theme.test.ts)) | Known persisted overrides; invalid tokens/values rejected without saving | 0 | — | KEEP |
| View flush, capture and close lifecycle ([view-lifecycle.test.ts](../packages/document/tests/view-lifecycle.test.ts)) | Required ordering and retained edits/editor through capture or failure | 0 | — | REWRITE complete private call traces; KEEP consequential ordering |
| Vocabulary, tree/list/record identity, previews and seeded edits ([vocabulary.test.ts](../packages/document/tests/vocabulary.test.ts)) | Literal outcomes and independent model, public snapshots and reopen | 0 | Former template clear/move cases | KEEP/FOLD semantics; REWRITE private projection oracle |
| Manifest and socket schemas ([schema tests](../packages/schema/tests)) | Independent valid/invalid envelopes and explicit required fields | 0 | —; TypeBox contract differs from native decoding | KEEP |
| Habit Heatmap, Doodle Board, Side Quest, Pocket Sheet and Soma Amp business rules | Application formulas, drawing geometry, skin parser and radio selection are outside platform requirements | — | No equivalent application proof claimed; shared owners above retain document invariants | DELETE application-only cases after FOLD |

## Preserved implementation evidence

The entries below record earlier implementation and release checks, including the
retired contract-1 baseline. They are historical evidence, not acceptance results
for the current contract-2 launch candidate. Current release acceptance must refer
to the final candidate commit and its own gate report.

These results describe the approved initial cleanup, before the runner simplification below. Historical replay counts include byte-identical candidate/release executions; current reports deduplicate them.

### Baseline validation

- Baseline targeted document/schema run: 105 passed in 4.73 seconds, existing helper, no Swift rebuild.
- New compiled-runtime replay: existing fixture matched independent expected state; update and checkpoint paths reopened under candidate and eligible released reader (4 cases).
- All 51 bundled templates compiled and opened/reopened with the candidate in Bun.
- Cache isolation rewrite: 3 tests passed; no generated/native template dependency.
- Baseline RuntimeCatalogTests: 5 passed; Swift build 51.76 seconds, tests 1.879 seconds. One successful run does not establish absence of flakiness.
- Full native baseline: 129 test cases passed (including opt-in skips); suite times 18.903s, 0.005s, 181.452s, 0.350s, 0.166s, 0.317s. Slowest default case: bundledExamplesRenderInNativeWindows, 47.961s.
- Replay mutation checks pass: corrupted expected state and a no-op compiled text handle are caught by state assertions, independently of the seal. A release rewrite committed in a temporary repository and a changed fixture fail against its earlier commit.
- Both crash adapters pass all five boundaries: held writer, pre/post append commit, pre/post checkpoint commit.
- Repeated cancellation-suite baseline: five additional runs passed (5/5); this does not prove the scheduling race is absent. At this baseline stage, final validation and per-invariant fault checks were still pending; completed evidence appears below.
- Existing staged Swift renderer/storage changes belong to the user and are retained.

## CI contract

Fast always runs. Native uses job-internal path filtering and must report even when unaffected. Both check names must become required after the new workflow has reported on the remote. Release validation runs on master/manual and inside the tagged signing workflow. Workflow changes alone do not modify GitHub branch protection.

## New coverage added during implementation

- `packages/cli/tests/compatibility.test.ts`: KEEP both replay-oracle faults and committed-history rewrite cases; independent altered expected values and earlier Git commit, compiled replay/history API, no authored slop behavior.
- `tests/compatibility/1-1-vocabulary`: additional frozen conformance specimen created with the verified sealed 1/1 runtime, covering rich text, counters, bounded integer, scalar list, record, tree and object row. Original 1-1 bytes and expected state remain unchanged.

## Implementation evidence

- Approved application-only files removed: Habit Heatmap, Doodle Board, Side Quest, Pocket Sheet and Soma Amp TypeScript suites; Habit Heatmap/Pocket Sheet Swift walkthroughs. No replacement claims coverage of their formulas, skin parser or decorative appearance.
- `dsl-regressions.test.ts` folded into `bindings.test.ts`, `handles.test.ts` and `storage.test.ts`. Public bindings cover Unicode composition and gesture flush/retry; handles cover optional/nested collection identity. Private UndoManager assertions were removed; public atomicity remains.
- `test-sensitivity.ts` passed all four disposable source faults: no-op text replacement, no-op Unicode splice, skipped preview flush and allowed replacement of identity-bearing collections. Evidence: `.hitslop/v1-evidence/test-sensitivity.json`.
- Compiled-runtime sensitivity tests passed: incorrect expected state and a disabled handle edit both fail the replay. Independently committed fixture/release changes fail against the trusted base. This includes edits already committed in the candidate, which the former HEAD-only comparison missed.
- The richer conformance specimen was seeded with the sealed 1/1 runtime, not a newly modified source engine. Both specimens pass eight update/checkpoint/current/historical cases. Two fresh runs produced identical logical state hashes (`replay-first.json`, `replay-second.json`). Original 1-1 package, expectation and seal remain unchanged.
- Default Bun tests after folding: 103 passed, zero failures. Cache tests use valid minimal packages; preview tests observe parsed executable injection rather than literal CSS. Template compilation/open/reopen is a separate generic corpus pass in the same command.
- Native fault tests distinguish write rejection from committed writes whose acknowledgement is lost, including renderer death. All three targeted storage-boundary integrations passed.
- The old queued-cancellation flake was not reproduced in five baseline repeats; the shared queue scheduling hazard was established from its synchronization, not from an invented failure rate. The replacement proves acquired ownership, cancellation release and successful reopen. Per-instance prewarm completion also passes (five catalog cases total).
- Baseline Swift suite passed: 129 cases across targets, including disabled opt-in walkthrough declarations. Its host group took 181.45 seconds. New timing is recorded only after a comparable final run; removing test files does not itself prove a speedup.
- Both SQLite adapters passed all five crash-matrix phases before the duplicate scripts were replaced by thin entry points.

Validation artifacts are local/CI output, not committed golden files. The release report records the final gate's actual successes and failures. The contract table groups the approved dispositions; folded/deleted files are intentionally absent from the final tree.

- Retained Soma Amp platform behavior now lives in `DocumentFilePickerTests.swift`: close/reload cancellation and exactly-once completion/concurrent-request rejection both pass. The app walkthrough and unused WSZ test asset were removed afterward.
- The replacement window/export test passed in 4.70 seconds (old template loop: 47.96 seconds). Frozen-app native integration passed in 4.20 seconds; the three presentation kinds retained usable editors after capture. The controllable progress-delay test passed without a wall-clock minimum.
- Swift renderer validation initially exposed an inappropriate animation-frame wait in an invisible test WebView; replaced with completion of the actual asynchronous UI turn. Concurrent export checks also hit the existing 30-second publication deadline; the final release gate runs expensive stages sequentially rather than relaxing that contract.

- The 51 identical per-template typecheck invocations were consolidated into one discovered-source Svelte program under the same repository compiler settings; all active sources pass with zero errors/warnings. Pure Bun test-only changes no longer trigger native CI, while native test/fixture changes do.

- The picker suite also retains the real HTML upload boundary: a selected temporary file reaches WebKit with its original bytes, and capture/disabled sessions cancel selection. This covers the platform invariant formerly buried in the skin-import walkthrough, without importing Soma Amp or its fixtures.

- First complete gate: all 51 native template builds, source checks, 103 Bun tests and 51 Bun template reopens passed. Swift exposed an incorrect theme oracle (the fixture already used the chosen accent) and a toolbar reachability gap when a borderless document extends above the visible screen. The theme test now compares reset with the original public theme using a distinct override. The toolbar now clamps to the upper visible edge as well as the lower edge; coverage retains the original cases plus this consequential straddling-display case, without duplicating the sizing formula. The failed attempt is retained as `release-check-first-attempt.json`; the final gate reuses that completed build.

## Final validation

`bun run release:check --built` passed after the completed full `bun run build` was reused. The report explicitly records `dirty: true` and `reusedBuild: true`; no commit, publication or fixture sealing was performed by this cleanup.

- `check`: passed, including the consolidated Svelte program (zero errors/warnings).
- `test`: 103 tests, eight compiled-runtime replay cases and 51 bundled template open/reopen checks passed, with no Swift build in this command.
- `swift:test`: 120 cases across targets passed. The host target took 98.01 seconds versus 181.45 seconds in the baseline; these are local observations, not a controlled benchmark.
- Native CLI: six cases passed. Generic native smoke: all 53 packages passed, retaining 106 PNG/PDF artifacts and confirming unchanged master bytes/state after reopen.
- Both crash adapters, actual host death, relocated helper, packed npm consumers, landing check/build and Apple build/installed-artifact validation passed.
- Two final compatibility replays produced identical logical state hashes. All four opt-in source faults and both normal compatibility sensitivity/history tests passed.
- `git diff --check` and workflow/action YAML parsing passed.

The final report is `.hitslop/v1-evidence/release-check.json`; native render evidence is under `.hitslop/v1-evidence/render/`. Cold local `test` took about 6.8 minutes, so the aspirational seconds-long tier-0 target is not established. CI timings remain to be measured. The workflow is ready for `fast` and `native` branch-protection requirements after those checks have reported; remote branch protection was not changed from this uncommitted working tree.

## Follow-up review of native test seams

No Swift code was changed in this simplification. These are scoped findings, not new deletion approvals:

- **Storage acknowledgement boundary:** `WasmSession` uses `StorageBridge.call` in production. Its `beforeWrite`/`afterWrite` hooks are test-only, but the three retained `LoroEngineTests` exercise distinct pre-commit rejection, lost acknowledgement and renderer death after commit. Keep this narrow per-instance fault mechanism for now; moving the hooks alone is not a design improvement. Any future consolidation must preserve those observable outcomes and demonstrate fault sensitivity before removing the hooks.
- **Prewarm completion:** `RuntimePrewarm.waitUntilFinished()` has only the catalog test as a caller and adds stored result/completion state plus a continuation list. Review this lifecycle as one follow-up: simplify completion bookkeeping while preserving runtime compilation and WebView release. The unconsumed static `isRunning`/`outcome` diagnostics were removed in the focused pre-launch cleanup; the completion lifecycle remains unchanged. Do not replace it with process-global polling.
- **Opening feedback:** the injected deadline wait controls a real clock boundary. `SlopOpeningProgress.waitForFeedback()` only serves the startup test. A focused follow-up should determine whether the existing completion boundary can prove delayed feedback, cancellation and late callback suppression without a separate test-only waiting method; preserve the behavioral test and avoid minimum wall-clock assertions.
- **Cancelled preparation:** production `open` calls `prepare` and `finishOpening`; the catalog test also calls the two stages directly. Keep the cancelled acquired-lease/reopen proof. Review stage visibility together with that test if opening is refactored, without restoring the shared preparation-queue semaphore.

## Runner simplification validation

- `bun run check`: passed, including zero Svelte errors/warnings.
- Focused compatibility tests: two passed; wrong expected state and the disabled compiled handle still fail, and the same document reopens after a failed assertion in the shared process.
- All four existing source fault checks passed (`test-sensitivity.json`).
- Two fresh corpus runs produced identical logical hashes, with four executed update/checkpoint cases. Each records byte-identical release `1-1` under `equivalentReleases`; the removed four historical executions provided no distinct-reader evidence.
- The isolated worker accepted a two-case batch, rejected wrong expected data with the document and phase in the error, and released ownership for a subsequent reopen. This validates batch plumbing with the existing runtime, not compatibility with an unshipped revision.
- Evidence: `.hitslop/v1-evidence/replay-simplified-first.json`, `replay-simplified-second.json`, and `runner-simplification.json`.
- Final `bun run test`: 103 tests, four compiled-runtime replay cases and all 51 bundled template compile/open/reopen checks passed without a Swift build. Full corpus report: `.hitslop/v1-evidence/compatibility.json`.
- `bun run hygiene`, targeted Prettier check, ledger-link validation and `git diff --check` passed. `CLAUDE.md` remains exactly `@AGENTS.md`.
- This batch changes no application production code or Swift. Runner tooling is +58 net lines; the existing compatibility test is +8 net lines for distinct mutant-artifact isolation, failure context and same-process reopen proof. No tests were deleted. Guidance and the ledger are shorter overall; no commit or publication was performed.

## Apple telemetry pass

| Coverage | Independent oracle | Removed tests | Equivalent owner | Disposition |
| --- | --- | --- | --- | --- |
| Guest/startup/renderer diagnostics (`LoroEngineTests`, `DocumentStartupTests`) | One fixed authored category, no private fields; duplicate renderer callbacks count once until ready | 0 | — | ADD gaps; guest and renderer tests failed before the fix |
| Save/quit propagation and duplicate rejection (`LoroEngineTests`) | One save incident, writer remains locked after failed quit; rejected destination leaves source usable | 0 | Existing native storage fault boundary | EXTEND |
| Export/create telemetry (`LoroExportTests`, `LocalTemplateStoreTests`) | Completed operations count once, cancellation never succeeds/fails, rejection retains format/category | 0 | Existing host/service boundaries | EXTEND payload assertions, ignore lifecycle breadcrumbs in success counts |
| Upload privacy and secondary budget (`SlopTelemetryTests`) | Literal permitted keys and stable code; at most two secondary reports, none after foreground platform failure | 0 | — | ADD new reporting contract |

Validation for this telemetry pass: source checks, all 51 template builds, 103 Bun tests, four compatibility replays, 51 Bun open/reopens, 125 Swift tests, and six native CLI tests passed. A final 13-case targeted Swift run passed after the live CLI export and cancellation changes. `git diff --check` passed.

Actual Firebase validation used a signed arm64 Release AppKit harness with the production reporting adapter, configuration, and dSYM upload script. The fatal crash and four final non-fatal categories arrived with symbolicated frames and correct build numbers; the four categories have distinct issue IDs, and export format did not leak into other reports. Common-domain reports initially grouped together despite different numeric codes; fixed category-specific domains were verified against the backend. No crash trigger is shipped. The full production app Release build was stopped during universal dependency compilation, so this is delivery/symbolication evidence for the reporting harness, not a complete production Release build.

Native render smoke completed with 51 of 53 packages passing and is not green: School Schedule PNG export repeatedly timed out with both the candidate and the pre-existing application helper on copies of the same package. Side Quest also hit the PNG deadline. Deadlines and assertions remain unchanged. The local telemetry report records the completed corpus and all retained failures: `.hitslop/v1-evidence/telemetry/validation.json`, with sanitized Firebase events, dSYM, logs, and baseline comparison alongside it. No template logic, sealed runtime bytes, or release records were changed by this pass.

## Typed platform contracts

| Contract | Oracle | Tier | Duplicate-of | Verdict |
| --- | --- | --- | --- | --- |
| Socket epochs and method-specific bridge types | [Compile-only checks](../packages/document/tests/platform-contracts.types.ts): missing edit epoch, missing nullable error and wrong reply fields fail type checking | Bun check | — | ADD compile-time boundary proof; no runtime declaration tests |
| Installed SDK resolves platform types | [Packed consumer](../scripts/v1/packed-test.ts) imports the adapter and socket/bridge types in an isolated project | Packed | — | EXTEND existing packaging owner; resolve the candidate schema tarball transitively |
| Attachment discovery works over live and closed command transport | [AttachmentTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopWasmTests/AttachmentTests.swift): exactly one blob, independently computed SHA-256 and known byte count | Swift | Existing put/read case | EXTEND with list calls in both modes |
| Stale socket edits never apply; refusal codes preserve retry guidance | [LoroCLITests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopHostTests/LoroCLITests.swift): literal stale epoch and code, unchanged state; controlled transport replies produce independently specified CLI guidance | Swift | Removed-flag case does not reach transport | EXTEND real socket coverage and ADD reply-code transport table, with no production hooks |
| Configured template masters remain immutable | [CLI native owner](../packages/document/tests/cli.native.test.ts): valid apply refuses with “writable copy,” creates no state and preserves every original file | Native Bun | — | ADD regression: baseline incorrectly returned exit 0; fixed helper passes all 9 assertions |

Runtime seals, schema-envelope tests and template-location predicate tests remain their existing owners. Transport fault sensitivity is recorded in `.hitslop/v1-evidence/contract-sensitivity.json`; mutants live only in a disposable package copy.

Validation: schema generation/drift and source checks passed; 103 Bun tests, four compatibility replays, all 51 template builds, 127 Swift tests, seven native CLI tests, relocated-helper checks, packed-package consumer checks, and all 53 native render smoke packages passed. The candidate remains byte-equivalent to release `1-1`; `headless.js`, validator schemas and release records are unchanged. All three transport mutants failed their owning tests, and the disposable generated source was restored before removal. The configured-master regression failed on the baseline because apply incorrectly exited 0, then passed after the shared predicate fix.

Initial attachment/PNG export timeouts were retained in the evidence; focused and complete reruns passed without changing deadlines or assertions. Choice Point PNG export also passed with both the pre-change release helper and the candidate in isolation. Final command results and logs are recorded in `.hitslop/v1-evidence/contracts/validation.json`. Neither npm package has a published `1.0.0`, so this change keeps their initial versions and does not publish them.

## Faster local builds and native PNG output

| Contract | Oracle | Tier | Duplicate-of | Verdict |
| --- | --- | --- | --- | --- |
| Native PNG fidelity, transparency and capture restoration | Existing presentation/export owners decode known fixture colors, transparent holes, dimensions and retained editor behavior | Swift | Retires SlopPNGTests' optimizer-specific round trips | REPLACE custom optimization coverage with real encoder/capture coverage; disposable color and alpha faults both fail the presentation owner |
| PNG compression ratio and custom chunk/filter rewriting | Smaller byte count is no longer a product requirement; native encoder output is accepted within existing bounds | Swift | — | REMOVE optimizer and its private format tests; keep window-mask, PNG/PDF, cancellation and publication-deadline owners |
| Installed helper refuses bundled template mutation | Existing relocated-helper owner: refusal with writable-copy guidance, no state directory, unchanged complete master digest | Native helper | Configured-root CLI regression covers a different root | EXTEND; baseline failed because apply returned exit 0, before changing the predicate |
| Fast packed consumers need no native renderer | Existing packed owner with a deliberately unavailable helper, real tarball installation, type checks, init and preview | Packed | Full native mode includes these checks | SPLIT modes; preserve full build/register/theme/PNG/PDF workflow under --native in release validation |
| Local native owners require only their specimens | Existing native owners consume two black-box apps and three presentation fixtures; full corpus remains explicit and in relevant PRs | Native/CI | — | SPLIT routine build from cached template builds and render smoke; keep complete PR/release coverage |

PNG fault evidence is in `.hitslop/v1-evidence/simplification/png-sensitivity.json`.
Mutants changed only disposable copies of the dedicated presentation fixture. Neither
fault changed production code or committed fixture bytes. The template-master baseline
failed at the new existing-owner assertion before the native predicate fix.

Validation: source/generated checks, 103 Bun tests, four compatibility replays, all
51 template compile/open/reopens, 122 Swift tests, seven native CLI tests, relocated
helper checks, both packed modes, built-package bounds, and all 53 native render
specimens passed sequentially without retries or deadline changes. The five retired
Swift cases belonged to the removed private PNG optimizer. Runtime release `1-1`
remains byte-equivalent; sealed headless bytes and release records are unchanged.

Measured on this checkout: the routine incremental build took 19 seconds; template
artwork took 461 seconds with 49 builds/two hits, then 3 seconds with all 51 cache
hits. Packed consumer checks took 40 seconds, or 59 seconds with native coverage;
the complete render sweep took 171 seconds. Cold template compilation and the full
Bun compatibility corpus still take minutes. Three sampled PNG exports fell from
4.7–15.8 seconds to 0.9–1.2 seconds. Their files grew 1.6–1.8×, with zero differing
decoded pixels across 7,988,800 pixels. Evidence and command logs are retained in
`.hitslop/v1-evidence/simplification/validation.json`. This is scoped validation;
the complete signed-app release gate was not run.

## Focused pre-launch cleanup

| Contract | Oracle | Tier | Duplicate-of | Verdict |
|---|---|---|---|---|
| Resource scheme isolates private files ([WasmTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopWasmTests/WasmTests.swift)) | Encoded traversal and noncanonical paths return no bytes; ordinary package/runtime resources still load; headless rejects authored assets | 1 | — | EXTEND; baseline served private marker bytes |
| Manifest controls bridge resizing ([WasmTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopWasmTests/WasmTests.swift)) | The same real WebKit request succeeds for a resizable manifest and is rejected for a fixed manifest | 1 | —; native style tests do not exercise bridge authorization | ADD; fixed manifest accepted request before fix |
| Saved downloads are quarantined before publication ([DocumentFileSaverTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopWasmTests/DocumentFileSaverTests.swift)) | Read quarantine metadata on new and replaced files; injected metadata I/O failure preserves old bytes and publishes no new file | 1 | — | EXTEND; baseline lacked metadata and ignoring metadata-write failure published output |
| SQLite backup does not follow a replaced source symlink ([SlopPackageTests](../apps/apple/Packages/HitSlopApple/Tests/HitSlopCoreTests/SlopPackageTests.swift)) | Database/state-directory symlinks are refused without creating output; ordinary SQLite input through the system /var alias preserves a literal marker row | 1 | —; package validation occurs before the backup open | ADD; baseline followed the symlink and created output |
| Published SDK excludes test-only persistence adapters ([packed consumer](../scripts/v1/packed-test.ts)) | Installed tarball has neither test-support nor the former adapter source paths; SDK imports still work | Packed | — | EXTEND; old tarball fails on shipped sqlite.ts |

No consequential tests were removed. Existing close/export tests now use the identical `finish()` wrapper. Bun SQLite/flock fixtures moved to `packages/document/test-support`; durability, crash and compatibility tests retain their behavior and ownership checks. The quarantine dependency is a per-call filesystem operation, with no global test mode or altered production save ordering.

Validation: the complete `bun run release:check` passed, including 103 Bun tests,
125 Swift tests, seven native CLI tests, all 53 render specimens, storage/crash
recovery, installed npm consumers and the packaged Mac app. Both final compatibility
replays produced identical state and remained equivalent to release `1-1`; sealed
runtime bytes and release records are unchanged. The relocated helper and app
packaging checks pass without the deleted placeholder resource bundle.

Separate command checks confirm `open-dev` is absent in debug and release, and
`storage-probe` exists only in debug. A live preview smoke check confirmed the
listener binds only to `127.0.0.1`. Baseline regression failures and focused fix
results are retained in `.hitslop/v1-evidence/focused-cleanup/`; the complete gate
report is `.hitslop/v1-evidence/release-check.json`. The first gate attempt caught
two pre-existing synthetic home-directory paths in telemetry tests; replacing
those fixture strings preserved their privacy assertions and cleared hygiene.

## Initial v1 bundled release fixtures

| Contract | Oracle | Tier | Duplicate-of | Verdict |
|---|---|---|---|---|
| Shipped bundled packages remain readable and renderable after authoring sources change | Immutable compiled package bytes and seal, captured initial state, successful PNG/PDF export and unchanged state after reopen | Bun compatibility + native render | Current template checks cover current sources, not retained release bytes | ADD 51 initial bundled specimens through `fixtures:seal --write`; preserve existing conformance fixtures and runtime 1/1 ledger unchanged |

These are black-box release specimens, not template business-logic tests. Existing
compatibility and render runners discover them; no new test runner or app-specific
assertions are added.

## Clean-checkout CLI help

| Contract | Oracle | Tier | Duplicate-of | Verdict |
|---|---|---|---|---|
| Help/version terminate without executing skills maintenance | Source-only CLI copy with installed dependencies and no generated skills returns exit 0, help/version output, and empty stderr, including the skill alias and update help | Bun CLI | Existing case used the developer checkout and could inherit generated skills | EXTEND existing command test; baseline fails with SkillSourceUnavailableError after printing help; run link maintenance only after completed skills actions |
| Root `--version` and `-v` identify the installed CLI | Output is exactly `slop v` followed by the package metadata version | Bun CLI | Existing help/version case only required output containing `slop`, allowing help to conceal the version | EXTEND existing case; baseline prints help instead of version; register version before the branch help fallback |

## Guided CLI creation and Crust 0.4 skills

| Contract | Oracle | Tier | Duplicate-of | Verdict |
|---|---|---|---|---|
| Successful init emits valid metadata, rejects invalid flags before writes, and retains deterministic unattended defaults ([commands](../packages/cli/tests/commands.test.ts)) | Parse emitted manifests through the independent platform validator; explicit expected metadata and absent invalid destinations | Bun CLI | Existing build case covers only the valid directory `starter` | EXTEND; regressions failed before their fixes because `init a` returned success with an invalid manifest and a 90-character directory name exceeded the title limit |
| Interactive brief/author answers reach BRIEF.md and the manifest; cancelling creation leaves no destination ([init](../packages/cli/tests/init.test.ts)) | Actual prompt IO supplies the brief and author; title/categories/description keep their agent-replaceable placeholders; Ctrl+C at the author prompt leaves no project | Bun terminal | Subprocess command cases run non-interactively; category limits are owned by the flag cases | ADD; exercises the user input boundary without prompt-frame snapshots |
| Agent selection hands off the project without shell evaluation or automatic permission changes ([agents](../packages/cli/tests/agents.test.ts)) | Fake executables report actual argv/cwd; known/custom prompt arguments, brief instructions, no launch for opt-out/CI/non-TTY, and retained project after failure | Bun process | — | ADD; no real agent or account is invoked |
| Skills maintenance respects scope, survives source eviction, and supports repair/uninstall/aliases ([skills](../packages/cli/tests/skills.test.ts)) | Real CLI entrypoint leaves the other scope's link unchanged, guides remain readable after deleting the package, repair restores dangling links, uninstall removes only its scope | Bun CLI | Low-level Crust link test still owns conflict refusal and relative/global link formats | EXTEND app-only harness to shipped entrypoint; original cache hook demonstrably rewrote a global link during project-scoped installation |
| Update notices cannot contaminate machine output or fail offline work ([updates](../packages/cli/tests/updates.test.ts)) | Isolated registry stub sees no requests for piped/CI/opt-out runs; successful JSON output, one cached interactive notice, and successful offline commands | Bun process | — | ADD; fault injection stays at registry I/O in disposable subprocesses |

Existing skill link, deterministic artifact, and help/version assertions remain.
No consequential coverage was removed. The two bug regressions were observed
failing for the intended reasons before their production fixes, as did the
long-directory title fallback.

Validation: `bun run check`, `bun run test` (126 tests, 161 compatibility replay
cases, and 51 bundled template open/reopen checks), `bun run packages:pack`,
`bun run test:packed`, `bun run landing:check`, and `bun run landing:build` passed.
The interactive init cases also passed with the enclosing runner marked as CI.
Agent launch tests use disposable executables, not real provider sessions.
Native runtime sources and sealed release records were not changed.

## Independent CLI 1.2.0 release

| Contract | Oracle | Tier | Duplicate-of | Verdict |
|---|---|---|---|---|
| Init pins the project's document SDK separately from CLI tooling ([init](../packages/cli/tests/init.test.ts)) | Generated dependency equals the SDK package version; dev dependency equals the CLI package version | Bun CLI | Existing init checks covered manifest metadata, not differing package versions | EXTEND; with CLI 1.2.0 the baseline requested document 1.2.0 instead of 1.1.0; observed failure before the dependency fix |
| Packed projects install and build with independently versioned SDK and CLI ([packed consumer](../scripts/v1/packed-test.ts)) | Emitted SDK and CLI pins match their independently packed manifests, followed by consumer install/check/build | Packed consumer | Existing test already asserted the SDK pin and ran the workflow | EXTEND with CLI pin assertion; retain existing install/build and runtime identity checks |

No consequential coverage was removed. The targeted init regression passed after
the fix. Full release validation is retained against the final commit in the
CLI GitHub Release evidence rather than recording a dirty-tree run here.
