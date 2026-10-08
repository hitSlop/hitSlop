# Landing artwork, fonts and data

The landing page is a playful desktop: a hero, the draggable desktop playground
(`playground/`), the template wall, and interactive sections. Its typography does
not apply to live slop demos or documentation.

## Template wall data

`src/data/templates.json` and `public/assets/templates/` are generated from every
built template (`bun run build:templates`), except the Shape Lab instrument:

```sh
bun apps/landing/scripts/templates.ts
```

Titles, descriptions, categories, window shape, tile colors and icon and preview
artwork come from each template's file; a template without preview artwork gets a
curated emoji tile. Commit the outputs: the Cloudflare build installs only
`apps/landing` and never builds templates. Rerun after templates change.

## Share image

`public/og.jpg` (1200×630) is rendered from `scripts/og-card.html` with the dev
server running (`bun run --cwd apps/landing dev`), then screenshotted in a browser at
1200×630 and saved as JPEG. It reuses the product-tour poster.

## File mascot

`public/assets/file-mascot.webp` is the original decorative file sticker beside
“I’m literally just a file.” Generated with the built-in image tool, then resized
to 384×437 and encoded as WebP (quality 90) with its transparency preserved. The
image is static, lazy-loaded, and has empty alt text; the heading carries the meaning.

Generation prompt:

> Use case: illustration-story. Asset type: transparent sticker mascot for the hitSlop landing page beside the headline ‘I’m literally just a file.’ Draw one original cute kawaii anime-style anthropomorphic paper file, full body, centered and tightly framed with a little clear margin. Cream paper body shaped like a softly rounded document with a folded lavender upper-right corner, dark navy ink outlines, closed u-shaped anime eyes and a tiny w-shaped uwu mouth, rosy pink cheek blush, tiny feet, little rounded hands, one hand raised in a shy wave. Slight jaunty tilt and charming hand-drawn linework, simple cel shading, warm approachable personality, crisp readable silhouette at 80–150 pixels wide. A thin cream sticker edge. Keep the face large and clear. The body is paper, not a folder. True transparent background; no backdrop, no ground plane, no checkerboard, no surrounding objects, no text, no watermark, no photorealism, no 3D.

## Fonts

Self-hosted Latin WOFF2 files in `public/assets/hero/fonts/`, downloaded from Google
Fonts. Each family includes its SIL Open Font License in the same directory.

- Omnes Black (`omnes-black-latin.woff2`, latin subset of a licensed font supplied by the owner): hero H1 only.
- Baloo 2, variable 400–800: floating header wordmark/nav, sticker badge, merch headings.
- Nunito Sans, variable 400–900 (opsz 6–12): hero interface/copy.
- Kalam, regular: handwritten annotations.
- Newsreader (500–700, italic 500) and Onest (400–800): the invoice, export paper and theme demo.

No third-party font CSS is loaded; every face is self-hosted with its license beside it.

Baloo 2 is compact and heavy; keep hero sizes tight (`letter-spacing: .005em`, line-height about .92) and check the title at 1100–1280 px.

Stickers, annotations, the smiley and underlines are inline SVG/CSS (`src/styles/motion.css`).
Annotations are decorative (`aria-hidden`) and always point at something interactive.
All motion stops under `prefers-reduced-motion`. Blob, the desktop pet, is original pixel
art (`playground/DesktopPet.svelte`).

## Product tour video and poster

`public/assets/desktop-hero.mp4` (30 s, 1920×1080, with music) and
`desktop-hero-poster.jpg` are rendered in code by `apps/promo`; there is no screen
recording. The README uses the same poster. On the site the poster lives in the
playground's "hitSlop in action.mp4" window (AVIF/WebP/JPEG at 960 and 1920 px) and
opens the video dialog (`VideoDialog.astro`).

1. `bun run capture` drives the real example apps through `slop dev` in headless
   Chromium: it plays SomaAmp and drops in a classic `.wsz` skin, imports a Codex pet
   ZIP, applies the checklist and theme edits that the terminal scene shows, and
   exercises the other montage apps. Each clip is saved as an alpha PNG sequence in
   `public/captures` (gitignored). `PROMO_SKIN` and `PROMO_PET` point at the imported
   files and default to `~/Desktop/slops`.
2. `bun run render` composes the desktop, Finder, dock, cursor and terminal in Remotion
   (`src/Promo.tsx`, with beat timings in `src/timeline.ts`) and writes the MP4 and
   poster into `out/`. Copy them here with `bun run publish`.

The first 4.5 s are silent. When the cursor presses SomaAmp's play button (frame 134),
Addison Rae's "New York" starts at its 89.1 s kick-in, so the player seems to be
playing the song. The ignored local file is `public/music/new-york.mp3`.
`src/music.json` holds the beat grid from that point (148 BPM, fitted with librosa to the
percussive onsets). `src/timeline.ts` snaps the skin and pet swaps, the CLI edits, the
montage pops and the logo to those beats. **This is a commercial recording: license it
or replace it before the video is published.**

The terminal commands and their output were run for real against copies of the
documents using CLI 1.2.0. The desktop chrome and wallpaper are drawn, not captured.
The Tenchi Muyo skin and Akitsuki Airi pet are third-party community files, used only
as import examples.
