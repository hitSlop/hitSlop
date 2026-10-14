# Pocket Sheet

One job: do a little everyday math in a grid, such as splitting a bill, tallying
points, or pricing a shopping list.

A single sheet of graph paper sits on a warm desk. A red notebook margin, a
Marker Felt title, and blue grid lines keep it friendly; SF Mono numbers stay
tabular and right-aligned so totals are easy to scan. Formula cells carry a small
red corner tick. When an edit changes a formula's result, that value hops once
and a sparkle twinkles; reduced motion removes both.

The grid is fixed at A–F × 1–12. Cells hold plain text, numbers (`1,200`, `15%`),
or formulas starting with `=`: `+ - * / ^ &`, comparisons, ranges, and SUM,
AVERAGE, MIN, MAX, COUNT, ROUND, ABS, IF. Errors appear in the cell (`#DIV/0!`,
`#REF!`, `#CYCLE!`) and flow downstream instead of breaking the sheet.

Keyboard works like a spreadsheet: arrows (Shift extends, ⌘ jumps to the edge),
Tab, Enter/F2 edits, typing replaces, Delete clears, ⌘A selects all, ⌘C/⌘X copy
values as tab-separated text, and paste fills from the active cell. Drag or
Shift-click selects a range. The formula bar edits the active cell's raw input.

Five crayons tint the selection, pressing the active crayon again erases it, and
an emoji stamp pad marks cells. The footer adds up the selected numbers. Column edges
drag to resize and double-click to reset.

Export CSV writes the used area as UTF-8 with a BOM, using raw entries, so formulas
travel as `=…` and recalculate when opened in Numbers, Excel, or Google Sheets. In
the Mac app, the host asks where to save it. Tints, stamps, and column widths
stay in the slop. PNG/PDF export shows the titled sheet without editing chrome.
The icon is a graph-paper corner with a pizza, a number, and a sparkle.

Not included: multiple sheets, xlsx import, xlsx files, undo UI, pointing at cells to build formulas,
number formats, or merged cells. Resizing and range selection borrow ideas from
the MIT svelte-sheets project, rewritten here.
