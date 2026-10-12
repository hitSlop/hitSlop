# macOS image widget qualification

Measured 2026-10-11 local time (UTC timestamps below are 2026-10-12), on macOS
26.6.2 (25G83), Apple silicon, Xcode 27.0 (27A266a). Local spike sources live in
`spikes/widget-images`; the original worktree is `/private/tmp/hitslop-widgets`.
This is platform-boundary evidence, not qualification of the product integration.

The signed host published three immutable PNGs and atomically replaced a generation
pointer in an App Group. A separately sandboxed reader decoded all three. The actual
WidgetKit extension also read the group and produced its timeline. Rejecting an
incomplete generation kept the previous pointer unchanged.

The corrected 164×164, 344×164 and 344×344 logical canvases captured at 2× in 0.992s
total. PNG sizes were 23,277, 38,317 and 47,919 bytes. Generation
`49BB124D-1A5D-4954-A656-89EC370F9F4D` published and requested reload at
04:24:31Z; WidgetKit read it at 04:24:32Z. This measures one timeline read, not a
guaranteed visible refresh rate or ongoing system budget.

A background activity requesting a 60s interval with 10s tolerance started at
04:20:39Z and fired at 04:21:50Z. It captured in 1.248s and published at 04:21:52Z
with zero visible host windows. The probe then exited. A production scheduler uses
longer, tolerant intervals and only documents represented by active widgets.

The first placed widget was solid gray except while dragging. It had successfully
read the image: macOS accented rendering flattened the artwork. The corrected view
uses `.widgetAccentedRenderingMode(.fullColor)`, disables content margins, provides a
clear nonremovable container background and lets WidgetKit own outer clipping. Matching
the large canvas aspect ratio removed letterboxing. The user confirmed the improved
placed large widget. All three families and all appearance modes still need review.

The host and sandboxed extension used the Mac team-prefixed App Group form described
in [Apple's App Group guidance](https://developer.apple.com/documentation/xcode/accessing-app-group-containers).
Manual Developer ID signing by certificate identity passed strict verification.
This was a local build without notarization or a secure timestamp; distributed builds
and the eventual iOS provisioning setup are not qualified by it.

An offscreen `requestAnimationFrame` readiness wait stalled. Bounded timer-based layout
settling worked, matching the production capture controller. WebKit's snapshot width
also interacts with display scale; production output should explicitly normalize PNG
pixel dimensions.

The spike used controlled HTML, not a `.slop` document. It did not prove a document
picker, bookmark relocation, real click-to-open, sustained refresh budgets, memory peaks,
sleep/wake, or iOS background rendering. Those remain integration checks. No application
tests were selected by the earlier verification runs because the spike was ignored.

The image modifier follows [Apple's accented rendering guidance](https://developer.apple.com/documentation/WidgetKit/optimizing-your-widget-for-accented-rendering-mode-and-liquid-glass).
Capture requests and visible timelines remain separate under
[WidgetKit's refresh policy](https://developer.apple.com/documentation/widgetkit/keeping-a-widget-up-to-date/).

## Document integration checkpoint

The `feat/slop-widget-documents` branch adds the dedicated role, typed viewport and
refresh declaration, native saved-state capture, atomic cache, AppIntents document
picker, bookmark opening handler and optional running-app scheduler. The Xcode widget
product contains the extracted `SlopWidgetIntent`, entity and query metadata.

The native boundary tests captured all three family sizes from saved document state,
refused a document without the widget role, retained an earlier generation after an
incomplete publication, distinguished a copied file, followed a rename and refused a
replacement at the previous location. Both focused tests passed. `bun run verify`
and `bun run verify --native` passed; the latter ran 218 Swift tests and 26 native CLI
tests, with three existing native scenarios skipped by their normal gates.
Five focused WebKit browser checks also passed, including the bundled Svelte widget
mounting without the editor and rejecting an absent widget capture target.

A manual native renderer read a newly created Quick Checklist `.slop` and produced
328×328, 688×328 and 688×688 PNGs. The three images were visually inspected. Their
sizes were 28,737, 43,688 and 90,922 bytes. Local review files are under
`.hitslop/widget-review/`. This verifies the real embedded Svelte view and native PNG
path; it does not substitute for testing the production picker and taps in an installed,
signed app. Signed product desktop qualification and iOS remain open work.


## Review refinements — 2026-10-12

The close regression was reproduced with the original implementation: an unchanged
widget document changed both bytes and modification time and published again. The
queue regression was reproduced separately: a held scheduled renderer prevented an
unrelated document from completing close. Both tests pass with the fixes. Missing
widget images now render without document artwork writes, and a complete unchanged
cache avoids capture. Four focused native widget tests pass, including saved-state
capture, generation retention, expired-pointer reads, renames and deleted sources.

Scheduling policy is checked when authoring, not reopening stored definitions; a
regression test first reproduced the stored-value refusal. The host clamps intervals.
Swift owns the disposable cache record and image bounds, with no core dependency in
the extension's shared target. Current and previous generations are retained; failed
publications remove partial output, and app maintenance removes abandoned output and
confirmed missing/replaced sources. Unavailable volumes and other access errors keep
cached images. Open-session selection uses identity recorded when the document opens.

The development composition selector exercises `capture.widget` with Rust-generated
viewports. Focused browser checks cover all families, accepted edits surviving size
switches, component HMR and live overflow warnings. Widgets clip at the fixed canvas;
the native geometry guard verifies that surface, not the fit of arbitrary children.
Authoring/design skills and guides now describe the widget role and preview workflow.


Manual Quick Checklist development captures at 164×164, 344×164 and 344×344 logical
points were reviewed at 2× scale. All three retain the authored composition with no
outer shadow or editor mask. Local images: `.hitslop/widget-review/dev-{small,medium,large}.png`.


The macOS app/extension Debug build passed. Removing the core dependency reduced the
extension's Debug dylib from roughly 18 MiB to 474,312 bytes; `nm` found no Loro,
SQLite, oxipng or core UniFFI symbols in it. These are local Debug artifact measurements,
not distribution-size claims.


Final verification: `bun run verify` passed (322 Rust tests); `bun run verify --native`
passed (220 Swift tests in three shards and 26 native CLI tests, with three existing
native scenarios skipped by their gates). Nine focused browser checks passed with
`bun run verify --no-build browser dev.browser capture.browser definition-build.browser`,
including Widget.svelte addition/removal during development. The final macOS app and
extension Debug build and `git diff --check` passed. Signed desktop picker/tap/refresh
qualification remains pending; no installation or distribution was performed in this pass.
