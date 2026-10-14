# Practice Sheet design

One job: learn a piece of music at a speed you can play.

**Readout.** The engraved score with a moving playback cursor, and the current tempo as
a big numeral beside it. Everything else (ladder, record) supports those two.

**Object.** A sheet of cream manuscript paper on a music stand, against oxblood piano
felt. The stand's dark ledge under the sheet holds the transport: a brass Play button,
Loop with its bar range, metronome Click and a one-bar count-in. The right rail is the
teacher's side: the tempo numeral with −/+ (one ladder step), a brass ladder from the
start tempo (bottom) to the goal (top) with the current rung marked, the "Clean run → N"
button, and a practice record index card with the streak, best run, total minutes and a
14-day strip shaded in blue pencil.

**Palette** (theme keys, all read by `styles.css`): `surface` oxblood felt, `surfaceDeep`
the ledge, `onSurface` text on felt, `paper`, `ink`, `muted`, `rule` (paper lines and
input borders), `brass` (Play, the cursor, ladder, primary actions), `pencil` (loop bands,
record labels and day cells, focus on paper). A playing note turns felt-red. State is also
given in words (status line, "Goal 112 · 28 to go", day cells have text labels).

**Type.** Fraunces (variable, optical sizes) for the title, the tempo numeral, the streak
and body copy; Barlow Condensed SemiBold for small caps labels, controls and numbers in
the ledge. Both bundled through Vite with their OFL texts. ABC is edited in the system
monospace.

**Behavior.**
- Edit notes opens the ABC text above the score; the score re-engraves 250 ms after the
  text settles. Text that cannot be engraved shows a plain "Nothing to engrave yet" state
  and disables Play.
- Play (or Space) plays the piece, or the looped bars, at the stored tempo, with an
  optional one-bar count-in and click. Clicking a note loops its bar; Shift-click extends
  the range; the bar inputs do the same from the keyboard. Looping repeats until Stop;
  a tempo change (a clean run, −/+) applies from the next pass. A single play-through
  ends by asking "Was that clean?" (Yes climbs a rung).
- A clean run adds the ladder step, never past the goal. The ladder popover edits start,
  goal and step and can drop back to the start.
- Log today records minutes at the current tempo. The minutes field suggests the time
  spent playing back this session (15 when nothing played). A streak stays alive through
  today until today is over. Sessions can be removed from the history popover.
- Playback stops when the window is hidden and everything is disposed on unmount.

**Sound.** No soundfont and no network: `player.ts` synthesizes every note with Web Audio
(a triangle oscillator plus a decaying sine octave through a low-pass, a soft
electric-piano/music-box tone) and a short square-wave click. The AudioContext is created
on the first Play press and closed on unmount. abcjs is used only to engrave and to time
the notes (`setUpAudio` + `setTiming`, repeats unrolled); its own synth is not used, so it
never fetches its default soundfont. Mid-tune `Q:` changes are scaled linearly; per-note
instruments, dynamics beyond MIDI velocity and multiple voices' timbres are not modelled.

**Saved vs view state.** Saved: `abc` (text), `tempo`, `ladder {start, goal, step}`, and
`log [{at: YYYY-MM-DD local day, tempo, minutes}]`. View only: playing, cursor, loop on and
its bar range, click and count-in toggles, the edit panel, the session timer and the
popovers. Edits are written once in `practice.ts` and shared by the page and `commands.ts`.
Command `logPractice` defaults to the current UTC date (the command runner has no local
time zone); callers should pass `day`.

**Motion.** The cursor glides across each note's width while playing; with reduced motion
it steps from note to note and line changes scroll without smoothing. Button and rung
colour changes are 120–200 ms transitions, removed under reduced motion.

**Export and icon.** Export is the same sheet at natural height: title block, a brass
tempo stamp (♩ = tempo, goal), the full score (no cursor or loop), then the ladder and the
practice record. The icon is a tilted corner of the sheet on felt with a phrase, the brass
cursor on a felt-red note, and the brass stamp showing the current tempo.

**Window.** Standard, resizable, 940 × 660, fullscreenable. Below 760 px wide the rail
stacks under the stand and the page scrolls.

**Size** (debug engine build, 2026-10-10): `dist/practice-sheet.slop` 712,704 bytes.
`ui.js` 690,606 bytes (180,573 stored with Brotli; abcjs is most of it), `ui.css` 15,990,
`commands.js` 8,802, fonts 360,440 (Fraunces) + 109,428 (Barlow Condensed SemiBold).
