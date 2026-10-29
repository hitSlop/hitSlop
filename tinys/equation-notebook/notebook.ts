import { refuse, type Tx, type Value } from "@hitslop/tinys";
import type schema from "./schema";
import { maxLatex, paramLimit, windowLimit, type Kind } from "./schema";
import { compilePlot, isParamName, type Params, type Plot } from "./latex";

export type Notebook = Value<typeof schema.descriptor>;
export type Cell = Notebook["cells"][number];
type Edit = Tx<typeof schema>;

export const defaultWindow = { xMin: -10, xMax: 10 } as const;
const minSpan = 0.01;

export type Line = {
  cell: Cell;
  /** Equation number shown in the right margin; notes are unnumbered (0). */
  number: number;
  plot: Plot | null;
  graphed: boolean;
};

/** The editor and the export derive numbering, plots and counts from the same snapshot. */
export function notebookView(data: Notebook) {
  let number = 0;
  const lines: Line[] = data.cells.map(cell => {
    const isMath = cell.kind !== "note";
    return {
      cell,
      number: isMath ? ++number : 0,
      plot: isMath ? compilePlot(cell.latex) : null,
      graphed: cell.kind === "graph" || (cell.kind === "equation" && cell.plot),
    };
  });
  return {
    lines,
    title: data.title.trim() || "Untitled problem",
    equations: number,
    graphs: lines.filter(line => line.graphed).length,
  };
}

/** Each named letter's value: the stored slider value, or 1 for a letter never moved. */
export function paramValues(cell: Cell, names: readonly string[], overrides: Params = {}): Record<string, number> {
  return Object.fromEntries(names.map(name => [name, overrides[name] ?? cell.params[name] ?? 1]));
}

export function sliderRange(value: number) {
  return { min: Math.min(-10, Math.floor(value)), max: Math.max(10, Math.ceil(value)), step: 0.1 };
}

/** A y-range that frames the curve over the window (its middle 90% when an asymptote
 * would flatten everything else) and keeps the x-axis in view when it is near. */
export function yRange(plot: Plot | null, params: Params, xMin: number, xMax: number): [number, number] {
  if (!plot?.ok) return [-10, 10];
  const ys: number[] = [];
  for (let i = 0; i <= 240; i++) {
    const y = plot.evaluate(xMin + (xMax - xMin) * i / 240, params);
    if (Number.isFinite(y)) ys.push(y);
  }
  if (!ys.length) return [-10, 10];
  ys.sort((a, b) => a - b);
  let lo = ys[0]!, hi = ys[ys.length - 1]!;
  const p5 = ys[Math.floor(ys.length * 0.05)]!, p95 = ys[Math.ceil(ys.length * 0.95) - 1]!;
  // An asymptote stretches the full range far past the body of the curve: frame the body.
  if (hi - lo > 4 * Math.max(p95 - p5, 1e-6)) { lo = p5; hi = p95; }
  if (hi - lo < 1e-6) { lo -= 1; hi += 1; }
  const span = hi - lo;
  if (lo > 0 && lo < span) lo = 0;
  if (hi < 0 && -hi < span) hi = 0;
  const pad = (hi - lo) * 0.12;
  return [lo - pad, hi + pad];
}

/** A tick step of 1, 2 or 5 × 10ⁿ giving about `target` ticks. */
export function tickStep(span: number, target = 8) {
  const raw = span / target, power = 10 ** Math.floor(Math.log10(raw)), unit = raw / power;
  return (unit < 1.5 ? 1 : unit < 3.5 ? 2 : unit < 7.5 ? 5 : 10) * power;
}

/** The curve as SVG path data in a width × height box, broken at gaps and asymptotes. */
export function curvePath(plot: Plot | null, params: Params, xMin: number, xMax: number, y: [number, number], width: number, height: number) {
  if (!plot?.ok) return "";
  const [yMin, yMax] = y, steps = Math.max(120, Math.round(width * 1.5));
  const sx = (x: number) => (x - xMin) / (xMax - xMin) * width;
  const sy = (v: number) => height - (v - yMin) / (yMax - yMin) * height;
  let d = "", previous: number | null = null;
  for (let i = 0; i <= steps; i++) {
    const x = xMin + (xMax - xMin) * i / steps, v = plot.evaluate(x, params);
    if (!Number.isFinite(v)) { previous = null; continue; }
    const py = Math.max(-height * 2, Math.min(height * 3, sy(v)));
    const jump = previous !== null && Math.abs(py - previous) > height * 1.5;
    d += `${previous === null || jump ? "M" : "L"}${sx(x).toFixed(1)} ${py.toFixed(1)}`;
    previous = py;
  }
  return d;
}

