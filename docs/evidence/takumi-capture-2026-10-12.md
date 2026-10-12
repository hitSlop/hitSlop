# Native capture integration — 2026-10-12

Widgets render in the restricted Rust child. Eligible `Icon.svelte` and
`Export.svelte` closures now use the same engine for icons, preview artwork and PNG
export. The builder retains WebKit for closures that depend on browser lifecycle,
DOM, attachments, locale formatting, asynchronous code or host helpers. A declared
native program's runtime failure is reported rather than silently rerouted.

The local catalog build survey completed all 76 slops without build errors: 68
native icons and 38 native exports. This establishes compilation and conservative
eligibility, not browser-equivalent CSS support. Global editor CSS can contain
unsupported declarations that never affect a capture; the one-off compiled CSS
report records those declarations without turning slops into regression fixtures.
Eligibility also checks the retained compiled bundle for `Intl` from third-party
dependencies. The dependency fixture failed before that check and passed afterward.

The subsequent [full catalog visual review](slop-capture-review-2026-10-12.md)
completed 152 captures but found blank icons, lost SVG styling and two exports
with collapsed content. Those findings remain open; compilation eligibility and
the smaller comparisons below do not establish catalog-wide visual acceptance.

Manual comparisons use Quick Checklist, Reading Tracker, Contact Card, Koi Pond,
Weekly Planner and Invoice, with the same packed state and resources. A second
disposable template omits the private native programs to exercise WebKit. Koi
Pond's icon uses Rust while its rich preview/export keeps WebKit. Quick Checklist
and Weekly Planner native preview/PNG samples took approximately 172–213 ms;
their WebKit counterparts took 644–938 ms. These are individual local samples,
including helper startup, not a general performance guarantee.

Native export measures at the saved viewport width and extends the output canvas
around horizontal overflow, without widening and reflowing the authored layout.
Viewport units retain the saved dimensions. The shared regression covers an
element positioned immediately beyond the viewport's right edge: repeated reflow
previously moved it farther out on every measurement.
Reading Tracker's Export view uses explicit grid rows/columns: Takumi accepts
named grid areas syntactically but falls back to automatic cell placement. Its
capture heading also uses an explicit size instead of unsupported container units.

Apple color emoji remain the chosen emoji font. Noto Sans Symbols 2 adds the
missing text-symbol coverage, including checkmarks, after the emoji face. It adds
1,233,128 bytes under the OFL; each slop can still package its own font faces.

The initial widget integration used a bounded fingerprint of the saved Loro frontier. The app
watches cached source files, coalesces edits, and checks again at startup/reopen.
It publishes an immutable image generation before requesting a WidgetKit reload.
The last image remains available while the app is fully quit; the app catches up
when restarted. WidgetKit still controls when the requested timeline reload runs.
The closed-edit/restart regression failed before the fix and passed afterward.

## Separate vector PDF experiment

Production PDF remains on WebKit. The isolated `takumi-pdf 0.16.1` experiment
preserves embedded selectable text and Apple emoji with Unicode mappings. All six
tested emoji sequences extract intact, including a skin-tone ZWJ sequence and a
flag. All 1,000 numbered rows extract exactly once and in order from both a
continuous page and a 56-page A4 version. First, second and final A4 pages were
visually inspected.

Small combined SSR/PDF/PNG samples took 129–180 ms; PDF rendering alone took
4.7–10.4 ms. The long continuous case took 340 ms and peaked at 135 MiB RSS;
pagination took 422 ms and 130 MiB. macOS first-launch executable validation was
completed before these samples and is excluded from those timings.

The continuous long page exceeds the usual 14,400-point page-size bound. Adoption
needs a pagination/physical-sizing policy and a reproducible dependency package:
the PDF crate is unpublished and contains a vendored krilla fork. The backend
explicitly refuses blur filters. The visual results support adoption for the
constrained subset, but this experiment does not change production PDF routing.

Local scripts and visual evidence are under `spikes/takumi-capture/`:
`capture-integration.ts`, `capture-css-survey.ts`, `vector-pdf.ts`,
`out/capture-integration-final/` and `out/vector-pdf/`. They remain manual evidence,
not per-slop automated tests.

## Verification before renderer-aware freshness

`bun run verify --native` passed the final inputs, including the Rust, SDK, CLI,
packed-document, Swift and native integration boundaries. Rust reported 328 passed
and 11 skipped; native integration reported 26 passed and 3 skipped. A subsequent
`bun run verify` confirmed every tier had already passed with these exact inputs.

