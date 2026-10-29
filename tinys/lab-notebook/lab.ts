import { refuse, type RowRef, type Tx, type Value } from "@hitslop/tinys";
import type schema from "./schema";

export const kinds = ["scatter", "line", "bar"] as const;
export type Kind = (typeof kinds)[number];
export const maxRows = 60;

export type Notebook = Value<typeof schema.descriptor>;
type Edit = Tx<typeof schema>;

export type Point = { id: string; label: string; x: number; y: number; err: number };
/** An ordinary least squares line. `r2` is null when every y is the same (R-squared is undefined). */
export type Fit = { slope: number; intercept: number; r2: number | null; n: number };

/** Ordinary least squares of y on x. Null with fewer than two distinct x values. */
export function olsFit(points: readonly { x: number; y: number }[]): Fit | null {
  const n = points.length;
  if (n < 2) return null;
  const mx = points.reduce((sum, p) => sum + p.x, 0) / n;
  const my = points.reduce((sum, p) => sum + p.y, 0) / n;
  let sxx = 0, sxy = 0, syy = 0;
  for (const p of points) {
    sxx += (p.x - mx) ** 2;
    sxy += (p.x - mx) * (p.y - my);
    syy += (p.y - my) ** 2;
  }
  if (sxx === 0) return null;
  const slope = sxy / sxx;
  const intercept = my - slope * mx;
  return { slope, intercept, r2: syy === 0 ? null : (sxy * sxy) / (sxx * syy), n };
}

/** Three significant figures (trailing zeros kept: they state the precision), no exponent for ordinary magnitudes. */
export function sig(value: number, digits = 3): string {
  if (!Number.isFinite(value)) return "-";
  if (value === 0) return "0";
  const abs = Math.abs(value);
  if (abs >= 1e6 || abs < 1e-3) return value.toExponential(digits - 1).replace("e+", "e");
  const text = value.toPrecision(digits);
  return text.includes("e") ? String(Number(text)) : text;
}

/** "Load (N)", "Load", "(N)" or an empty string. */
export function axisTitle(label: string, unit: string): string {
  const name = label.trim();
  const u = unit.trim();
  return name && u ? `${name} (${u})` : name || u;
}

/** The editor and the export derive points, the fit and the caption from the same snapshot. */
export function notebookView(data: Notebook) {
  const points: Point[] = data.rows.map(row => ({
    id: row.$id, label: row.label.trim(), x: row.x, y: row.y, err: row.err,
  }));
  const fitApplies = data.kind !== "bar";
  const fit = data.fit && fitApplies ? olsFit(points) : null;
  const xTitle = axisTitle(data.xLabel, data.xUnit);
  const yTitle = axisTitle(data.yLabel, data.yUnit);
  const tinyeUnit = data.xUnit.trim() && data.yUnit.trim() ? ` ${data.yUnit.trim()}/${data.xUnit.trim()}` : "";
  const interceptUnit = data.yUnit.trim() ? ` ${data.yUnit.trim()}` : "";
  let note = "";
  if (data.fit && !fitApplies) note = "A fit needs a scatter or line chart.";
  else if (data.fit && !fit) note = "A fit needs two rows with different x values.";
  const equation = fit
    ? `y = ${sig(fit.slope)}x ${fit.intercept < 0 ? "-" : "+"} ${sig(Math.abs(fit.intercept))}`
    : "";
  const readout = fit ? {
    slope: `${sig(fit.slope)}${tinyeUnit}`,
    intercept: `${sig(fit.intercept)}${interceptUnit}`,
    r2: fit.r2 === null ? "undefined" : sig(fit.r2, 3),
    n: String(fit.n),
  } : null;
  const caption = fit
    ? `Ordinary least squares, n = ${fit.n}: ${equation}; R² = ${readout!.r2}. Descriptive only, no significance is claimed.`
    : "";
  return {
    points, fit, fitApplies, xTitle, yTitle, equation, readout, note, caption,
    canAdd: data.rows.length < maxRows,
    hasBars: data.errorBars && points.some(p => p.err > 0),
    summary: `${data.kind} chart of ${yTitle || "y"} against ${xTitle || "x"}, ${points.length} rows${fit ? `, least squares slope ${sig(fit.slope)}` : ""}.`,
  };
}

function finite(value: number, what: string): number {
  if (!Number.isFinite(value)) refuse(`${what} must be a finite number.`);
  return value;
}
function text(value: string, max: number, what: string): string {
  const trimmed = value.trim();
  if ([...trimmed].length > max) refuse(`${what} is at most ${max} characters.`);
  return trimmed;
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function setTitle(tx: Edit, title: string) {
  tx.fields.title.set(text(title, 60, "The title"));
}
export function setChart(tx: Edit, change: { kind?: Kind; fit?: boolean; errorBars?: boolean }) {
  if (change.kind !== undefined) tx.fields.kind.set(change.kind);
  if (change.fit !== undefined) tx.fields.fit.set(change.fit);
  if (change.errorBars !== undefined) tx.fields.errorBars.set(change.errorBars);
}
export function setAxis(tx: Edit, axis: "x" | "y", change: { label?: string; unit?: string }) {
  if (change.label !== undefined) tx.fields[`${axis}Label`].set(text(change.label, 32, "An axis label"));
  if (change.unit !== undefined) tx.fields[`${axis}Unit`].set(text(change.unit, 12, "A unit"));
}

/** Without an x, continues the spacing of the last two rows (or counts up by one). */
export function nextX(current: Notebook): number {
  const rows = current.rows;
  if (!rows.length) return 0;
  const last = rows[rows.length - 1].x;
  const step = rows.length > 1 ? last - rows[rows.length - 2].x : 1;
  return Math.round((last + (step || 1)) * 1e9) / 1e9;
}
export function addRow(tx: Edit, current: Notebook, row: { x?: number; y?: number; err?: number; label?: string } = {}) {
  if (current.rows.length >= maxRows) refuse(`A notebook holds at most ${maxRows} rows.`);
  const err = finite(row.err ?? 0, "The uncertainty");
  if (err < 0) refuse("The uncertainty is zero or more.");
  return tx.fields.rows.insert({
    label: text(row.label ?? "", 24, "A row label"),
    x: finite(row.x ?? nextX(current), "x"),
    y: finite(row.y ?? 0, "y"),
    err,
  });
}
export function updateRow(tx: Edit, row: RowRef, change: { x?: number; y?: number; err?: number; label?: string }) {
  const target = tx.fields.rows.item(row);
  if (change.x !== undefined) target.x.set(finite(change.x, "x"));
  if (change.y !== undefined) target.y.set(finite(change.y, "y"));
  if (change.err !== undefined) {
    if (finite(change.err, "The uncertainty") < 0) refuse("The uncertainty is zero or more.");
    target.err.set(change.err);
  }
  if (change.label !== undefined) target.label.set(text(change.label, 24, "A row label"));
}
export function removeRow(tx: Edit, row: RowRef) {
  tx.fields.rows.remove(row);
}
/** Parses what a person typed; null when it is not a finite number. */
export function parseNumber(input: string): number | null {
  const trimmed = input.trim().replace(/,/g, ".");
  if (!trimmed) return null;
  const value = Number(trimmed);
  return Number.isFinite(value) ? value : null;
}
