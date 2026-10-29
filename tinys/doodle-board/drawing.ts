import { getStroke } from "perfect-freehand";
import { refuse, type Tx, type Value } from "@hitslop/tinys";
import type schema from "./schema";

export type Board = Value<typeof schema.descriptor>;
type Edit = Tx<typeof schema>;
export type Stroke = { geometry: string; color: string };

export const boards = {
  landscape: { label: "Landscape", width: 1000, height: 700 },
  square: { label: "Square", width: 1000, height: 1000 },
  portrait: { label: "Portrait", width: 700, height: 1000 },
} as const;
export const markers = ["charcoal", "coral", "blue", "green", "purple"] as const;
export const widths = [{ label: "Fine", size: 4 }, { label: "Medium", size: 9 }, { label: "Bold", size: 18 }] as const;
export type Point = [number, number, number];
export type Brush = { size: number; smoothing: number; thinning: number; taper: "none" | "short" | "long" };
export const defaultBrush: Readonly<Brush> = { size: 9, smoothing: 0.6, thinning: 0.45, taper: "none" };

export function fitWindowSize(shape: keyof typeof boards, chrome: { width: number; height: number }, available: { width: number; height: number }) {
  const board = boards[shape];
  const scale = Math.max(0.01, Math.min(720 / Math.max(board.width, board.height),
    (available.width - chrome.width) / board.width, (available.height - chrome.height) / board.height));
  return { width: Math.max(240, Math.round(board.width * scale + chrome.width)), height: Math.max(180, Math.round(board.height * scale + chrome.height)) };
}

/** Bound active-gesture memory while keeping the first and latest sample. */
export class StrokeSamples {
  points: Point[] = [];
  private spacing = 0.6;
  add(point: Point, force = false) {
    if (!point.every(Number.isFinite)) return;
    const last = this.points.at(-1);
    if (!force && last && Math.hypot(point[0] - last[0], point[1] - last[1]) < this.spacing) return;
    this.points.push([point[0], point[1], Math.max(0, Math.min(1, point[2]))]);
    if (this.points.length > 2048) {
      this.points = this.points.filter((_, i, points) => i % 2 === 0 || i === points.length - 1);
      this.spacing *= 1.5;
    }
  }
}

export function strokePath(points: Point[], brush: Brush | number, pen: boolean) {
  const settings = typeof brush === "number" ? { ...defaultBrush, size: brush } : brush;
  const length = points.reduce((sum, point, i) => i ? sum + Math.hypot(point[0] - points[i - 1]![0], point[1] - points[i - 1]![1]) : 0, 0);
  // A tap is still a dot even when taper is enabled.
  const taper = length * ({ none: 0, short: 0.15, long: 0.3 }[settings.taper]);
  const outline = getStroke(points, {
    size: settings.size, thinning: settings.thinning, smoothing: settings.smoothing, streamline: 0.35,
    start: { taper, cap: true }, end: { taper, cap: true },
    simulatePressure: !pen, last: true,
  });
  if (!outline.length) return "";
  const first = outline[0]!;
  let path = `M${first[0].toFixed(2)},${first[1].toFixed(2)}`;
  // Closed midpoint quadratics smooth the outline without introducing SVG markup.
  for (let i = 0; i < outline.length; i++) {
    const a = outline[i]!;
    const b = outline[(i + 1) % outline.length]!;
    path += `Q${a[0].toFixed(2)},${a[1].toFixed(2)} ${((a[0] + b[0]) / 2).toFixed(2)},${((a[1] + b[1]) / 2).toFixed(2)}`;
  }
  return path + "Z";
}

/** Sample an eraser sweep so quick pointer movements don't skip thin lines. */
export function sweep(from: Point, to: Point, spacing = 3): Point[] {
  const count = Math.min(2000, Math.max(1, Math.ceil(Math.hypot(to[0] - from[0], to[1] - from[1]) / spacing)));
  return Array.from({ length: count + 1 }, (_, i) => [
    from[0] + (to[0] - from[0]) * i / count,
    from[1] + (to[1] - from[1]) * i / count,
    0.5,
  ]);
}

/** The editor and the export draw the same board and ink from the same snapshot. */
export function boardView(data: Board) {
  const board = boards[data.boardShape];
  return { shape: data.boardShape, board, strokes: data.strokes, blank: data.strokes.length === 0 };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
const hexColor = /^#[0-9a-f]{6}([0-9a-f]{2})?$/i;
export function addStroke(tx: Edit, stroke: Stroke) {
  if (!stroke.geometry.startsWith("M")) refuse("A stroke needs an outline.");
  if (!stroke.color.trim()) refuse("A stroke needs a color.");
  return tx.fields.strokes.insert({ geometry: stroke.geometry, color: stroke.color.trim() });
}

/** Builds a finished stroke from "x,y x,y ..." board coordinates with the default brush. */
export function drawStroke(tx: Edit, points: string, color: string, size: number = defaultBrush.size) {
  const parsed = points.trim().split(/\s+/).filter(Boolean).map(pair => pair.split(",").map(Number));
  if (!parsed.length || parsed.some(pair => pair.length !== 2 || !pair.every(Number.isFinite))) refuse("Points are x,y pairs separated by spaces, such as \"120,80 300,220\".");
  if (!hexColor.test(color.trim())) refuse("Color is a hex value such as #343744.");
  if (!(size >= 1 && size <= 64)) refuse("Size is between 1 and 64.");
  const geometry = strokePath(parsed.map(([x, y]) => [x!, y!, 0.5]), size, false);
  if (!geometry) refuse("Those points do not make a stroke.");
  return addStroke(tx, { geometry, color: color.trim().toLowerCase() });
}

/** Removes the strokes that are still on the board; returns how many went. */
export function eraseStrokes(tx: Edit, current: Board, ids: Iterable<string>) {
  const present = new Set(current.strokes.map(stroke => stroke.$id));
  let removed = 0;
  for (const id of ids) if (present.delete(id)) { tx.fields.strokes.remove(id); removed++; }
  return removed;
}

/** Removes every stroke, including ink outside the current board; returns the count. */
export function clearBoard(tx: Edit, current: Board) {
  for (const stroke of current.strokes) tx.fields.strokes.remove(stroke.$id);
  return current.strokes.length;
}

export function setBoardShape(tx: Edit, shape: Board["boardShape"]) {
  if (!(shape in boards)) refuse("Choose landscape, square, or portrait.");
  tx.fields.boardShape.set(shape);
}