/** Describes the plotted function for captions and screen readers. */
export function plotCaption(plot: Plot | null) {
  if (!plot) return "";
  if (!plot.ok) return plot.reason;
  return plot.mode === "difference" ? "Plotted as left − right: where it crosses zero, the line is true." : "";
}

function cellIn(current: Notebook, id: string) {
  return current.cells.find(cell => cell.$id === id) ?? refuse("There is no cell with that id.");
}
const finite = (value: number, what: string) => { if (!Number.isFinite(value)) refuse(`${what} must be a number.`); };

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addCell(tx: Edit, kind: Kind, content: { latex?: string; note?: string; plot?: boolean; after?: string } = {}) {
  const latex = (content.latex ?? "").trim();
  if (kind === "note" && latex) refuse("A note holds words; put LaTeX in an equation or graph cell.");
  if (kind !== "note" && content.note) refuse("Only a note cell holds words.");
  if (latex.length > maxLatex) refuse(`Keep a line under ${maxLatex} characters.`);
  const row = {
    kind,
    latex,
    note: content.note ?? "",
    plot: kind === "equation" && !!content.plot,
    ...(kind === "graph" ? defaultWindow : {}),
  };
  return tx.fields.cells.insert(row, content.after ? { after: content.after } : undefined).id;
}

export function setLatex(tx: Edit, current: Notebook, id: string, latex: string) {
  if (cellIn(current, id).kind === "note") refuse("That cell is a note; it holds words, not LaTeX.");
  tx.fields.cells.item(id).latex.set(latex.trim());
}

export function setNote(tx: Edit, current: Notebook, id: string, text: string) {
  if (cellIn(current, id).kind !== "note") refuse("Only a note cell holds words.");
  tx.fields.cells.item(id).note.set(text);
}

export function setPlot(tx: Edit, current: Notebook, id: string, plot: boolean) {
  const cell = cellIn(current, id);
  if (cell.kind === "note") refuse("A note has nothing to plot.");
  if (cell.kind === "graph") refuse("A graph cell always plots.");
  tx.fields.cells.item(id).plot.set(plot);
}

export function setParam(tx: Edit, current: Notebook, id: string, name: string, value: number) {
  const cell = cellIn(current, id);
  if (cell.kind === "note") refuse("A note has no parameters.");
  if (name === "x") refuse("x is the plotting variable, not a parameter.");
  if (!isParamName(name)) refuse("A parameter is one letter other than x, y or e.");
  finite(value, "The value");
  if (Math.abs(value) > paramLimit) refuse(`Keep parameters between −${paramLimit} and ${paramLimit}.`);
  tx.fields.cells.item(id).params.put(name, Math.round(value * 1000) / 1000);
}

export function setWindow(tx: Edit, current: Notebook, id: string, xMin: number, xMax: number) {
  if (cellIn(current, id).kind === "note") refuse("A note has no graph.");
  finite(xMin, "The left edge"); finite(xMax, "The right edge");
  if (xMax - xMin < minSpan) refuse("The right edge must be to the right of the left edge.");
  if (Math.abs(xMin) > windowLimit || Math.abs(xMax) > windowLimit) refuse(`Keep the graph between −${windowLimit} and ${windowLimit}.`);
  const row = tx.fields.cells.item(id);
  row.xMin.set(xMin);
  row.xMax.set(xMax);
}

/** Zooms about the middle by `factor` (below 1 zooms in), within the stored limits. */
export function zoomedWindow(xMin: number, xMax: number, factor: number) {
  const middle = (xMin + xMax) / 2;
  const half = Math.min(windowLimit / 2, Math.max(minSpan, (xMax - xMin) * factor) / 2);
  const round = (v: number) => Math.round(v * 1e4) / 1e4;
  const shift = Math.max(0, Math.abs(middle) + half - windowLimit) * Math.sign(middle);
  return [round(middle - shift - half), round(middle - shift + half)] as const;
}

export function moveCell(tx: Edit, current: Notebook, id: string, direction: -1 | 1) {
  const index = current.cells.findIndex(cell => cell.$id === id);
  const neighbor = current.cells[index + direction];
  if (index < 0 || !neighbor) return false;
  tx.fields.cells.move(id, direction === -1 ? { before: neighbor } : { after: neighbor });
  return true;
}
