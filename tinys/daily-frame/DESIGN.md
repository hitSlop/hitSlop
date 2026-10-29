# Daily Frame

## Job
Take one photo a day of the same thing (a plant, a drawing, a workout, a baby) and watch it change as a flipbook.

## Readout
Today's frame as a large instant print, and the stack so far: frame count and current day run in orange
date-imprint numerals, a film strip of every frame, and "this day last month" beside it.
When today has no photo, the print is undeveloped film showing the last frame faintly, and the
primary action is **Take today's frame** (it becomes **Retake today** once taken).

## Object
A camera body at night: charcoal surface, a cream instant print on the stage, a 35 mm film strip with sprocket
holes, and the orange LED date imprint (`'26 10 10`) of a 90s film camera in the corner of every photo. The
shutter in the viewfinder is a red disc in a cream ring.

## Palette (tiny.ts theme, all read by styles.css)
body (window), panel (raised cards, menus), film (strip, empty photo areas), print (print borders, primary buttons),
ink and inkMuted (text on prints), onBody and muted (text on the body), stamp (date imprint and numerals),
live (camera-on dot, shutter, destructive action), accent (focus ring, selected thumb, Ghost toggle on),
blank (undeveloped film). The drawn sample frames use their own fixed illustration colours. Dark only: the object is a camera.

## Type
Space Grotesk (UI, notes, headings) and Space Mono Bold (date imprint, numerals, day labels), bundled through Vite
under the SIL OFL. Numbers are always in the mono.

## States
- **View**: the print for the chosen day (default today). The note on a saved frame is edited in place on the print.
- **Camera**: started only by a click. "Asking for the camera…", then a live viewfinder with a "Camera on" badge,
  "Live preview · nothing is saved until you press the shutter", today's imprint, and a Ghost overlay
  (the newest earlier frame at 35% opacity, to line the shot up). Esc or Cancel stops every track. The camera
  also stops after the shutter, on unmount, on `pagehide` and when the window is hidden (with a notice).
  Errors (`NotAllowedError`, `NotFoundError`, `NotReadableError`, others) show a titled explanation with
  Try again and Use a photo file, never a black rectangle. A camera list appears when there is more than one.
- **Flipbook**: plays the stack oldest first at 3, 6 or 12 frames per second, loops, pauses while hidden;
  scrubber, pause, Stop/Esc. The neighbouring frames are mounted so the swap does not flash.
- **Sample**: a new document has no photos (templates cannot carry attachments), so fourteen drawn frames of a
  basil seedling, dated relative to today and marked SAMPLE, fill the strip, the flipbook, the export and the icon.
  They are derived in `frames.ts`, never stored, and vanish when the first photo is kept.

## Storage
`frames: list({date, attachment: {id, mimeType, byteLength}, note})`, date as the local `YYYY-MM-DD`, kept in date
order. One frame per day: a retake sets the day's attachment fields to the new photo and keeps the note; the old
photo is no longer referenced and the host removes it when the file closes. A frame is a JPEG re-encoded on a
canvas at most 1280 px on the longest edge, quality 0.84 (camera or chosen file, EXIF rotation applied by the decoder).
A file holds at most 256 attachments and 100 MiB, so at most 256 frames (about eight months of daily frames);
the strip header shows the count and total size, and a 257th day is refused with a plain message.

## Motion
The shutter flash (280 ms), the pulsing camera-on dot, button press feedback. Reduced motion removes the flash
and the pulse. The flipbook only plays when asked.

## View state (local, not in the document)
The selected day, mode (view, camera, flipbook), camera phase, chosen camera, Ghost on/off, flipbook position,
speed and pause. Captures cannot see them: the export shows the newest frame large and every frame
as a contact sheet. `Preview.svelte` selects the most recent 8 frames for Quick Look,
with "and N earlier frames" beyond that.

## Size
Built `dist/daily-frame.tiny`: about 360 KB, of which 235 KB is the two fonts; `ui.js` is 230 KB
(63 KB Brotli). A kept camera frame measured 10 to 15 KB from Chromium's 640x480 fake camera, and 54 to 133 KB
for real photographs re-encoded to 1280 px; a noisy real webcam frame will likely be larger (an estimate,
not measured), still far below the 10 MiB attachment limit.
