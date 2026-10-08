# Native fullscreen qualification

`window.fullscreenable: true` is implemented for standard and skinned windows. It
defaults to false. The green hover-toolbar control and View → Enter/Exit Full Screen
use native macOS Spaces and preserve the mounted page and document owner.

## Checked

- Rust acceptance, defaulting, stored round trips and page projection passed with the
  full Rust suite. TypeScript and Swift use the generated Rust contract.
- Native tests passed for responsive, fixed-size, explicitly shaped and PNG-skinned
  windows. They cover fit/reflow, pointer coordinates, transparent hit regions, actual
  Spaces transitions, viewport dimensions, toolbar reachability and restoration of
  frame, style, aspect constraint and pinned level.
- The same mounted page and its timer survive entry/exit. A deliberately terminated
  renderer can recover inside fullscreen, and the host still permits exit while the
  renderer is unavailable.
- A freshly built Mac app passed the app tier, including packaged commands, installed
  editing and export.
- Final `bun run verify --native` passed all selected tiers: types, SDK/shell, CLI,
  packed package, 200 Swift tests, and 84 native tests. The existing host-death release
  check was skipped in this development run. The opt-in fullscreen tests were run
  separately and passed, together with the app tier, after the rendering changes.
- Manual inspection of a disposable Quick Checklist document in the generated app
  confirmed visible content across the fullscreen viewport. Checking a task worked;
  exiting restored the original 480 × 620 window with that edit preserved.

The local desktop uses an external 1920 × 1080 display and keeps the macOS menu bar
visible in fullscreen. AppKit reserves its 30-point strip; the page fills the remaining
window. The host respects this system preference.

## Findings and fixes

Changing the fitted composition's frame before its bounds autoresized its WebView into
the scaled dimensions. Fullscreen layout now sizes its children in the authored
coordinate system. Hit testing also converts AppKit's superview point into that system;
the regression test failed for all three fitted cases before the correction.

Desktop style restoration runs on the next main-loop turn after AppKit's exit
notification. Aspect constraints reset through resize increments; setting the aspect
ratio to zero caused an AppKit failure.

The original transition fixture mounted an empty page, and viewport assertions alone
did not qualify painting. The fixture now has an explicit colored surface and label;
the transition test also samples WebKit's rendered image across the viewport. These
checks passed, but a user screenshot still showed an orange strip above the skinned
page. A WebKit snapshot does not capture AppKit's outer frame. Fullscreen preparation
now detaches the document and installs the opaque stage before changing window style,
and uses an opaque titlebar without a separator. The rebuilt app's washer skin showed
a clean black surround, aligned content and working clicks; the user also confirmed
that the background looked better.

That visual check exposed soft page text from scaling the native view's backing.
Fitted fullscreen now keeps native views at display size and applies WebKit page zoom,
preserving authored CSS dimensions while painting text at display resolution. Mask
geometry scales separately, including absolute corner radii and hit testing. PNG skins
retain their original bitmap resolution; enlarging a low-resolution skin cannot create
missing detail. The updated Spaces tests and app build passed. Manual comparison of
the same washer document confirmed sharper button text, accepted clicks and restored
desktop zoom. The 1× and 2× PNG fixtures still expose their bitmap edges when enlarged;
this is distinct from the page text, which now renders at the destination resolution.

## Scope

This is Mac fullscreen qualification. It does not implement browser document opening,
browser fullscreen, Wake Lock, downloads or hosted share URLs. The separate
[browser evaluator evidence](browser-evaluator-2026-10-08.md) records the first browser
gate and the durable owner work that remains.
