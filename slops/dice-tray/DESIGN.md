# Dice Tray design

**Job:** roll dice for a tabletop game. **Readout:** the last roll's total on a brass
plate, with what was rolled ("1d20 + 1d4 + 5") and how it added up ("17 + 3 + 5"), and a
short score slip of recent rolls.

## The object

The window is a walnut dice tray. From top to bottom: the brass plate (title, formula,
breakdown, a large total), the felt well where 3D dice tumble and rest, a dark wooden rack
with one slot per die (d4, d6, d8, d10, d12, d20), the modifier stepper and a wide brass
**Roll** button that names the pool, and a paper slip with the history (newest first, each
row has "roll again"). A lone d20 that comes up 20 or 1 gets a "Natural 20" / "Nat 1" tag,
in words, not only colour.

- **Palette** (theme keys, all read by `styles.css`): `wood`, `woodDark` (frame, grain,
  rack), `felt`, `feltDeep` (well and the 3D floor), `die`, `pip` (dice body and numerals,
  both flat and 3D), `brass` (plate, Roll), `paper`, `ink`, `muted`, `rule` (slip).
  The scene reads `--tray-*` custom properties that point at the theme, and re-paints the
  dice atlases when the theme changes.
- **Type:** Fraunces (variable, bundled) for numerals: the total, formulas, the Roll verb and
  the numbers painted on the dice. System sans for labels and the slip's small text.
- **3D:** Threlte (`Canvas`, `T`, `useTask`, `useThrelte`) over three.js. Dice are flat-shaded
  polyhedra built in `dice3d.ts`; each kind gets one numeral atlas drawn on a canvas, so any
  face can be read. d6 uses pips, d4 prints three numbers per face with the result at the top
  corner, d10/d12/d20 underline 6 and 9. A nearly overhead camera makes the face that came up
  the one you read. Renders on demand; idle frames cost nothing.

## Rolls, randomness and motion

There is no physics engine. A roll draws every value from an explicit random source (the page
passes `crypto.getRandomValues`, commands use `ctx.random`) and stores the values, modifier,
time and a `seed`. The seed only shapes the tumble and the resting layout; the animation is a
pure function of seed and elapsed time that ends exactly on the stored values, so the editor,
a reopened document and the export show the same arrangement.

The tumble lasts about 1.2–1.6 s; the plate shows "Rolling…" until the dice land, then
announces the result through a polite live region. With reduced motion the dice appear at
rest at once and the plate never waits. A roll already present when the window opens, or one
revisited by undo, rests without tumbling. Resizing lands a tumble immediately.

## Saved vs view state

Saved: title, the pool (count per kind, 12 dice at most), modifier, and the rolls (oldest
dropped beyond 60). The total, formula, breakdown and natural tag are derived in `dice.ts`
(`trayView`, `rollView`), which App and Export both use. View state: which roll is mid-tumble,
the motion preference, the clock used for "8:07 PM", and every three.js object.

## Keyboard and access

Every control is a button with a spoken label ("Add a d20, 1 in the pool", "Remove a d20",
"Roll 1d20 + 1d4 + 5 again"). **R** rolls from anywhere except while typing; right-click on a
rack die removes one. Focus rings are paper on wood and ink on the slip. The well is an
`img` role whose label lists the dice showing.

## Export and icon

Export is a fresh page: it mounts the tray in still mode, renders one frame of the last roll at
rest, keeps it as a PNG data URL and unmounts the WebGL canvas (context released) before
capture, through `capture.onPrepare`. Without WebGL it shows flat die drawings with the values.
Below it: the plate and up to twelve recent rolls, no controls. The icon is the tray from
above: walnut rim, felt, a d20 showing the last d20 value and a d6.

## Edits

Written once in `dice.ts` and shared by the page and `commands.ts`: `roll`, `rollFormula`,
`rollAgain`, `setPool`/`setFormula`, `addDie`, `removeDie`, `setModifier`, `clearHistory`,
`rename`. Formulas accept `NdX` terms and a signed modifier ("1d8 + 3d6 + 4"); unknown dice,
more than 12 dice and modifiers outside ±99 are refused in words.

## Size (built 2026-10-10)

`dist/dice-tray.slop` is 610,304 bytes. Inside: `ui.js` 847,996 bytes (188,696 Brotli),
`ui.css` 11,892, `commands.js` 8,848, Fraunces 360,440. three.js is most of `ui.js`; Threlte's
`T` catalogue imports all of three, which costs about 210 KB raw over a plain three.js slop
such as Tiny Planet.

## Unverified

Rendering, tumbling, reduced motion, a 360 px width and the export still were checked in
headless Chromium through `slop dev`. Not verified: WebGL and the tumble under `slop://` in
WKWebView, native PNG/PDF export timing of the still, theme overrides in the native panel.
