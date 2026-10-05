# More slops: study, music, science and everyday tools

Status: planned (2026-10-05). Not started. Nothing is committed without asking.

## Why

Only `quick-checklist` and `shape-lab` are active under `examples/slops`
(`bundled.json` ships `quick-checklist`). The earlier ~60 templates wait in
`examples/archive`. We want a new set of slops that are **useful to students and in real
life, novel, and fun**, and that make use of these libraries:

| Library | Use |
| --- | --- |
| `ts-fsrs` | spaced-repetition scheduling |
| `@observablehq/plot` | clean statistical and scientific charts |
| `layerchart` | Svelte-native charts (upgrade path for existing dashboards) |
| `vis-network`, `vis-timeline` (from visjs.org) | graphs, mind maps, timelines |
| `mathlive` | typeset, editable maths |
| `jsxgraph` | interactive geometry and function graphs |
| `abcjs` | ABC notation: score rendering and playback |
| `3dmol` | molecule viewer |
| `wavesurfer.js` | audio waveforms, regions, loops |
| `matter-js` | 2D physics |
| `threlte` | Svelte-native three.js (`three` is already a dep) |
| `moveable` | drag, resize, rotate, snap |
| `pixijs` | 2D WebGL (only where it earns its weight) |

Each slop follows `examples/slops/PRODUCT.md`: **one understandable job, one dominant
action or readout, a look of its own**, and an export (PNG/PDF) that is recognizably the
same object. The archive is design evidence only; it is not a contract
(`AGENTS.md`: `archive/` is not active).

## How this relates to `plans/slop-capabilities.md`

That plan already covers the host work for microphone, camera, speech-to-text and AI
(`capabilities` in `slop.ts`, the media-capture delegate, entitlements, usage strings,
`ctx.ai` and `ctx.speech`). **This plan does not duplicate it.** The camera and mic slops
below are blocked on its phases 1, 2 and 6 (manifest field, capture gate, secure-context
probe). The speech and AI ideas wait for phases 3 to 5. Everything else here has no host
dependency and can start now.

Two corrections to earlier drafts of this plan that the capabilities plan settles:

- Authors declare `capabilities` in `slop.ts`, not in a `manifest.json`. The manifest
  file no longer exists in a slop directory.
- The consent UI and grant store are explicitly out of scope there, so these slops rely
  on declare-to-use plus the macOS one-time privacy prompt.

## Ground rules for every slop

**Layout** (as in `examples/slops/quick-checklist` and `shape-lab`): `slop.ts`,
`schema.ts`, `App.svelte`, `Export.svelte`, `Icon.svelte`, `styles.css`,
`svelte.config.js`, `tsconfig.json`, plus `README.md` and `DESIGN.md` when the design
needs explaining. Use `defineSlop` with `schema`, `initial`, a hex `theme` palette,
`presentation` and `categories`. Read `docs/guides/authoring.md` and use the
`hitslop-authoring` and `hitslop-design` skills.

**Document state is data, never library objects.** The schema holds equation strings,
ABC text, measurement rows, review state and attachment references. A MathLive field, a
JSXGraph board, a wavesurfer instance or a three.js scene is built from that data in the
view and thrown away with it.

**Platform rules** (`AGENTS.md`):

- Writes are async and resolve after the snapshot updates; `change` collectors are
  synchronous; reads come from immutable snapshots. Preserve `$id` identity.
- Attachments (photos, audio) are host-owned immutable blobs in the file. Never write
  your own blob store or JSON mirror.
- There is no network. Fonts, models and sample data ship inside the slop.
- A slop must keep working at its manifest window size, with keyboard operation, visible
  focus and reduced-motion behavior.
- Add a new dependency to `examples/slops/package.json` with an exact pin, and check its
  license before it ships inside a template.

**Compatibility.** Before launch we start fresh, so there are no migrations. A persisted
schema still has to be decided once, because after the first public release a newer
build must open every document a released build wrote. Slops that keep review or history
state (Exam Deck, Daily Frame, Lab Notebook) therefore get their schema reviewed before
they enter `bundled.json`.

**Registration.** A slop is developed under `examples/slops/<slug>` and added to
`bundled.json` only after review.

## The ideas

Each entry says what the slop is for, its readout, how it uses the libraries, a schema
sketch, and what could go wrong.

### Study

#### 1. Exam Deck (`ts-fsrs`, `@observablehq/plot`)

- **Job:** get a set of facts memorized by an exam date.
- **Readout:** "N cards today" and a bar strip forecasting the next 14 days, plus a
  "ready by exam" confidence figure.
- **Behavior:** you enter an exam date and a target retention. The load forecast is
  computed by simulating FSRS forward. A review session shows the card, four rating
  buttons and the next interval. Study can be capped to a daily minutes budget.
