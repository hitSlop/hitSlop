# Landing artwork, fonts and data

The landing page is a playful desktop: a hero, the draggable desktop playground
(`playground/`), the template wall, and interactive sections. Its typography does
not apply to live pop demos or documentation.

## Template wall data

`src/data/templates.json` and `public/assets/templates/` are generated from every
built template (`bun run build:templates`):

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

## Brand art

The current Corner Hello file character is a hand-authored vector with its editable
Rive project in `artwork/brand/corner-hello`. That source exports `mark.svg`, `hello.svg`,
`mark.webp`, the main `logo.webp` paired with the existing lettering, and the live
`mascot-wave` and `mascot-party` images. The app keeps its gradient icon and documents
keep their captured artwork. Run the source folder's `export.mjs` to rebuild those
outputs; its README describes the CLI requirement. The remaining artwork below is
retained from the earlier sticker treatment.

`public/assets/brand/` holds the `.pop` logo and its parts (`logo`, `mark`, `wordmark`, favicons and
`apple-touch.png`), cut from the supplied sticker. The hero's cup, the mascot poses (`mascot-wave`,
`mascot-phone`, `mascot-party`), the washi tape and watercolor strips, and the three collaborator
faces (`face-*`) were generated with `openai/gpt-image-2.5/flare` (transparent background for
everything except the faces), then resized to WebP. Prompts: a rainbow slushie with the supplied
sticker on it; separate semi-transparent washi tape and watercolor swashes; the same mascot waving,
holding an iPhone, or celebrating; flat sticker-style head-and-shoulders portraits on pastel squares.

The current header uses two new assets generated through fal's `openai/gpt-image-2`
text-to-image API at high quality. The original `cup.webp` is retained.

- `cup-v2.webp` (640×1352): an unbranded cup generated at 1024×1536 on white, cut
  out with a smooth silhouette mask and resized to WebP. It sits behind the hero
  desktop. GPT Image 2 refused the transparent-background option, so the cutout
  was done locally. Prompt: "One tall clear plastic slushie cup
  with a crisp transparent domed lid, a generous heaped mound of cherry-red and
  hot-pink crushed ice, and a tall translucent electric-blue drinking straw rising
  at the upper left. Rainbow slush: raspberry pink at the top, golden lemon,
  turquoise blue, and violet near the base; naturally icy, with wet sparkling
  crystals and realistic condensation. Front-facing three-quarter view, gently
  tapered cup, realistic tactile materials, saturated color, soft studio
  highlights. Leave the center front free of labels. Full object including straw
  and base in frame, isolated on uniform pure white without a cast shadow. No
  text, logos, faces, hands, thick outlines or extra objects."
- `hero-paper.webp` (1536×1024): generated at the same size, blended 22% toward the
  site's ivory paper color, then encoded as WebP. Prompt: "Warm almost-white ivory
  paper with subtle natural fine paper fibers. Broad hand-painted dry-brush and
  pastel collage marks only around the left and right edges and lower corners:
  hot candy pink near the upper-right edge, turquoise and sky-blue diagonal paint
  strokes at both side edges, a little warm lemon-yellow at the bottom corners.
  Real wax crayon and gouache grain, irregular edges and translucent overlapping
  tissue-paper shapes. Keep the central 70 percent and upper central area clear
  warm ivory paper. No gradients, blurry blobs, text, people, icons, cups, devices,
  interface, frames or drop shadows."

These are static, locally hosted assets. Generation credentials are never part of
the website source or browser runtime.

The hero desktop (`playground/Desktop.svelte`) shows playable stand-ins for Focus Timer, Little Days
and Pocket Pod. The iPhone beside it is a second Little Days (`PhoneDays.svelte`) that shares the
journal in `playground/journal.svelte.ts` with the Mac window, so a tap on one appears on the other.

## Fonts

Self-hosted Latin WOFF2 files in `public/assets/hero/fonts/`, downloaded from Google
Fonts. Each family includes its SIL Open Font License in the same directory.

- Omnes Black (`omnes-black-latin.woff2`, latin subset of a licensed font supplied by the owner): hero H1, section headlines and hero step labels.
- Baloo 2, variable 400–800: sticker badges and supporting display text.
- Nunito Sans, variable 400–900 (opsz 6–12): hero interface/copy.
- Kalam, regular: handwritten annotations.
- Newsreader (500–700, italic 500) and Onest (400–800): the invoice, export paper and theme demo.

No third-party font CSS is loaded; every face is self-hosted with its license beside it.

The hero uses container-relative Omnes sizing, two explicit lines and a .9 line-height.
Check both title lines at 1100–1280 px and at 320 px; the sticky note must never
cover the download buttons or secondary links.

Stickers, annotations, the smiley and underlines are inline SVG/CSS (`src/styles/motion.css`).
Annotations are decorative (`aria-hidden`) and always point at something interactive.
All motion stops under `prefers-reduced-motion`. Blob, the desktop pet, is original pixel
art (`playground/DesktopPet.svelte`).

## Product tour video and poster

`public/assets/desktop-hero.mp4` (30 s, 1920×1080, with music) and
`desktop-hero-poster.jpg` are rendered in code by `apps/promo`; there is no screen
recording. The README uses the same poster. On the site the poster lives in the
playground's "pop in action.mp4" window (AVIF/WebP/JPEG at 960 and 1920 px) and
opens the video dialog (`VideoDialog.astro`).

1. `bun run capture` drives the real example apps through `pop dev` in headless
   Chromium: it plays SomaAmp and drops in a classic `.wsz` skin, imports a Codex pet
   ZIP, applies the checklist and theme edits that the terminal scene shows, and
   exercises the other montage apps. Each clip is saved as an alpha PNG sequence in
   `public/captures` (gitignored). `PROMO_SKIN` and `PROMO_PET` point at the imported
   files and default to `~/Desktop/pop`.
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
