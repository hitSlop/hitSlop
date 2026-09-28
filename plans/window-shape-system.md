# Window shape system: evaluation and redesign

Status: proposal, not started. Nothing here has been implemented or run.
Decisions already made: manifest schema is **free to break** (pre-launch); direction is **vector path silhouettes**.

## 1. Summary

"Shape" is the window-silhouette layer of a slop: the outline that clips the window, gates pointer hit-testing, clips PNG/preview exports and (for skins) supplies backing art.

Today it is small and mostly sound, but:

- The three built-in shapes are really one primitive (a rounded rectangle with CSS `border-radius` semantics) dressed as an enum.
- Authors cannot see the geometry, so they cannot align their own edges to the native mask.
- The presentation mode is derived in four places and they already disagree.
- Anything beyond the three shapes forces a fixed-size 1x PNG that conflates artwork, mask and hit region.

The proposal:

1. Replace the enum with a CSS-style radius string or an SVG-path silhouette (one `CGPath` on the native side).
2. Derive presentation once: the host forwards the manifest `presentation` verbatim and the runtime derives the stage for both native and preview.
3. Expose the radius to authors as `--slop-radius`.
4. Optionally retire PNG masks later, once vector silhouettes prove out.

## 2. How the system works today

All statements below come from reading the code. Nothing was run.

| Layer | Location | Behavior |
|---|---|---|
| Manifest contract | `packages/schema/src/manifest.ts:49-65`, tolerant read schema `:89-107` | `presentation` is a union: standard `{width, height, resizable?, shape?, background?}` or skin `{width, height, skin}`. `shape` is `rounded\|ellipse\|capsule`. Unknown shape strings read as absent (rounded). |
| Bridge reply | `packages/schema/src/bridge.ts:73-80` | `config.presentation` = `{width, height, resizable, shape, mode}` |
| Swift model | `HitSlopCore/SlopPackage.swift:115-120`, `:246` | `isSkinned`, `usesTransparentBackground`, `isResizable`, `shape` (default `.rounded`); unknown shape/background stripped from the decoding copy only |
| Native mask | `HitSlopHost/SlopWindowMask.swift` | `Content = geometry(Shape) \| image(CGImage, AlphaMap)`. One class provides the `CALayer` mask, `contains` (hit test), `png(from:)` (export clip) and the backing layer |
| Window wiring | `HitSlopHost/SlopWindow.swift:65-84`, `:252-276`, `:669-673` | `ShapedView` sets `layer.mask` and returns `nil` from `hitTest` outside the mask; the toolbar pointer sampler reuses `contains`. Window flags: `isOpaque=false`, `hasShadow`, `minSize 240x180`, square-ellipse aspect lock |
| Config payload | `HitSlopDocument/DocumentSession.swift:281-293` | Builds the `presentation` dictionary for the runtime |
| Runtime CSS | `packages/document/src/presentation.ts`, `boot.ts:150-154` | Native: installs `config.presentation` as sent. Preview: derives it from the manifest with `presentationStage`. Sets `data-slop-presentation`, `data-slop-shape`, `data-slop-resizable`, `--slop-width/-height` and the stage sizing CSS |
| Dev preview | `packages/cli/src/authoring.ts:77-100` | `previewFrame` emulates the window with `border-radius` (shape) or `background`/`mask` of the skin PNG |
| Export | `HitSlopHost/SlopRenderer.swift:219`, `SlopPreviewImage.swift:34-36` | Window snapshots go through `SlopWindowMask.png`; dedicated export views skip the mask |

Behavior summary of the mask:

- Geometry: `.rounded` radius 22 (scaled for 2x export), `.capsule` radius `min(w,h)/2`, `.ellipse` an oval (`SlopWindowMask.swift:121-132`).
- Image: the skin PNG is both backing artwork and alpha mask. Alpha 0-25 is click-through, 26-255 receives input. It must match the manifest dimensions exactly and the window cannot resize (`SlopPackage.swift:123-155`).

Strengths worth keeping:

- One native class owns pointer, pixels and export, so the three cannot drift.
- Skin validation is strong (containment, symlinks, PNG type, exact size, alpha).
- The tolerant reader degrades unknown enum values instead of refusing to open.

## 3. Findings

### F1. The three shapes are one primitive
`rounded` = radius 22, `capsule` = radius `min(w,h)/2`, `ellipse` = radii `(w/2, h/2)`. All are a rounded rect with CSS `border-radius` semantics: `22px`, `9999px`, `50%`. The CLI preview already maps them to `border-radius` (`authoring.ts:91`). The enum is repeated in `manifest.ts:53` and `:96`, `bridge.ts:78`, the generated Swift `Shape` enum, `SlopPackage.swift:246` and `presentation.ts:9`.

### F2. Authors cannot see the geometry
The 22pt radius is a literal in Swift and again in the preview (`min(22px,50%)`). Nothing exposes it. `data-slop-shape` is the only hook and the active example does not reference it. Quick Checklist hard-codes its own radii (`examples/slops/quick-checklist/styles.css:102`, `:617`). An app cannot align a hairline, border or shadow with the native edge, and per-corner or elliptical silhouettes are not expressible at all.

### F3. Presentation mode is derived in four places and they already disagree
- `DocumentSession.swift:287-292` always sends `shape: package.shape.rawValue`, which defaults to `rounded`, including for skinned windows.
- `boot.ts:154` installs the native config unchanged, so native skinned windows get `data-slop-shape="rounded"`.
- `presentation.ts:14-20` (the preview path) omits shape for skins, and `docs/guides/authoring.md:118` documents it as "absent for PNG skins".
- `SlopWindow.swift:263` (`!usesTransparentBackground || isSkinned`) is `mode != transparent` written as booleans.

This is a documented-behavior mismatch found by reading; reproduce before treating it as confirmed (inspect `document.documentElement.dataset.slopShape` in a native skinned window).

### F4. A skin conflates art, mask and hit region, at 1x
`SlopPackage.swift:142` requires PNG pixels to equal manifest points, so a Retina window upscales the art. Skinned windows cannot resize. Feathered alpha needs a threshold and author warnings (`authoring.md:138`). Every custom outline is forced through this route.

### F5. Repeated constants and a hidden rule
- Minimum size 240x180 appears in `SlopWindow.swift:260`, `manifest.ts:46` and `authoring.ts:95`.
- "Ellipse and square implies aspect lock" (`SlopWindow.swift:269`) is implicit and applies only to that shape.

### F6. Minor
- `SlopWindowMask.contains` rebuilds an `NSBezierPath` on every call (`:89`), invoked from each hit test and each toolbar pointer sample. Cheap, but cacheable per bounds.
- `invalidateShadow()` is never called anywhere in `HitSlopHost`. Shaped-window shadows may go stale after a resize. **Unverified**; check visually.

### F7. Test gap
`Tests/HitSlopHostTests/SlopWindowMaskTests.swift` covers image masks (orientation, 10% alpha threshold, ring hole, scale independence, non-resizable) and the transparent backing. There is no direct test of geometry hit-testing or of the export clip agreeing with the window mask.

## 4. Evaluation of the vector-path options

The libraries suggested during the discussion fall into three groups.

**Not applicable.**
- Silhouette Studio / `.studio3` cutter-file parsers: unrelated (name collision with "silhouette").
- Boolean-union / outer-contour extraction (`paper.js`, `martinez`, `i_overlay`, `geo`, `Clipper2`): solves a problem we do not have. The author supplies one outline, not overlapping paths.

**What we actually need is small.**

| Need | Approach |
|---|---|
| Swift: `d` string to `CGPath` | Own parser, ~120 lines: M/L/H/V/C/S/Q/T/A/Z, absolute and relative, arcs converted to cubics. No first-party CoreGraphics `d` parser I can vouch for. Hit-test with `CGPath.contains(_:using:)` (no flattening). |
| CLI preview | No parser. Inline SVG `<clipPath clipPathUnits="objectBoundingBox">` with `transform="scale(1/vbW,1/vbH)"` on the `<path>`. The browser scales it, and `clip-path` also clips pointer events, so preview becomes more faithful than today's PNG mask. |
| TS validation | ~50-line grammar check (command letters and argument counts) plus a schema charset and length bound. No geometry. |