- **Schema sketch:** `deck` with `title`, `examDate`, `targetRetention`, and
  `cards: list(object({ front: text, back: text, state: FSRS card fields, due, reps,
  lapses, lastReview }))`, plus a `reviews` log of `{cardId, rating, at}` for the
  forecast. FSRS card state is plain numbers and dates; keep them as such.
- **Why not just update flashcards:** the archived `flashcards` is a Leitner box app. A
  deadline and a forecast change the product, not just the scheduler.
- **Needs from the platform:** nothing. Later, `ctx.ai` could draft cards from pasted
  notes, using the structured `output` of `slop-capabilities` phase 3a.
- **Risks:** the persisted FSRS state is the compatibility surface. Parameters (the
  weights) must be stored or versioned so old reviews stay meaningful. Decide this
  before shipping.

#### 2. Equation Notebook (`mathlive`, `jsxgraph`)

- **Job:** work through maths with typeset lines and a live graph.
- **Readout:** the typeset line you are on, with its graph beside it.
- **Behavior:** a vertical stack of cells. A cell is an equation (MathLive field), a
  text note, or a graph (JSXGraph function plot with slider parameters named in the
  equation). An equation cell can plot itself with one toggle. Exports as a clean
  one-page worked solution.
- **Schema sketch:** `cells: list(union-like object({ kind, latex: text, note: text,
  params: record of number, plot: boolean }))`. Store LaTeX strings only.
- **Risks:** MathLive ships fonts and its virtual keyboard; both must be bundled and
  checked under `slop://`. Check bundle size and consider a dynamic import. Parsing
  LaTeX into plottable functions needs a small, deliberately limited expression set.
  Say which functions work.

#### 3. Cheat Sheet (`mathlive`, `@chenglou/pretext`)

- **Job:** build the one page of formulas you are allowed to bring into an exam.
- **Readout:** the page itself, at true letter or A4 proportions, and how full it is.
- **Behavior:** formula boxes in columns; drag to reorder; a density meter warns when
  text would be too small to read printed. PDF export is the whole product.
- **Dependency:** reuses Equation Notebook's MathLive wiring, so build it second.

#### 4. Lab Notebook (`@observablehq/plot`)

- **Job:** turn measurements into a figure for a lab report.
- **Readout:** the chart.
- **Behavior:** a small data table; pick scatter, line or bar; optional linear fit with
  slope, intercept and R²; error bars; axis labels and units; export the figure as a
  PNG. It also works for anyone logging their own numbers.
- **Schema sketch:** `columns`, `rows`, `chart` settings, `fit` settings. Fit results
  are derived, never stored.
- **Risks:** be honest about statistics. Say what the fit is (ordinary least squares)
  and avoid claiming significance.

#### 5. Molecule Shelf (`3dmol`)

- **Job:** look at a molecule and turn it.
- **Readout:** the 3D molecule with its name and formula.
- **Behavior:** a bundled handful (water, caffeine, glucose, an amino acid, a short DNA
  fragment); paste PDB, SDF or XYZ; sticks, spheres or cartoon; atom labels; save a
  view as the icon. Everything bundled works offline, so no PDB download.
- **Schema sketch:** a list of `{name, format, data: text, style, view}` where `view` is
  the camera state.
- **Risks:** WebGL under `slop://`, bundle size, and the export path (capturing a WebGL
  canvas needs the canvas to be rendered and preserved at capture time).

#### 6. Concept Web (`vis-network`)

- **Job:** map how ideas connect while studying.
- **Readout:** the graph.
- **Behavior:** nodes and typed links ("causes", "contrasts with", "part of"); collapse
  branches; a "quiz me" mode hides one node and asks you to recall it. Idea for later:
  export the web into Exam Deck cards.
- **Risks:** `vis-network` is large and its layout is physics-driven, so layout must be
  stored (node positions), or reopening shuffles the picture.

#### 7. Study Timeline (`vis-timeline`)

- **Job:** see the shape of a thesis, project or exam season.
- **Readout:** a timeline with today marked.
- **Behavior:** drag-resizable bars, milestones, and groups per subject.
- **Note:** `assignment-tracker` (archived) is a list; this is the visual counterpart.

### Music and language

#### 8. Practice Sheet (`abcjs`)

- **Job:** learn a piece of music at a speed you can play.
- **Readout:** the rendered score with a moving playback cursor, and the current tempo.
- **Behavior:** type or paste ABC; play, loop a selection, slow down; a tempo ladder
  that raises the tempo a few bpm after each clean run; a practice log with a streak.