The final `bun run build:templates` completed all 76 catalog entries: 19 cache hits
and 57 rebuilt in 209.3 seconds. The comparison folder contains 36 successful icon,
preview and PNG captures across six templates. Widget publication and reload
callbacks were verified by shared integration tests; the installed desktop widget
was not manually exercised in this pass.

## macOS renderer-aware freshness

The widget source token now combines the saved Loro frontier, the actual evaluator
executable's SHA-256 digest and the canvas dimensions/scale. Bundled Takumi and font
changes therefore invalidate cached images without editing or migrating a document.
The digest is memoized in the app against file identity, size and modification/change
times and is computed off the main thread. A renderer replaced during capture is
refused; the previous complete image generation remains available.

The shared core regression uses unchanged saved state and identical PNG replies from
two different evaluator executables. With the renderer digest omitted, it failed at
the expected assertion: changing the renderer did not invalidate the token. With
the digest restored, the same regression passed (0.23 seconds), including stable
tokens for identical executable bytes and refusal of replacement during capture.
The native integration regression also covers missing/old cache tokens, redundant
refresh suppression and preservation of prior images when capture fails.

The installed app inspected during qualification was the 08:32 build, with an 08:24
evaluator. It predates these changes. Its catalog also reported five invalid local
pre-launch templates, including directory-shaped packages and older database layouts.
Those warnings do not establish a Takumi rendering failure; qualification must use
the rebuilt, installed app and a freshly created review document.

The full Rust suite passed 329 tests with 11 skipped using
`NEXTEST_TEST_THREADS=2`. At default concurrency during the release build, the
existing `runner_bounds_cpu_and_memory` case hit the parent's 3-second watchdog.
It passed in isolation and with two workers. Direct child measurements returned
the expected loop interruption, heap-exhaustion and recursion errors in 2.03 s,
0.37 s and 0.02 s. No production limit or test expectation was changed.

Takumi remains exactly pinned to `=2.15.1` with default features disabled and only
raster/HTML/SVG/PNG/WOFF2 enabled. The locked graph still includes `libwebp-sys
0.14.4` through `takumi-raster 0.5.3`, despite WebP output not being enabled; disabling
our WebP feature does not remove that upstream dependency. Vector PDF and its
vendored backend remain outside the production dependency graph.

## Installed macOS build and verification

The arm64 Release app, built with Cargo's `dist` profile, passed signed artifact
acceptance and replaced `/Applications/hitSlop.app`. Acceptance covered packaged
commands and reopen without an evaluator override, embedded widget intent metadata,
matching host/helper/engine resources, all 76 packaged starters, and installed editing
and PNG/PDF export without Bun or Node on the runtime path. Deep code-signature
verification passed. The old app was retained outside Applications for comparison.

Final code verification passed with `NEXTEST_TEST_THREADS=2 bun run verify --native`:
329 Rust tests (11 skipped), 222 Swift tests, and 26 native integration tests
(3 skipped), alongside the other required tiers. The new Swift regression exercises
old/missing renderer tokens, refresh suppression and failure retention. An initial
Swift compile error was a missing `try` on the fixture PNG helper and was corrected
before the successful run.

After reconciling the active docs and packaged skills, `bun run verify` passed again:
85 CLI tests, clean diagnostics for all 22 landing files, and 3 packed-package tests
(2 skipped); unchanged tiers retained their successful results. `git diff --check`
also passed. No commits or staging changes were made.

The installed app opened a newly created `Mac Refresh Review.slop` and published a
complete small/medium/large image generation. The source token, generation identifier,
document hash and image hashes are retained in local `widget-initial.json`. This proves
installed capture and publication; it does not prove that WidgetKit displayed the new
generation. The user elected to perform desktop widget testing themselves and then
reported that it worked. No per-size results or measured refresh delay were supplied,
so this records a successful user review without claiming every checklist scenario.

After installation, macOS still had the widget extension process launched at 08:32
alive, and the gallery showed its older placeholder text. Only that stale hitSlop
extension process was terminated so macOS could launch the installed code. No widget
preferences, selections or other widget processes were reset. The successful user
review followed that handoff; gallery appearance and reload timings were not separately
recorded.

## macOS capture cost

Measured on the same arm64 Mac with 16 GiB RAM and macOS 26.6.2, using the installed
Release helper. Each pair uses identical saved state and resources; the WebKit fixture
omits the private native capture programs. Each latency row is the median of five
fresh-process PNG exports after a separate warmup. It includes helper startup, SSR,
font registration, layout and PNG encoding. First-launch executable validation is
excluded. These are export measurements, not three-widget batch timings.

