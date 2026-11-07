# Launch video

The current launch is a 30-second Remotion film in `LaunchWide` (1920×1080)
and `LaunchVertical` (1080×1920), defined in `src/Launch.tsx`.
It keeps the existing Finder → SomaAmp Play opening and starts Addison Rae's
“New York” at frame 134, from the same 89.14-second song offset.
The closing copy is “Little apps. Better together.”

From this directory:

```sh
bun run prepare:launch   # Discover templates and create/inspect fresh writable .pop copies
bun run capture:launch   # Record current app previews, including the imported local song
bun run check
bun run review:launch    # Review frames for each aspect ratio
bun run render:launch   # Both MP4s and montage posters in out/
bun run studio          # Interactive timeline review
```

Run `bun pop build pop/focus-timer` and `bun pop register pop/focus-timer` from
the repository root before preparing the demo if Pomodoro is not registered.
`prepare:launch` leaves existing demo documents intact and copies brand artwork
from the landing page. Templates are immutable; all demo documents use `.pop`.
There is no `.slop` conversion.

Doodle Board uses `public/captures/launch-doodle-native.png`, a native window
capture of a heart created in its writable document through the `drawStroke` CLI
command. Preserve this still when refreshing browser animation captures.

The current Little Days collaboration footage is real: an iPhone 13 mini joins
the Mac document through a live link. The phone logs a nursing entry and the
Mac adds “First little laugh today.” Both recordings show the edits arriving.
The T/J avatar treatment is an editorial overlay modeled on the landing page,
not a native presence feature. The brief agent exchange is simulated.

The raw phone take was recorded in QuickTime using the USB iPhone Screen input;
the Mac window was captured with screencapture. Raw recording creation times are
23:26:33 UTC (phone) and 23:26:37 UTC (Mac) on 2026-11-03. The edit uses phone
20.3–23.8s + 37.5–40.5s and Mac 16.3–19.8s + 33.5–36.5s, preserving the four-second
start offset. Trimmed, silent clips are `public/footage/little-days-phone.mp4`
and `little-days-mac.mp4`. Record replacements through the real live session;
never substitute independently edited preview state for collaboration footage.

`public/captures`, `public/demo`, `public/footage`, `public/music`, and `public/brand`
are local production assets and ignored by Git. Keep `new-york.mp3` in
`public/music`. `capture:launch` regenerates `src/launch-media.json`, which holds
only capture dimensions, frame counts, timing marks, and a start-frame offset
for captures that originally included transparent loading frames. Raw media should be
retained locally alongside the project. Shared invitation links and credentials
are not included in the source or films.

The renderer bundles source once and serves media in place through a temporary
symlink, avoiding a duplicate copy of the PNG sequences for each aspect ratio.
The existing `Promo`, `Film`, and website publishing commands remain available;
rendering the launch does not publish or replace the website video.
