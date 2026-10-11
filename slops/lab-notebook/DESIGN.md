# Lab Notebook

## Job
Turn a few measurements into a figure for a lab report. It also works for anyone logging their own numbers.

## Readout
The chart. Under it, when the line is on, the least squares slope, intercept and R-squared in a ruled strip. The fit is ordinary least squares of y on x and is described as exactly that on screen and in the export; no significance, p-value or confidence claim is made.

## Object
A page from a bound lab notebook: warm paper with punched holes in the left margin, ink-black hand-drawn borders, a graph-paper figure card. Measured points are blue ink, the fit is a red dashed pencil line.

## Palette (slop.ts theme, all read by the CSS)
paper, card, ink, muted, rule, grid (graph paper lines), series (blue ink), fit (red). Figure colours reach Plot as `var(--slop-*)`, so the theme panel recolours the chart too. Dark mode is not offered: the object is paper.

## Type
Newsreader (headings, labels, prose) and Space Mono Bold (numbers, axis ticks, readouts), both bundled through Vite under the SIL OFL. Numbers are never in the body serif.

## Data model
`rows` hold `{label, x, y, err}` (y plus-or-minus err). Bar charts use `label` as the category and fall back to x. `kind`, `fit`, `errorBars`, axis labels and units are stored. Fit results, the caption and the chart SVG are derived from the rows and never stored. A fit is only drawn for scatter and line charts.

## Motion
None beyond 150 ms colour changes on controls, removed under `prefers-reduced-motion`.

## View state (local, not in the document)
The measured width of the figure stage (so the chart fills the window and follows resizing). Nothing else. Export shows the figure at a fixed 600 px width plus the data table when there are twelve rows or fewer.

## Rules for numbers
Cells accept decimals (a comma is read as a point). Anything that is not a finite number restores the previous value. Negative uncertainty is refused. At most 60 rows.

## Size
Built `dist/lab-notebook.slop`: about 709 KB, of which roughly 550 KB is the two bundled fonts; Plot and its d3 parts are the rest of the script.
