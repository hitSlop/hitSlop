# Quiet Meter

**Job:** show whether a room is too loud. **Readout:** one big lamp (green, amber, red) with the level
inside, a 25-lamp LED gauge beneath it, and a word ("Quiet", "Getting loud", "Too loud") so state never
rests on colour alone (the word also carries a distinct glyph: circle, triangle, square).

**Object:** a dark instrument case on a concrete-grey desk with a paper panel below it for the settings.
Palette is in `slop.ts` (surface, panel, housing, housingEdge, onHousing, ink, muted, rule, quiet, warn,
loud, focus); every key is read by `styles.css`. Type is Outfit (bundled, OFL), tabular numerals.
Window 420 x 660, not resizable; the page scrolls if the settings panel does not fit.

**Capture:** `capabilities: ["microphone"]`. The mic starts only from the Listen button, with
echo cancellation, noise suppression and auto gain off so the level reflects the room. A Web Audio
`AnalyserNode` gives RMS, mapped from -80..0 dBFS to 0..100 and smoothed (fast attack, slow release).
The level is relative to the microphone, not calibrated sound pressure, and the UI says so. Tracks and
the AudioContext are released on stop, on unmount, and when the window is hidden (`visibilitychange`),
which also saves the summary. Errors (denied, no device, busy) show a plain explanation, never a blank lamp.
No audio is recorded, stored or sent; there are no attachments.

**Stored:** room name, amber and red thresholds, and the "too loud" sustain seconds (all edits written
once in `meter.ts`, shared by the page and `commands.ts`), plus `sessions`: one summary per finished
session (date, seconds, too-loud seconds, episodes, peak), keeping the latest 14.
**Why a session list rather than a live daily count:** the live tally cannot be saved continuously
without writing on every frame, and a day count would need merging rules between windows. The live tally
is transient; stopping writes one summary in one edit. "Too loud" means red for at least the sustain time;
each such run is one episode and its time counts as too-loud time.

**View state (not stored):** microphone phase, smoothed level, running tally, slider drag values.
**Export** (`Export.svelte`) and the icon never touch the microphone: Export shows the thresholds and the
last saved summary, with the lamp lit at the last peak and captioned "peak, last session".

**Motion:** lamp colour fades and a pulsing live dot; all removed under reduced motion and in capture.
Focus is a 3px ring; sliders and steppers are native controls.

**Built size:** quiet-meter.slop is about 188 KiB (Outfit.ttf is most of it).
