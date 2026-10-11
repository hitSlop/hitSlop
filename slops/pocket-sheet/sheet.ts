import { refuse, type Tx } from "hitslop";
import type doc from "./schema";
import { tints } from "./schema";
import { cellKey, columnName, COLUMNS, display, evaluate, isError, parseKey, ROWS, type Value } from "./formula";

type Edit = Tx<typeof doc>;
export type Data = typeof doc.current;
export type Tint = (typeof tints)[number];

export const DEFAULT_WIDTH = 96;
export const MIN_WIDTH = 56;
export const MAX_WIDTH = 320;

/** The editor and its captures evaluate the same snapshot, so they cannot disagree. */
export function sheetView(data: Data) {
  const inputs = Object.fromEntries(Object.entries(data.cells).map(([key, cell]) => [key, cell.input]));
  return { inputs, results: evaluate(inputs) };
}

export const widthOf = (data: Data, column: number) => data.widths[columnName(column)] ?? DEFAULT_WIDTH;
export const gridTemplate = (width: (column: number) => number) =>
  `40px ${Array.from({ length: COLUMNS }, (_, column) => `${width(column)}px`).join(" ")}`;

export const kindOf = (value: Value | undefined) =>
  value === undefined || value === "" ? "blank" : isError(value) ? "error" : typeof value === "number" ? "number" : "text";

export const cellLabel = (key: string, cell: Data["cells"][string] | undefined, value: Value | undefined) =>
  `${key}${cell?.stamp ? ` ${cell.stamp}` : ""}: ${display(value ?? "") || "empty"}${cell?.input.startsWith("=") ? `, formula ${cell.input}` : ""}`;

/** "B2" or "B2:C4" (either corner order) as the cell keys it covers, row by row. */
export function rangeKeys(range: string): string[] {
  const [from, to = from, extra] = range.split(":");
  const a = parseKey(from ?? ""), b = parseKey(to);
  if (extra !== undefined || !a || !b) refuse(`Use a cell like B2 or a range like B2:C4 (columns A-${columnName(COLUMNS - 1)}, rows 1-${ROWS}).`);
  const keys: string[] = [];
  for (let row = Math.min(a.row, b.row); row <= Math.max(a.row, b.row); row++)
    for (let column = Math.min(a.column, b.column); column <= Math.max(a.column, b.column); column++)
      keys.push(cellKey(column, row));
  return keys;
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.

/** Sets the typed input of cells; an emptied cell with no tint or stamp is removed. */
export function writeCells(tx: Edit, data: Data, entries: readonly (readonly [string, string])[]) {
  for (const [key, input] of entries) {
    if (!parseKey(key)) refuse(`${key} is not a cell on this sheet.`);
    if (input.length > 500) refuse("A cell holds at most 500 characters.");
    const cell = data.cells[key];
    if (!cell) { if (input) tx.fields.cells.put(key, { input }); continue; }
    if (!input && cell.tint === undefined && cell.stamp === undefined) tx.fields.cells.delete(key);
    else if (cell.input !== input) tx.fields.cells.entry(key).input.set(input);
  }
  return entries.length;
}

/** Colors cells; `null` erases. Painting a selection that already wears the tint erases it. */
export function tintCells(tx: Edit, data: Data, keys: readonly string[], tint: Tint | null) {
  if (tint !== null && !tints.includes(tint)) refuse(`Pick a crayon: ${tints.join(", ")}.`);
  const clearing = tint === null || keys.every(key => data.cells[key]?.tint === tint);
  for (const key of keys) {
    const cell = data.cells[key];
    if (!cell) {
      if (!clearing) tx.fields.cells.put(key, { input: "", tint: tint! });
    } else if (clearing) {
      if (!cell.input && cell.stamp === undefined) tx.fields.cells.delete(key);
      else if (cell.tint !== undefined) tx.fields.cells.entry(key).tint.clear();
    } else tx.fields.cells.entry(key).tint.set(tint!);
  }
  return { cleared: clearing };
}

/** Stamps cells; `null` removes the stamp. */
export function stampCells(tx: Edit, data: Data, keys: readonly string[], mark: string | null) {
  if (mark !== null && (!mark || mark.length > 16)) refuse("Pick a stamp of 1 to 16 characters.");
  for (const key of keys) {
    const cell = data.cells[key];
    if (!cell) {
      if (mark !== null) tx.fields.cells.put(key, { input: "", stamp: mark });
    } else if (mark === null) {
      if (!cell.input && cell.tint === undefined) tx.fields.cells.delete(key);
      else if (cell.stamp !== undefined) tx.fields.cells.entry(key).stamp.clear();
    } else tx.fields.cells.entry(key).stamp.set(mark);
  }
  return keys.length;
}

/** Sets a column's width in pixels, or restores the default when `width` is null. */
export function setColumnWidth(tx: Edit, data: Data, letter: string, width: number | null) {
  const column = letter.length === 1 ? letter.toUpperCase().charCodeAt(0) - 65 : -1;
  if (column < 0 || column >= COLUMNS) refuse(`Columns are A-${columnName(COLUMNS - 1)}.`);
  const key = columnName(column);
  if (width === null) { if (data.widths[key] !== undefined) tx.fields.widths.delete(key); return; }
  if (!Number.isInteger(width) || width < MIN_WIDTH || width > MAX_WIDTH) refuse(`Width is ${MIN_WIDTH} to ${MAX_WIDTH} pixels.`);
  tx.fields.widths.put(key, width);
}
