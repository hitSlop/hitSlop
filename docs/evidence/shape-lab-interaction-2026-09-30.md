# Shape Lab native feasibility gate — 2026-09-30

Method: two borderless AppKit windows in separate processes, foreground layer mask
with an offset even-odd hole and geometric hit testing. Real human mouse clicks;
manual mode did not post CGEvents. Automated posting remains unauthorized.

The first attempt exposed an ordering issue in the diagnostic: clicking the hole
activated the blue receiver, which covered the orange foreground. Both independent
counters received input (foreground 9, receiver 14); those totals alone do not prove
which samples passed. Manual mode was adjusted to restore foreground ordering after
a received click, without synthesizing or rerouting mouse input.

The maintainer then confirmed: “Yes, both hole sizes and the rim work.” This answers
the explicit request to check that both hole clicks increment only the blue receiver
and the visible-rim click increments only the orange foreground. The second run's
logs recorded two foreground and ten receiver clicks; these are diagnostics, not an
automated assertion of the requested sequence.

Result: **human-verified feasibility pass** for hole/rim delivery and resized-hole
delivery. It is not proof of the production hitSlop host, SVG parsing, browser pointer
routing, capture parity, outside-outline routing, or shadow refresh. Those remain
separate acceptance checks for the production geometry implementation and Shape Lab.

## Browser diagnostic

Playwright WebKit accepted edge-button clicks and text edits in rounded and washer
variants. An actual pointer click into the visually transparent PNG washer hole did
**not** reach an underlying button in the preview page. This is a known existing
`previewFrame` gap: its CSS PNG mask clips pixels without excluding the hole from
iframe input. The repeatable diagnostic is `scripts/shape-lab-browser.ts`; it
returns failure for this case and records screenshots/results. No parity pass is claimed.


## Production geometry checks

All six Shape Lab variants now build. Playwright WebKit 26.6 confirms real edge clicks,
text edits and transparent-corner routing for the five radius/vector variants. The
concave notch and offset holes deliver clicks to an underlying DOM receiver before
and after resize. Locked resize stays 4:3; free resize changes proportions. Browser
radius corners initially intercepted clicks despite rounded pixels; explicit
`clip-path: inset(0 round …)` fixed the repeated diagnostic. PNG skin masking retains
the known pointer limitation above; the diagnostic still exits nonzero for that case.

Native focused tests pass all six dedicated PNG/PDF/icon captures, editor restoration
and fallback masking. The manifest tuple-member regression failed before the generated
validator fix and passed afterward. Full release and production-window manual results
are recorded separately; the original probe is not represented as production proof.


## Reported black strip and capture restoration

The maintainer supplied a screenshot of the earlier rounded lab with an extra black
area beneath its 480×360 editor, and said it appeared on opening. Resetting the size
cleared it. Native UI automation subsequently verified the same document at 480×360,
600×400, and a bottom-edge drag to 480×450 with no strip. After saving/quitting the old
process and launching the rebuilt app, opening that saved document again filled the
480×360 window correctly; its five accepted clicks and note remained intact. The
original opening-only trigger has not been reproduced or conclusively attributed.

Inspection also found an independent capture/resize race: an async capture restored
the WebView frame saved before a concurrent window resize. A deterministic native test
paused capture preparation, resized the window, and observed the stale frame (failed
before the fix). Restoration now fits the current native container, while standalone
render sessions restore their original frame. The regression passes after the fix.
This is not asserted to be the cause of the opening-only screenshot.


## Final automated verification

The complete 16-stage release gate passed after the capture-restoration fix: 137 fast
tests, 165 Swift tests, nine native CLI tests, template captures/reopens, packed CLI,
landing build, matching app/helper resources and five crash-recovery phases. Separately,
all 53 Rust tests passed. See the release report (`shape-lab-release-2026-09-30.json`; report not retained).
The production free-resize vector-hole lab was opened successfully; its manual
cross-process click-through and shadow check remains awaiting maintainer observation.
