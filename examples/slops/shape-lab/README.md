# Shape Lab

An unbundled developer instrument for actual host clipping, pointer delivery,
resizing, text editing and capture independence. The page does not draw its own
silhouette. `variant.ts` contains display metadata only; the manifest owns geometry.

## Run

From the repository root, after `bun run build`:

```sh
bun run shape:lab list
bun run shape:lab build rounded
bun run shape:lab open rounded
bun run shape:lab open washer
bun run shape:lab open washer --fallback
bun run slop dev examples/slops/shape-lab
```

`open` uses `generated/app/hitSlop.app` (build with `bun run apple:build`) or
`HITSLOP_APP`, and opens a new writable copy under `.hitslop/shape-lab/documents`.
Generated masters are never edited. To preview a generated variant:

```sh
bun run shape:lab build washer
bun run slop dev generated/shape-lab/sources/washer
```

The launcher builds all six cases: rounded, radii, concave, hole, locked and washer.
The same production geometry contract drives native clipping, hit testing and fallback
PNG masking. Browser preview uses the validated radius/path with real CSS clipping.

## Observe

- Click N/W/E/S: the accepted count and target update through the real document owner.
- Type in the note; export and return to the editor. The note and controls must survive.
- Resize the rounded window using the native toolbar/window and the size buttons.
  The dimensions report the observed content size. Skins cannot resize.
- Dedicated export is a rectangle with four orange corner markers. Icon is a fully
  opaque square. `--fallback` removes only Export.svelte from generated source, so
  PNG export must retain the native shape mask instead.
- For holes, place a different application's window behind the slop. Hole clicks
  must reach it; visible edge clicks must reach the slop. JS button activation and
  screenshots alone are not proof of native input routing.
- The PNG washer's centre intentionally has no UI. Its count remains in document
  data and the rectangular export; the small edge buttons and text field exercise
  the visible rim. Use the host toolbar for dragging and closing.

## Verification

```sh
bun run swift:test --filter 'shapeLab|presentationFixturesExport'
```

The normal native fixture builder includes dedicated and fallback versions of each
case. It retains the pre-existing standard, ellipse and PNG-skin fixtures.
The shape contract is in the [runtime reference](../../../docs/reference/runtime.md)
and [manifest and windows](../../../apps/landing/src/content/docs/docs/guides/manifest-and-windows.mdx);
remaining acceptance work is listed in the [roadmap](../../../docs/roadmap.md).

Browser diagnostic (Playwright WebKit):

```sh
bun scripts/shape-lab-browser.ts
```

Writes screenshots and `browser.json` under `.hitslop/evidence/shape-lab`.
It exits nonzero when the real browser receiver does not receive a hole click.
The current PNG-mask preview has this known failure: CSS masking clips pixels,
not the iframe's pointer region. Do not treat matching screenshots as input parity.