- **Schema sketch:** `abc: text`, `tempo`, `ladder` settings, `log: list({at, tempo,
  minutes})`.
- **Note:** pairs with the archived `metronome-tapper` for ideas, but is its own slop.

#### 9. Lecture Marker / Shadow Loop (`wavesurfer`)

- **Job:** annotate a recording, or loop a phrase until you can say it.
- **Readout:** the waveform with notes pinned to moments.
- **Behavior:** import audio into an attachment; drop timestamped notes; select a
  region and loop it at 0.5× to 1.25× speed. Version 2 adds recording yourself over the
  loop and comparing waveforms.
- **Needs from the platform:** v1 needs nothing. Recording needs `microphone` from
  `slop-capabilities`.
- **Risks:** large audio in attachments; confirm size limits and how playback streams
  from `slop://`.

### Real life, fun and tactile

#### 10. Room Planner (`moveable`)

- **Job:** lay out a room, event or classroom seating chart to scale.
- **Readout:** the plan, with dimensions.
- **Behavior:** furniture blocks you drag, rotate and resize with snapping; set the
  scale; show measurements.
- **Schema sketch:** `room: {width, height, unit}`, `items: list({label, x, y, w, h,
  rotation, color})`.

#### 11. Dice Tray (`threlte`)

- **Job:** roll dice for a tabletop game.
- **Readout:** the roll total and a short history.
- **Behavior:** 3D dice tumble in a tray with real physics; modifiers; saved history.
  Because it is small and self-contained, it is the lowest-risk test of Threlte inside
  a slop.
- **Risks:** Threlte plus a physics engine (rapier) is a heavier dependency. Start with
  a seeded random result and an animation that lands on it. Real physics can be added
  later if bundle size allows. WebGL under `slop://` and export capture also apply.

#### 12. Pachinko Picker (`matter-js`)

- **Job:** let a ball decide who does the dishes or where to eat.
- **Readout:** the bin the ball lands in.
- **Behavior:** options become bins; drop a ball; physics picks. A later variant is a
  physics lab (pendulum, projectile) with a live graph.
- **Risks:** a physics outcome is not reproducible, so store the result, not the
  simulation.

#### 13. Dashboard upgrades (`layerchart`)

Not a new slop. Consider moving expense and habit charts to `layerchart` only if doing
so deletes code, and only when those slops are brought back from the archive.

#### 14. Study Wall (`pixijs`, `moveable`)

Deferred. A corkboard of notes and photos does not need WebGL. Revisit only if we want
filters or particles.

### Camera and microphone (blocked on `plans/slop-capabilities.md`)

All of these run locally. Photos and recordings are stored as attachments.

#### 15. Daily Frame (camera)

- **Job:** one photo a day, played back as a flipbook.
- **Readout:** today's frame and the stack so far.
- **Behavior:** capture with `getUserMedia`, store the frame as an attachment, show "this
  day last month", and play the flipbook. For plants, art progress, workouts or a baby.
- **Schema sketch:** `frames: list({date, attachment, note})`.
- **Why first:** the simplest camera slop and a good proof of the capture gate.

#### 16. Quiet Meter (microphone)

- **Job:** show whether a room is too loud.
- **Readout:** a big green, amber, red light with a level.
- **Behavior:** a smoothed input level from Web Audio, adjustable thresholds, optional
  "too loud for N seconds" tally. No audio is stored.
- **Why first:** one job, one readout, no attachments, and it proves the mic gate.

#### 17. Tuner and Play-Along (microphone, `abcjs`)

- **Job:** tell whether you are in tune, then whether you played the right notes.
- **Readout:** the detected note and cents off.
- **Behavior:** version 1 is a tuner using autocorrelation pitch detection. Version 2
  follows a score from Practice Sheet and highlights each note as you play it.