**Rejected.**
- PocketSVG / SVGKit: full-document SVG libraries, a signed-app dependency for one function, drag in transforms and styles we do not want.
- Rust `kurbo::BezPath::from_svg` (in `hitslop-core` via UniFFI/WASM): best-tested parser, but it would change engine artifacts the repo treats as sealed and host/helper-matched, for window chrome. Revisit only if arc handling proves buggy.
- `svg-path-commander`: only worth adding as a CLI dev-dependency if we add normalization/transform tooling. Must never enter the runtime bundle.

**Useful later, authoring-time only.** Vision `VNDetectContoursRequest` returns a `CGPath` from an image's alpha. It could power `slop shape trace assets/skin.png`, converting an existing PNG skin to a path (Phase 3).

## 5. Target design

### 5.1 Manifest

```jsonc
"presentation": {
  "width": 480, "height": 620,
  "resizable": true,
  "background": "transparent",
  "lockAspect": false,

  // exactly one of shape / skin; shape is the default form
  "shape": "22px",
  // "shape": "9999px"                         capsule
  // "shape": "50%"                            ellipse (circle when square)
  // "shape": "40px 40px 120px 40px"           per-corner
  // "shape": { "path": "M0 40 ... Z", "viewBox": [480, 620], "fillRule": "evenodd" }

  // "skin": "assets/skin.png"                 Phase 3: artwork only
}
```

Rules:

- Radius string is a CSS `border-radius` subset: `px` and `%`, one to four values, optional `/` for elliptical radii. Default `"22px"`. CSS's over-large-radius scaling (`f = min(side / sum of adjacent radii)`) is implemented once in Swift, so browser and native agree; `9999px` therefore yields a capsule.
- `path.d` uses SVG path syntax in `viewBox` units. `viewBox` defaults to `[width, height]`. The path scales to the window; non-uniformly when resizable. `fillRule` defaults to `nonzero` (matches SVG); `evenodd` gives holes.
- `lockAspect` (default false) replaces the implicit square-ellipse rule and also serves fixed-proportion paths.
- `shape` and `skin` stay mutually exclusive, like today's union variants. Skinned windows keep forbidding standard controls.
- Bounds: `d` up to 4096 characters, a segment cap (start at 512), `viewBox` values 1-16384. Charset pattern `^[MmLlHhVvCcSsQqTtAaZz0-9eE+\-.,\s]+$`.
- Tolerant read schema keeps its contract: unknown future `shape` strings read as absent (default radius). A malformed path in a known form is an authoring error and refuses to open, like a bad skin does today.

### 5.2 Derive once

- The host forwards the normalized manifest `presentation` verbatim as the `config.presentation` reply.
- The runtime always calls `presentationStage(manifest.presentation)`, for native and for `slop dev`.
- This removes the Swift-assembled dictionary (`DocumentSession.swift:287-292`) and the bespoke reply schema (`bridge.ts:73-80`), and fixes F3 by construction.
- Swift keeps one `PresentationMode` enum (`standard | transparent | skin`) on `SlopPackage`, used only for window flags: backing, shadow (`hasShadow = mode != .transparent`), WebView background.

### 5.3 Author surface

- Add `--slop-radius`: the `shape` string verbatim (`0` for path shapes). An app can set `border-radius: var(--slop-radius)` on its shell to match the native edge exactly.
- Remove `data-slop-shape`; nothing uses it.
- Keep `data-slop-presentation`, `data-slop-resizable`, `--slop-width`, `--slop-height`, `data-slop-controls`.
- Not adding safe-area insets. No slop needs them, and deriving them for arbitrary paths is speculative. Revisit when a real slop asks.

### 5.4 Native

New `HitSlopCore/SlopSilhouette.swift`, pure CoreGraphics with no AppKit:

```swift
public struct CornerRadii { /* per-corner rx, ry as px or fraction; CSS clamping */ }
public struct SVGPath { /* parsed segments; parse(_:) throws */ }

public enum Silhouette {
  case radii(CornerRadii)
  case path(SVGPath, viewBox: CGSize, fillRule: CGPathFillRule)

  /// Path in AppKit (y-up) coordinates for `rect`; the y-flip lives here.
  public func cgPath(in rect: CGRect) -> CGPath
  public var fillRule: CGPathFillRule { get }
}
```

- `SlopPackage` gains `silhouette()` validated in `init` (next to `validatedSkin`), plus `presentationMode`.
- `SlopWindowMask.Content` becomes `vector(Silhouette) | image(CGImage, AlphaMap)`. Vector uses one code path for `CAShapeLayer.path` (with `fillRule`), `contains` and the export clip; the scaled `CGPath` is cached per bounds.
- `SlopWindow` uses `PresentationMode` for `hasShadow` and reads `lockAspect` for `contentAspectRatio`.
- Check `invalidateShadow()` after resize (see F6).

### 5.4a Corner and coordinate details to get right
- Per-corner elliptical arcs: build with `addArc` under a scale transform, or four `addQuadCurve`-free arc segments. Verify `50%` on a non-square window is an exact ellipse.
- SVG is y-down and AppKit is y-up. Flip inside `cgPath(in:)`. Export uses `NSImage(size:flipped:false)`, so verify orientation at both the layer mask and the export clip.
- Squircle (continuous) corners are not in scope: `CAShapeLayer` paths do not use `cornerCurve`. Compare visually before adding.

### 5.5 CLI preview

`previewFrame`:
- Radius: `border-radius: <shape string verbatim>`. Deletes the enum ternary.
- Path: inline SVG `<clipPath clipPathUnits="objectBoundingBox">` with the `d` string (escaped, and already charset-restricted by the schema) and `transform="scale(1/vbW,1/vbH)"`, `clip-rule` from `fillRule`. Apply as `clip-path: url(#...)` on the window element.
- `lockAspect`: `aspect-ratio: w/h`.
- Skin (until Phase 3): unchanged.

## 6. Phases

### Phase 1: Native cleanups (no contract change)
1. Add `PresentationMode` to `SlopPackage`; replace `usesTransparentBackground`/`isSkinned` combinations in `SlopWindow.swift:263` and `DocumentSession.swift:195`.
2. Cache the geometry path per bounds in `SlopWindowMask`.
3. Allow skin PNGs at an integer multiple of the manifest size (same aspect). Keep hit-test scale independence (existing test covers it). Update `SlopPackage.skin()` validation, the authoring docs and the CLI skin check.
4. Verify shadow staleness on resize; add `invalidateShadow()` in `windowDidResize` if it reproduces.

### Phase 2: Vector silhouettes (land before contract 4 is sealed)
Timing: `runtimes/releases.json` seals only contract 3 revisions 1 and 2; contract 4 (`runtimes/4/identity.json`, revision 1) is the unsealed development baseline. `runtimes/4/index.js` bundles `presentation.ts` and `boot.ts`. Editing them now is free; after a contract-4 seal, byte changes need a new runtime revision (`docs/versioning.md`). Confirm the seal state before starting.

1. Schema: `manifest.ts` (standard/skin variants, read schema), remove the `bridge.ts` presentation reply object in favor of the manifest presentation type; run `bun run schema:generate`.
2. `SlopSilhouette.swift`: `CornerRadii` parser and clamping, `SVGPath` parser, `cgPath(in:)`.
3. `SlopPackage`: validate at open; drop `Shape` normalization (`:246`); keep tolerant handling of unknown future values.
4. `SlopWindowMask`: `Content.vector`; shared layer mask, hit test and export clip; keep the image case untouched.
5. `DocumentSession.config`: forward the normalized manifest `presentation`.
6. Runtime: `presentation.ts` and `boot.ts` derive the stage from the forwarded manifest; add `--slop-radius`, drop `data-slop-shape`.
7. CLI: `previewFrame` per 5.5; grammar validation of `d` at build/validate time.
8. Docs: `docs/guides/authoring.md` (window presentation, `:109-139`), `docs/versioning.md:58-62`, `packages/cli/skills/hitslop-design/**` (edit `skills/`, not the generated `.crust/root` copy).
9. Optional: adopt `border-radius: var(--slop-radius)` in a shell of the active example only if it improves the design; do not change Quick Checklist merely to exercise the feature.

