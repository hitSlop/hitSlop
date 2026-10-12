# Catalog capture review — 2026-10-12

The catalog needs a focused capture-styling pass before broad native-rendering
qualification. Most compositions remain recognizable, including the rich browser
captures. The urgent findings are six blank icons, four icons with lost SVG
styling, and two exports that lose their main content. These are open findings;
this review does not change the slops or renderer routing.

## Scope and method

Freshly built all 76 entries in `slops/bundled.json`, which also covers every active
`slops/*/slop.ts` at review time. Captured an icon and PNG export from each disposable
document's initial saved state and default theme using the installed macOS helper
and evaluator. All 152 captures completed without errors. The builder selected
Rust for 68 icons and 38 exports, and WebKit for 8 icons and 38 exports.

Reviewed all icons and exports on contact sheets. Repeated seven suspect cases
with a disposable definition omitting the private native capture programs, so
the same component, state and resources render through WebKit. Inspected those
comparisons side by side: Ivy Lee Method, Practice Sheet, Fog, Household Chores,
MASH, Grocery List and Pixel Art. The comparison confirms visible regressions in
their native captures; successful completion is not visual acceptance.

This is a first visual sweep, not a pixel-diff qualification, theme matrix,
long-document check, PDF review or populated-state check for initially empty
templates. No per-slop automated tests were added. Local scripts and images remain
under ignored `spikes/takumi-capture/`: `catalog-review.ts`, `catalog-compare.ts`,
`out/catalog-review/` and `out/catalog-compare/`. The first output folder includes
the 76 pairs, routing results, two icon sheets, nine export sheets and three
comparison sheets. These local artifacts are not required to build the repository.

## Fix first

| Slop / role | Observed result | Suggested repair |
| --- | --- | --- |
| Choice Point — Icon | Blank | Remove the role's hidden-by-default dependency or make the shared native capture wrapper honor the existing capture attributes. Then check the fork's SVG stroke styles. |
| Cornell Notes — Icon | Blank | Same capture-attribute dependency; retain the recognizable cue/notes/summary paper silhouette. |
| Grade Calculator — Icon | Blank | Same capture-attribute dependency; restore the grade instrument composition. |
| Ivy Lee Method — Icon | Blank; WebKit shows the six-slot paper card | Same capture-attribute dependency. |
| Semester Planner — Icon | Blank | Same capture-attribute dependency; restore the four-month calendar composition. |
| Three Three Three — Icon | Blank | Same capture-attribute dependency; restore the three task bands. |
| Practice Sheet — Icon | Solid black rounded tile; WebKit shows the staff, notes and tempo seal | Give inline SVG explicit fill, stroke and text presentation attributes rather than relying on page CSS classes. |
| Fog — Icon | Nearly black tile with a barely visible smile; WebKit shows the mint glass | Materialize SVG gradient stop colors and frost fills. A static mint smile is sufficient; camera or animation fidelity is unnecessary. |
| Household Chores — Icon and Export | Wheel becomes a black disk | Fix the shared `Wheel.svelte` SVG styling so the wheel and points readout survive standalone SVG rendering. |
| MASH — Icon | Spiral becomes a black filled shape; title is also much smaller | Set the spiral's `fill="none"`, stroke and stroke width explicitly, then use a capture-safe title size. |
| Grocery List — Export | Notepad collapses to a clipped header; grocery rows disappear while the sticky note remains | Give the export a content-sized notepad with visible overflow and no editor flex sizing; account for its capture-mode CSS. WebKit shows all eight initial items. |
| Pixel Art — Export | 16×16 sprite collapses into a tiny dark shape on a wide strip | Establish explicit square bezel/grid dimensions and cell tracks in the export. Verify all initial sprite cells remain visible. WebKit renders the complete sprite correctly. |

The six blank icon views are styled as `display: none !important` until selectors
matching `html[data-slop-renderer="true"][data-slop-capture="icon"]` reveal them.
The native adapter currently emits `<html data-slop-capture>` without those values.
This is a shared host-markup mismatch, not six unrelated design problems. Inspect
[`capture-render.ts`](../../packages/hitslop/src/sdk/app/capture-render.ts) and the
affected slops' `styles.css` together. A shared fix belongs at that boundary, with
a minimal infrastructure fixture rather than tests depending on these slops.

