# Equation Notebook design

**Job:** work through a maths problem one typeset line at a time, with a live graph
beside any line.

**Readout:** the line you are on, swiped with a highlighter, with its graph beside it
(stacked under it when the window is narrow) and a slider for every letter it names.

## Object

A sheet torn from a green engineering computation pad. A printed title block across the
top (Problem, Lines, Sheet 1 of 1) in condensed technical lettering, a pale green grid,
and a red margin rule. Each line is a cell:

- **equation**: one MathLive field. The chart button in the left gutter plots it.
- **note**: plain words in the same roman face as the maths.
- **graph**: a larger plotting card above its function, with sliders.

Equation and graph lines are numbered in the right margin, (1), (2), as in a textbook.
The ⋯ menu beside each number inserts a line below, moves the line or removes it. Enter
moves to the next line, or starts a new equation after the last one.

## Plotting

`latex.ts` compiles a deliberately small set and refuses everything else with a plain
reason under "Can't plot this":

- forms: `y = …`, `f(x) = …`, `… = y`, a bare expression in x, or `left = right`, which is
  plotted as left − right and captioned "where it crosses zero, the line is true";
- numbers, `x`, the constants `e` and `\pi`, and single-letter parameters (any letter but
  x, y and e), which get sliders;
- `+ − \cdot \times / \div`, implicit multiplication, `^`, `\frac`, `\sqrt`, `\sqrt[n]`,
  brackets with or without `\left`/`\right`, and `|…|`;
- `\sin \cos \tan \sec \csc \cot \arcsin \arccos \arctan \sinh \cosh \tanh \ln \log \log_b
  \exp`, with `\sin^{-1}` read as `\arcsin` and `\sin^2 x` as `(\sin x)^2`.

Refused: inequalities and ≈, ±, words (`\text`), subscripts, sums, integrals, matrices,
more than one `=`, empty boxes. The y-range fits the curve over the saved x-window (its
middle 90% when an asymptote would flatten the rest).

## What is saved, what is view state

Saved (`schema.ts`): the title, and each cell's kind, LaTeX string, note text, plot
toggle, slider values (`params`, a record keyed by letter; a letter with no entry reads
as 1) and x-window. Only LaTeX strings are stored, never MathLive or JSXGraph objects.

View state: the active line, slider values mid-drag (committed on release or key press),
the open menu and the trace point. Typing previews the LaTeX live (the graph follows) and
commits on Enter, Tab or blur. Zoom buttons and +/− on a focused graph write the window.

The MathLive field and the JSXGraph board are built from that data and freed on unmount.
JSXGraph is loaded with a dynamic import so it evaluates only when a graph shows; the
build inlines it (`codeSplitting: false`), so this saves start-up work, not bytes. The
curve is a parametric `curve` with plain functions: `functiongraph` builds x(t) through
JessieCode, which needs `eval`, and the page CSP blocks it.

## MathLive under slop://

No CDN. The KaTeX fonts are bundled (copies of `mathlive/fonts` in `assets/fonts/katex`,
declared in `styles.css`; `--ML__static-fonts` tells MathLive not to load them itself,
and `fontsDirectory` is `null`). `mathlive-static.css` is MathLive's static stylesheet
without its `@font-face` rules (they point into `node_modules`, which `slop dev` does not
serve). Sounds are off (`soundsDirectory`, `keypressSound`, `plonkSound` are `null`).
The virtual keyboard is `manual`: its toggle shows in a field on hover or focus and opens
MathLive's own keyboard, which is plain DOM with no network. The MathLive menu is hidden.

## Palette, type, motion

Theme tokens: `paper` #eef3e1, `grid` #cddfc0, `header` #dfe9cc, `margin` #cf8f7c,
`ink` #1e2b23, `muted` #55685b, `pencil` #3b4a40, `curve` #c2412d (red pencil),
`accent` #255f86 (focus and caret), `highlight` #f5e48c, `plotPaper` #fbfcf6. The graph
reads them from CSS when it is built and rebuilds when the root's style changes.

Type: KaTeX Main and Math for maths and notes; Barlow Condensed SemiBold for the title
block, buttons and menu. Motion: only short opacity and colour transitions, which
`hitslop/base.css` removes under reduced motion. Nothing animates continuously.

## Export and icon

Export is a fresh page with the same title block and numbered lines: the maths typeset
by MathLive's static renderer, notes, and each graph drawn as plain SVG from its saved
window and slider values (no library, nothing asynchronous to wait for), with the values
listed under it. The icon is a corner of the pad: grid, margin rule, a highlighted
numbered line and a red parabola with its two roots.

## Size

Built `dist/equation-notebook.slop`: 905,216 bytes. `ui.js` is 2,131,495 bytes
(454,791 Brotli-compressed in the file; MathLive and JSXGraph are most of it), `ui.css`
25,457, `commands.js` 8,823; 21 font files total 369,220 bytes (20 KaTeX woff2, 296 KB, and
Barlow Condensed, 109 KB).
