# Capture Notebook

## Job
Keep what you see and hear in the field, in one document: take a photo, record a short sound, jot a line, and find all of it again later.

## Readout and dominant action
The viewfinder and its **Take Photo** button. Under it the tape strip (**Record Audio**, then **Stop and Keep**). Below, the notebook page lists everything kept, newest first.

## Object
A dark field instrument (camera and tape recorder) clipped above a cream notebook page. Photos are taped-in prints, recordings are round mic marks with a play bar, notes are pen marks. The red margin line and masking tape make the page a page; the instrument stays matte and quiet until something is live.

## State language (always words as well as colour)
- Camera live: green viewfinder rim and a "LIVE · CAMERA ON" pill. Camera off: "Camera is off. Nothing is seen or saved until you take a photo."
- Recording: red strip outline, pulsing red dot, "RECORDING", a running `0:42 / 5:00` clock, a level meter, and "Not saved until you press Stop and Keep."
- Busy: "Starting the camera…", "Keeping…", "Starting microphone…", "Keeping recording…".
- Errors (in the viewfinder or the tape strip, `role="alert"`): access off (`NotAllowedError`/`SecurityError`), no device (`NotFoundError`/`OverconstrainedError`), device busy (`NotReadableError`/`AbortError`), no capture in this window (no `mediaDevices`/`MediaRecorder`), file too large or notebook full. Each says what to do next and offers Try again.

## Capture rules
- Devices start only from a click (Turn on camera / Record Audio) and only in the editor. `Preview.svelte`, `Export.svelte` and `Icon.svelte` render saved content and never import `devices.ts` or call getUserMedia. Preview selects a few photos, recordings and notes; export includes them all.
- Photos: the current frame drawn on a canvas, longest edge at most 1600 px, JPEG at quality 0.86, kept immediately as an attachment.
- Recordings: `audio/mp4` (m4a) when MediaRecorder offers it, otherwise WebM or Ogg Opus, at 64 kbit/s, capped at 5 minutes (about 2.4 MB), imported when you press Stop and Keep. Discard throws the take away.
- Every file is checked against the limits before import: 10 MiB per file, 256 files and 100 MiB per document.
- Cleanup: Turn off stops the camera tracks. Hiding the window (minimise, Hide) turns the camera off and stops-and-keeps a recording in progress. Unmount (closing the window, Reload Interface) stops every track; an unfinished recording is lost then, which the strip says while recording.

## Palette (tiny.ts theme, all read by the CSS)
body, bodyInk, bodyMuted, lens (the instrument); paper, ink, muted, rule, tape (the page); shutter + onShutter (the Take Photo button); rec (recording, margin line, playback bar); live (camera on, level meter). The page is light and the instrument dark in every system appearance; there is no separate dark mode.

## Type
Space Grotesk (UI, captions, notes) and Space Mono Bold (stamps, clocks, counts), bundled through Vite under the SIL OFL.

## Motion
A 280 ms white flash on Take Photo, a pulsing record dot, the level meter and small press feedback. Under reduced motion the flash is skipped and the dot holds still; the meter remains because it carries information.

## Data model
`title` (text) and `entries: list({kind: photo|audio|note, at: ISO string, caption: text, file?: {id, name, mimeType, byteLength}, width?, height?, seconds?})`. A file is the attachment reference returned by `attachments.import`; removing the entry releases it (unreferenced attachments are dropped when the document closes). Recording length is stored because WebM from MediaRecorder carries no duration.

## View state (local, never saved)
Camera and microphone streams, recorder and its unfinished chunks, level, elapsed time, playback position, the open photo and the pending remove confirmation. Export cannot see them and shows every entry: a photo grid, recordings listed with length and format (no playback in a PNG/PDF), and notes.

## Commands
`rename`, `addNote`, `setCaption`, `removeEntry`. Commands cannot capture or carry file bytes; agents add a photo or recording with `tiny batch --attach` and an `insert` into `entries`.

## Window
520 × 700, resizable. Below 400 px wide the camera controls stack; from 860 px the instrument and the page sit side by side.

## Size
Built `dist/capture-notebook.tiny`: 339,968 bytes. `ui.js` 140 KB, `ui.css` 17.5 KB, `commands.js` 6.9 KB, fonts 235 KB (Space Grotesk 137 KB, Space Mono Bold 98 KB).