Inline SVG is a separate issue: materializing theme variables in attributes does
not bring page CSS classes into the standalone SVG image. Practice Sheet's staff,
Fog's gradient stops and MASH's spiral rely on those classes. Their existing
designs can be retained with explicit SVG presentation attributes. Household
Chores uses the same wheel in both roles, so review both after changing it.

Pixel Art combines `width: min(100%, 420px)`, `aspect-ratio`, percentage heights and
fractional grid tracks. The comparison establishes the sizing failure but does
not isolate which declaration is responsible. Prefer a small export-only sizing
fix before changing general renderer behavior. Keep WebKit as the qualification
baseline for these cases; the current native eligibility check establishes
executable compatibility, not CSS fidelity.

## Smaller improvements

- **Departure Board — Icon:** enlarge the board face inside its tile. The current
  tiny center strip loses the board's identity at Finder sizes.
- **Quick Checklist — Export:** consider trimming the large empty background below
  short lists while keeping the paper treatment. Its dedicated widget layouts are
  already more deliberate about the available space.
- **Choice Point and Small Expenses — Export:** these are the only two catalog
  entries without `Export.svelte`. Their App fallback works, but a static summary
  would remove editing controls. Choice Point could emphasize the towards/away
  decision map; Small Expenses could use a compact total and itemized receipt.
- **MASH — Export:** the long narrow list is readable at full size but could group
  choices more compactly for sharing. This is lower priority than its icon.

Koi Pond's pond still, Little Lamp, Tiny Planet, Codex Pet and the music/radio
objects communicate their identity adequately as static captures. Reproducing
live p5, 3D, audio or animation is not a requirement for this pass. An empty
Doodle Board or Moodboard initial export is also not evidence of dropped content;
populated documents need a separate check. Dice Tray retains its total and roll
history without requiring a perfect still of the physical dice.

## Eight widget candidates

Quick Checklist is already implemented. The following are proposed additions,
ordered roughly by useful glanceable content and implementation effort. Each
needs a dedicated `Widget.svelte`; shrinking the full export is insufficient.

| Candidate | Small | Medium / large | Notes |
| --- | --- | --- | --- |
| Water Tracker | Filled gauge, amount and target percentage | Gauge plus today's saved drink entries | Strong visual identity and simple saved-state data. Start here. |
| Ivy Lee Method | Next unfinished priority and completed count | Ordered six-task docket | Very close to Quick Checklist's data needs. Fix its icon separately. |
| Grocery List | Remaining count and first few items | Remaining items grouped by category, optional memo | Useful while planning a shop. Use fresh widget sizing rather than the currently broken export CSS. |
| Reading Tracker | Currently reading title and author | Current books, next-up titles and finished count | All fields exist today; do not imply page progress or a reading-goal field that the schema lacks. |
| Habit Heatmap | One habit's recent cells and today's completion | Several habit rows with a compact recent calendar | Daily rollover needs approximate scheduled refresh and synchronous date formatting. |
| Daily Planner | Saved day's leading priority | Top priorities and a few time blocks | Label the plan's date; an old saved plan must not masquerade as today's plan. |
| Assignment Tracker | Nearest incomplete due item | Upcoming due items and overdue count | Use capture-safe date formatting; relative due labels need approximate refresh. |
| Countdown Milestones | Event name and whole days remaining | Main event plus milestone checklist | Use approximate refresh; avoid seconds, minute-accurate promises and browser-only date helpers. |

All are image widgets: clicking opens the document; the image itself does not
provide checkboxes, drink logging or other inline edits. Successful saves trigger
image refresh while hitSlop runs. Time-sensitive views can request
`widget.refreshMinutes` (15–1440), but macOS controls visible reload timing and
the extension cannot run the renderer while the host app is quit. Show dates and
saved context when needed to make stale images understandable.

Recommended order: fix the shared blank-icon boundary and the two content-losing
exports, repair the SVG attributes, then add Water Tracker, Ivy Lee Method and
Grocery List widgets. The remaining five are a second authoring pass, with the
time-dependent ones receiving explicit stale-image review.