- **Depends on:** Practice Sheet (#8).

#### 18. Rehearsal Room (microphone, optional camera, `wavesurfer`, Plot)

- **Job:** rehearse a talk and see how it went.
- **Readout:** talk time, pauses and loudness over time.
- **Behavior:** record, then play back against your notes. No speech recognition in v1,
  so no invented transcripts. Once `ctx.speech` lands, add a transcript and words per
  minute.

#### 19. Whiteboard Snap (camera)

- **Job:** photograph a whiteboard and straighten it.
- **Readout:** the straightened image beside your typed notes.
- **Behavior:** capture, drag four corners, perspective-correct with a small homography
  on canvas (no OpenCV).

#### 20. Sound Lab (microphone)

- **Job:** see a sound.
- **Readout:** a live oscilloscope and spectrogram, with the dominant frequency labelled.
- **Behavior:** works with a voice or a tuning fork; freeze a frame and save it.

#### 21. Pixel Booth / Sticker Booth (camera)

- **Job:** turn a webcam frame into pixel art, or a cut-out sticker.
- **Behavior:** version 1 is pixelate and palette-reduce only. Background removal needs
  a model, so check its bundle size before committing.

Held back: always-on presence detection that pauses a timer (privacy-sensitive and a
heavy ML bundle) and hand-gesture drawing (a gimmick that needs MediaPipe).

### Ideas that become possible with `ctx.ai` and `ctx.speech`

These are not planned now, but they shape the schemas so we don't paint ourselves into
a corner:

- Exam Deck: draft cards from pasted notes, structured `output` of a list of
  `{front, back}`.
- Concept Web: ask for related concepts to add around a selected node.
- Rehearsal Room: transcript, words per minute, filler words.
- Lecture Marker: transcribe a region, search by text.
- Practice Sheet: "explain this passage" is not worth it; skip.

## Order of work

1. **Batch 1, no host dependency (start now):** Exam Deck, Equation Notebook, Practice
   Sheet, Lab Notebook, Dice Tray. Between them they cover the libraries best, and they
   are the most useful or the most delightful.
2. **Batch 2:** Cheat Sheet (reuses Equation Notebook), Molecule Shelf, Concept Web,
   Room Planner, Pachinko Picker.
3. **Batch 3:** Study Timeline, Lecture Marker (import-only), dashboard upgrades if the
   archived slops return.
4. **Camera/mic batch, after `slop-capabilities` phases 1, 2 and 6:** Daily Frame and
   Quiet Meter first as the two proofs, then Tuner and Play-Along, Rehearsal Room,
   Whiteboard Snap, Sound Lab, Shadow Loop recording and Pixel/Sticker Booth. This runs
   in parallel with batches 1 to 3.

Within a batch, build the first slop completely (design, schema, export, icon, tests)
and use it to settle shared choices (chart style, card chrome, loading patterns for
heavy libraries) before starting the rest. Share code across slops only once two slops
actually need it; each slop stays self-contained.

## Per-slop definition of done

1. `slop.ts` with `defineSlop`, `schema`, `initial`, theme, `presentation` and
   categories. The document opens with a useful sample, not an empty state.
2. A single clear job and dominant readout that someone can identify within seconds of
   opening, with no onboarding.
3. `Export.svelte` and `Icon.svelte` that are recognizably the same object.
4. Works at the declared window size and resized, with keyboard operation, visible
   focus, enough contrast and reduced-motion behavior.
5. All assets bundled, no network at runtime, bundle size measured and recorded in the
   slop's README or DESIGN note.
6. Dependency license checked and the exact pin added to `examples/slops/package.json`.
7. Heavy objects are rebuilt from document data on open; closing and reopening shows the
   same state (including camera or layout state where relevant).
8. Tests at the owning boundary only: SDK-over-WASM for schema behavior, and Swift
   integration where the host is involved. No tests of CSS strings or private call
   sequences.

## Verification

```sh
bun slop dev examples/slops/<slug>
bun run check && bun run test
cargo test --locked --workspace
bun run build && bun run swift:test && bun run test:native
```

Manual, in a built app, for every slop: open, edit, close, reopen; undo; PNG and PDF
export; resize the window. For library-heavy slops, confirm rendering under `slop://`
specifically: MathLive fonts and virtual keyboard, WebGL for 3Dmol and Threlte, audio
playback for wavesurfer. The dev preview does not prove these. For camera and mic slops,
use a signed build, since macOS attributes privacy prompts to the launching app when run
from Xcode.

## Risks and open checks

- **Bundle size.** `mathlive`, `3dmol`, `vis-*`, `pixijs` and `threlte` plus a physics
  engine can each be large. Measure per slop and use dynamic imports where it helps.
- **WebKit under `slop://`.** The capabilities plan's probe (phase 6) confirms
  `navigator.mediaDevices` exists on the origin; the other libraries need their own real
  run in the host.
- **Licenses.** Check every package before it ships inside a template. For example,
  confirm JSXGraph's license terms for redistribution inside a bundled slop; do not
  assume.
- **Persisted FSRS and history state** is a compatibility surface once we release. Decide
  its shape deliberately.
- **Export of canvas and WebGL content** (3Dmol, Threlte, Pixi) must capture correctly at
  PNG and PDF export time.
- **Scope.** Fourteen to twenty slops is a lot. Ship batch 1 completely and use it to
  learn before starting the next.

## Out of scope

- Consent UI and the permission grant store (see `docs/ideas.md`)
- Porting or reviving archived slops
- Hosted catalog, accounts and sharing
- Always-on presence detection and gesture tracking
- Any change to `bundled.json` without review