| Saved document | Rust median (range), ms | WebKit median (range), ms |
| --- | ---: | ---: |
| Quick Checklist | 115 (100–143) | 667 (506–921) |
| Reading Tracker | 115 (114–117) | 661 (635–680) |
| Weekly Planner | 121 (116–135) | 770 (741–783) |

Memory was measured separately over three captures per engine, with a native `libproc`
sampler pausing 2 ms between sweeps and no concurrent builds or tests. Rust includes
the native helper and evaluator. WebKit includes the native helper plus newly appearing
WebContent, GPU and Networking XPC processes; host RSS alone would undercount it.

| Saved document | Rust peak RSS range, MiB | WebKit peak RSS range, MiB |
| --- | ---: | ---: |
| Quick Checklist | 92.27–92.33 | 202.92–204.13 |
| Reading Tracker | 97.34–97.39 | 216.13–222.30 |
| Weekly Planner | 113.58–113.73 | 298.31–300.63 |

These are sums of resident pages at the sampled peak, not unique physical memory:
shared pages can be counted twice, brief peaks can be missed, and unrelated newly
launched WebKit processes could inflate the estimate. Recorded process identities
confirm the expected renderer processes at every peak. An earlier 40 ms shell sampler
missed the short Rust child; its RSS fields are superseded by this native sampler.
The WidgetKit extension does not run these captures or load these fonts, so these
figures are not extension memory usage or evidence of a WidgetKit memory ceiling.

## Distribution cost

Both builds use arm64 Swift Release and Cargo `dist`. The baseline is commit
`6e2af0f6`, before the uncommitted Takumi work. App size counts regular-file bytes,
excluding symlinks; ZIP sizes use `ditto`. Both contain 76 starters. The baseline outer
app is unsigned and the installed candidate is development-signed, so the full-app
delta includes signing metadata and rebuilt artwork as well as renderer changes.

| Artifact | Before | After | Increase |
| --- | ---: | ---: | ---: |
| Complete app | 89.50 MiB | 118.59 MiB | 29.10 MiB |
| App ZIP | 52.65 MiB | 76.30 MiB | 23.65 MiB |
| Embedded evaluator | 1.55 MiB | 28.91 MiB | 27.36 MiB |
| Widget extension bundle | 331.32 KiB | 351.33 KiB | 20.02 KiB |
| Standalone CLI engine | 9.11 MiB | 36.46 MiB | 27.35 MiB |

The standalone engine also contains the private evaluator path and therefore pays
the renderer/font cost; it is not an additional executable bundled in the macOS app.
Embedded fallback font files total 21,906,236 bytes (20.89 MiB), predominantly the
20,091,552-byte Apple emoji font. The widget extension remains core- and font-free.

Local reproduction and raw evidence live under `spikes/takumi-capture/`:
`macos-qualification.ts`, `macos-rss.c`, `macos-rss.ts`, `macos-size.ts`, and
`out/macos-qualification/{capture-costs,rss,sizes,widget-initial}.json`. The scripts
and image comparisons are manual evidence, not committed per-slop tests. The earlier
widget platform spike's foreground/background batch times measure a different workload
and are not substituted for the paired export results above.

## Desktop widget review checklist

The user reported the installed widget working after this checklist was handed over.
Keep the scenarios below for detailed qualification; individual outcomes were not logged.

Use the installed `/Applications/hitSlop.app` and the disposable document at
`.hitslop/widget-review/macos-qualification/Mac Refresh Review.slop`.

1. Add a **hitSlop** widget from Edit Widgets, then edit its **Document** selection to
   **Mac Refresh Review**. Preserve any existing Image Spike widget; it is a separate
   experimental extension.
2. Open the review document. Change its checklist title and toggle a task. Leave the
   document open and check that the desktop widget picks up both changes. WidgetKit
   schedules the requested reload; it may not appear immediately. Record the observed
   delay instead of removing/re-adding the widget to force it.
3. Inspect small, medium and large sizes for readable text, clipping and the expected
   content. Make another edit in each size and check that its image refreshes.
4. Close the document window and check that the latest image remains. Click the widget
   and confirm it opens the correct document. Quit and reopen hitSlop and confirm the
   widget still selects that document and retains its image.
5. Report any stale content, wrong selection, missing emoji or clipped layout, including
   which size was affected. Light/dark appearance and sleep/wake remain further product
   qualification, beyond this initial refresh review.

Closed-file edits while the app is quit, restart catch-up and renderer-only invalidation
are covered at the shared core/host boundaries. Their visible desktop outcomes have
not been separately recorded. The user's successful desktop review is additional
evidence; image publication and reload callback tests alone do not prove visible refresh.
