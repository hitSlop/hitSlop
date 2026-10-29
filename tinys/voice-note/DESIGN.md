# Voice Note

## Job
Say something now, read it later: record a short voice note, keep the clip, and get its text
transcribed on-device, then fix any word by hand.

## Readout and action
One dominant action, the red Record key, beside an amber display showing state (READY, REC,
SAVING, NO MIC), the running time with the time left out of five minutes, and a 20-segment input
level. Below, the notes, newest first: play, the date, length and format, the editable text, and
the transcription state.

## Object
A pocket dictation recorder clipped above a steno pad. The deck is warm graphite with a speaker
grille, a domed record key in a ring that fills as the five minutes run out, and a backlit LCD.
The pad is eye-ease green paper with a wire binding, a red margin rule, ruled transcript lines and
perforated tear lines between notes. Typed notes (no recording) carry a pen mark in the margin;
recordings carry a play key.

## Palette (tiny.ts theme, every key read by the CSS)
body, bodyEdge, faceplate, label (deck); record, recordDeep (record key, danger); lcd, lcdInk,
lcdDim (display); paper, paperShade, rule, margin, ink, muted, wire (pad); warn (unavailable and
cancelled states). No dark mode: the object is a physical recorder and pad.

## Type
Lora for transcripts and the pad title, Space Mono Bold for the display and durations, Barlow
Condensed SemiBold for labels and keys. All bundled through Vite under the SIL OFL.

## Capture rules
- The microphone starts only from the Record key (or Space/Enter on it). The display reads REC with
  a blinking dot while it is on, and the key becomes Stop & keep.
- Stop & keep imports the clip as an attachment and asks the host for an on-device
  transcription at once. Discard drops the recording; "Not saved until you stop" says so
  while recording.
- Every track stops on keep, discard, unmount and when the window is hidden (minimised, hidden or
  ordered out): hiding keeps what was recorded and says so.
- Recording prefers `audio/mp4` (WebKit and current Chromium record AAC in mp4), then WebM or Ogg.
  64 kbit/s and a five-minute cap keep a note near 2.3 MB at most, under the 10 MiB limit; a clip
  over the limit is refused with a message.
- Errors are explained on the deck, never as a blank: microphone access off (`NotAllowedError`),
  no microphone (`NotFoundError`), microphone busy (`NotReadableError`), recording not available
  in this window, nothing recorded, could not save.
- Speech: `unavailable` for host, system, permission, locale and notDeclared shows one notice on the
  pad with what to do; format, cancelled and failed show under the note. Every clip has a
  Transcribe key; the host decides which formats it supports and supplies the message. Silence leaves the text alone.
  Retranscribing a note with text asks first, because it replaces hand corrections.

## Document state
`title`, `locale` (empty is the device's language) and `notes: list({recordedAt, durationMs, clip?,
transcript, transcribed?})`. `clip` is the attachment reference exactly as `attachments ref`
prints it, so `batch --attach` can add a recording. `transcribed` records the locale and time of
the last on-device transcription. Deleting a note removes the reference; the host drops the blob at
close.

## View state (local, never saved)
The microphone stream, recorder, level, elapsed time, problems, per-note transcription progress
and errors, the speech notice, playback (which note, position) and open dialogs.
Export cannot see any of it: it shows every note with its text, length, format, size and whether it
was transcribed on-device. It never imports the recorder and never asks for a device.

## Motion
The REC dot blinks and the transcription dot pulses; both stop under `prefers-reduced-motion`,
as does the key press transition. The level meter is a live readout, not decoration.

## Size
Built `dist/voice-note.tiny`: about 537 KB. `ui.js` about 200 KB, `ui.css` 17 KB,
`commands.js` 7 KB; the three fonts are about 420 KB of it.