### Phase 3: Decision gate, retire PNG masks
Only after Phase 2 proves out. If vector covers the skin use cases (holes via `evenodd`), demote `skin` to artwork only (any size or 2x, drawn behind the page, clipped by the silhouette). Then delete `AlphaMap`, the alpha threshold, the `destinationIn` export branch and the exact-size and non-resizable rules, and add `slop shape trace`. This is where the large deletion happens.

## 7. Tests

Follow AGENTS.md: name the observable failure and independent expected result, extend the existing boundary, keep fault injection narrow.

| Test | Failure it catches | Expected result (independent of implementation) |
|---|---|---|
| `SlopWindowMaskTests`: extend the asymmetric-mask test to a vector path | y-flip error in hit-test or export | Point near the visual top inside the path is hit; near the bottom is not |
| `SlopWindowMaskTests`: vector ring with `evenodd` | Holes not click-through | Ring points hit; center is not (mirrors `ringMaskLetsClicksFallThroughItsTransparentHole`) |
| `SlopWindowMaskTests`: radius cases from CSS spec values | Radius parsing or clamping drift from browsers | `9999px` equals a capsule, `50%` an ellipse, oversized radii scaled per spec; corner pixel outside, edge-center inside |
| `SlopWindowMaskTests`: 2x export clip | Export clip disagreeing with the window mask | Alpha at the corner of the exported PNG is 0 |
| `SlopPackageTests` | Malformed or out-of-bounds path opens | Open refuses with a clear error; unknown future shape value still reads as default |
| Parser table | Silent mis-parse | Implicit repeats (`M 0 0 10 10`), compact numbers (`.5.5`, `1-2`, `1e-3`), relative commands, arcs; expected values from the SVG spec examples |
| `packages/schema/tests/manifest.test.ts` | Contract regression | `shape`/`skin` exclusivity, charset and size bounds, read-schema tolerance |

Ledger: record changed contracts in `docs/test-ledger.md`. Delete the preview shape-ternary assertions only if equivalent proof remains; before removing consequential coverage, break the protected behavior and confirm the remaining owner test fails.

## 8. Verification

1. `bun run schema:generate && bun run check && bun run test`
2. `bun run build && bun run swift:test && bun run test:native`
3. Native manual: open a path-shaped slop with an `evenodd` hole and a `transparent` background. Confirm clicks pass through the hole, the shadow follows the outline after a resize, and PNG export is clipped to the path.
4. Preview manual: `slop dev` shows the same outline and clicks also fall through the hole.
5. Skinned window (before Phase 3): `document.documentElement.dataset.slopShape` is unset and `--slop-radius` is absent.

## 9. Risks and open questions

- **Parser correctness, especially arcs.** Mitigated by spec-derived table tests. Fallback: reconsider `kurbo` if arcs prove buggy.
- **Preview/native parity for `%` and mixed units.** The browser is the oracle for `border-radius`; the clamping test uses spec values.
- **Non-uniform scaling of resizable paths** can distort art. Authors who need proportions set `lockAspect` or `resizable: false`.
- **AGENTS.md says "preserve shipped contracts."** This plan assumes nothing from contract 4 has shipped and that the manifest may break, as decided. Confirm before Phase 2 that no released host reads the current `shape` enum.
- **Skin authors** lose a one-file workflow in Phase 3 (they need a traced path). `slop shape trace` is the mitigation; do not start Phase 3 without it.
- **Open:** default `fillRule` (`nonzero` matches SVG; `evenodd` makes holes easier). Plan uses `nonzero`.
- **Open:** whether any slop beyond Quick Checklist needs safe-area insets; deferred until one does.

## 10. Explicitly not doing

Boolean-union libraries, `kurbo` in the core, SVGKit/PocketSVG, speculative safe-area insets, squircle corner smoothing (needs a visual comparison first).
