# Hero artwork and fonts

The marketing hero owns this visual direction. Its typography does not apply to
live slop demos or documentation.

## Fonts

Self-hosted Latin WOFF2 files in `public/assets/hero/fonts/`, downloaded from Google
Fonts. Each family includes its SIL Open Font License in the same directory.

- Mochiy Pop One, regular: marketing headline, sticker badge, merch headings.
- Nunito Sans, variable 400–900 (opsz 6–12): header and hero interface/copy.
- Kalam, regular: handwritten annotations.

Mochiy Pop One runs wider than a typical display face; keep hero sizes tight
(`letter-spacing: -.04em`, line-height about .96) and check the title at 1100–1280 px.

Sticker, speech bubble, callouts, smiley, stars and underlines are inline SVG/CSS and
decorative (`aria-hidden`). They hide below 1200 px where the poster has no margin.

## Feature illustrations

Generated with `genmedia` and `fal-ai/recraft/v4.1/text-to-vector` on 2026-09-17.
Three generations at the reported $0.08/image price. No runtime generation.

- `folder.svg`: request `01a0b259-7186-73c1-850d-35c306980385`
- `edit.svg`: request `01a0b259-dec5-7093-ae6e-76a8f375720f`
- `together.svg`: request `01a0b259-e293-7471-9542-8bc69f442e44`

Shared prompt direction: a single small icon for a playful indie Mac app website;
expressive, slightly irregular rounded felt-tip purple outline; one small magenta
accent; almost no interior detail; flat vector; no lettering, shadows, or 3D;
simple friendly geometry readable at 40px.

The subjects are an open folder, a diagonal pencil with two sparkles, and two
people represented by round heads and shoulders. Generated SVGs were cropped,
color-normalized to the hero palette, and stripped of their full-canvas background
and generation metadata. The three production SVGs total approximately 12.5 KB.

Arrows, marker strokes, sparkle decorations, and playback controls are authored
inline SVG/CSS. All headline and annotation lettering is live HTML text.

## Product tour video and poster

`public/assets/desktop-hero.mp4` (30 s, 1920×1080, with music) and
`desktop-hero-poster.jpg` are rendered in code by `apps/promo`; there is no screen
recording. The README uses the same poster.

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
