# Fog

**Job:** play with a steamed-up bathroom mirror. The webcam is the mirror (mirrored);
breath fogs it, a pinch draws in it, a fist swipe wipes a wide patch, and S keeps a frame.

**Readout:** the mirror itself, full window. Done and a small device indicator sit top
left; Re-fog and Help sit top right. The lowercase `brush` pill stays bottom left and
the shutter bottom right. A Bits UI popover holds the gesture guide and shortcuts;
only startup and device/tracking problems need a persistent message. Space operates a
focused control normally. Escape closes help before it exits the mirror.

## Layers (per frame, one `requestAnimationFrame` loop)
1. The camera frame, cover-fitted and mirrored.
2. A hidden canvas that stores only fog density (alpha). It starts at 0.55. Breath adds
   soft radial blobs; pinch strokes and fist stamps erase with `destination-out`.
   Nothing is ever painted white.
3. Frost: the same frame blurred (18 CSS px), brightened (1.18), with the frost tint
   (`frost`, default rgb(240,246,242)) mixed in at 0.42, with cached moisture texture, masked by the density canvas with
   `destination-in`. With canvas filters it is drawn at quarter resolution with
   `blur() brightness()`; without them (older WebKit) a 1/24 downscale gives the blur and
   an additive 0.18 pass gives the brightness. The masked frost is composited twice:
   alpha 0.55 becomes about 0.80 optical coverage, without adding more white pigment;
   erased pixels remain clear. A faint diagonal reflection and a one-pixel polished
   edge finish the pane, including saved snapshots. These do not obscure wiped areas.

`buildGlass` caches sparse water highlights, soft cloudy variation and the edge reflection
at the current pixel density, rebuilding only on resize with a fixed random seed. The
moisture is inside the erased mask, never a noisy overlay on the clear camera. Breath
blobs fade from their centre instead of holding a flat disc of opacity.

All pixel sizes are multiplied by `devicePixelRatio` capped at 2. Tuned values live in
`gestures.ts` (`TUNING`) and are the brief's numbers; change them only by hand-held testing.

## Tracking (`mediapipe.ts`)
One runtime, `@mediapipe/tasks-vision` 1.1.0, bundled: the SIMD wasm and its loader, the
hand landmarker and the face detector models are assets of this tiny. Both run in VIDEO
mode, synchronously from the frame loop, only when the camera has a new frame: the hand
every tracked frame (GPU, falling back to CPU), the face every 5th (CPU; its tiny model is
faster there). Landmarks are normalized to the unmirrored camera frame and mirrored onto
the screen; handedness is not used. The trackers exist only while the mirror is live:
created on start, closed on Esc, hide and unmount. tasks-vision sends anonymous usage
metrics (task, timings, OS; no images) to Google; this is accepted and stated on the start screen.

## Gestures (pure functions in `gestures.ts`)
- Breath: RMS of the raw mic (echo cancellation, noise suppression and AGC off) against a
  floating floor: trigger above floor + 0.0025, full at + 0.003; the floor moves 5% toward
  a lower level and 0.08% toward a higher one per frame. Ignored for 300 ms after any key.
  12 blobs every other frame, 1.45x wider than tall, reaching 0.43 of the short edge,
  0.36 each, blur 0.6 of radius. Paused while drawing. Space holds full-strength fog.
- Mouth: the face detector (BlazeFace short range) runs on every 5th tracked frame; the
  largest face's mouth keypoint is the mouth (else 0.22 of the face box below the nose,
  else 0.78 down the box). The fog centres 0.04 of the short edge below it and glides
  there at 0.4 per frame. With no face for 1 s it falls back to a fixed mirror-typical
  mouth (centred, 62% down the camera frame).
- Pinch: thumb-tip to index-tip over wrist to middle knuckle; down below 0.22, up above
  0.38. A new stroke needs the index extended. Raw point to target at 0.45, brush chases
  at 0.22, quadratic curves through midpoints, moves under 0.0006 of the width ignored.
  A vanished hand keeps the pen down 250 ms; a release must last 2 frames. A small
  glowing star follows the pinch point at 0.55.
- Fist: all four fingertips nearer the wrist than their middle joints. Radius 1.6x hand
  width; one 0.34 radial stamp per half radius travelled, at most 40 per frame; a jump of
  over 30% of the screen stamps once.

## Palette and type
Pale mint glass (`surface` #e5f2ed, `room` #bcd8cc), a white highlight (`lamp`),
soft green tile lines (`tile`), near-white mint frost (`frost` #f0f6f2), deep green
`ink` #203e34 and readable `muted` #526e63. The camera itself stays untinted;
only the frost receives color. The live canvas observes theme changes. The brush
track and shutter stay white (`accent`), the device dot is green (`live`), and
permission errors use `alert`. System rounded sans, no bundled fonts.

The start screen is a flowing centered composition: a small smile, lowercase “fog,”
“a little room to exhale,” three short gesture instructions, and one Start mirror
button. Privacy copy states that images/audio stay on the Mac and discloses anonymous
tracker metrics. Errors replace the instructions and offer Try again. Short windows
scroll; essential controls have 44px targets and visible keyboard focus.

## Still views (`Glass.svelte`)
A pale mint pane with faint tile lines, fine condensation, a small wiped corner and
finger-drawn smile. The start screen shows the last snapshot behind the frost when
there is one. The export shows the saved snapshot unaltered, with its date, or the
resting glass when empty. The icon is a larger wiped smile. No still view requests a
device. Start, export and icon share the same palette.

## State
Document: `brush` (7 to 38, default 15) and `snapshot` (attachment reference with size
and time). Saving again replaces the snapshot; the old image leaves the file when it
closes. View state: streams, audio graph, trackers, the last mouth seen, fog density, gestures, the live brush
value while dragging. Fog is not saved: Esc, hiding the window (devices off, back to the
start screen) or closing loses it; C and Re-fog restore the initial 0.55 density.
Re-fog clears stroke/wipe anchors and waits for a held gesture to release before
accepting another mark. The Bits UI brush slider previews locally and commits once
per completed adjustment.

## Motion
Live canvas only while the mirror is on; the loop, tracks, trackers and audio graph stop on Esc,
unmount and `visibilitychange`. The shutter flash is reduced to a faint blink under
reduced motion. The still views do not animate.

## Packaging
The runtime, WASM and both models remain bundled. This visual pass adds no dependencies
or schema changes. Native preview/icon artwork and PNG/PDF exports use the same mint
glass; the icon deepens the room behind its wiped smile for legibility at small sizes.
