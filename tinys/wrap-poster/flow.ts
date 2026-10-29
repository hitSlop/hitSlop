import { layoutNextLine, type LayoutCursor, type PreparedTextWithSegments } from "@chenglou/pretext";

export type Obstacle =
  | { kind: "circle"; cx: number; cy: number; r: number }
  | { kind: "square"; x: number; y: number; size: number };

export interface FlowLine { text: string; x: number; y: number }
export interface Box { top: number; bottom: number; left: number; right: number; lineHeight: number; gap: number; minSpan: number }
export interface Flowed { lines: FlowLine[]; bottom: number; fits: boolean }

/** The horizontal stretch an obstacle takes out of the row between y0 and y1, if it touches it. */
function blocked(obstacle: Obstacle, y0: number, y1: number, gap: number): [number, number] | null {
  if (obstacle.kind === "square") {
    if (y1 <= obstacle.y - gap || y0 >= obstacle.y + obstacle.size + gap) return null;
    return [obstacle.x - gap, obstacle.x + obstacle.size + gap];
  }
  const radius = obstacle.r + gap;
  const distance = obstacle.cy < y0 ? y0 - obstacle.cy : obstacle.cy > y1 ? obstacle.cy - y1 : 0;
  if (distance >= radius) return null;
  const half = Math.sqrt(radius * radius - distance * distance);
  return [obstacle.cx - half, obstacle.cx + half];
}

/** What is left of a row once the obstacles are cut out, widest enough to be worth a line. */
function freeSpans(box: Box, y: number, obstacles: Obstacle[]): [number, number][] {
  const cuts = obstacles
    .map((obstacle) => blocked(obstacle, y, y + box.lineHeight, box.gap))
    .filter((cut): cut is [number, number] => cut !== null)
    .sort((a, b) => a[0] - b[0]);
  const spans: [number, number][] = [];
  let x = box.left;
  for (const [from, to] of cuts) {
    if (from - x >= box.minSpan) spans.push([x, Math.min(from, box.right)]);
    x = Math.max(x, to);
  }
  if (box.right - x >= box.minSpan) spans.push([x, box.right]);
  return spans;
}

/** Pour prepared text into a box, row by row, one line per free span, left to right. */
export function flow(prepared: PreparedTextWithSegments, box: Box, obstacles: Obstacle[]): Flowed {
  const lines: FlowLine[] = [];
  let cursor: LayoutCursor = { segmentIndex: 0, graphemeIndex: 0 };
  let y = box.top;
  let finished = false;
  while (!finished && y + box.lineHeight <= box.bottom) {
    for (const [from, to] of freeSpans(box, y, obstacles)) {
      const line = layoutNextLine(prepared, cursor, to - from);
      if (!line) { finished = true; break; }
      lines.push({ text: line.text, x: from, y });
      cursor = line.end;
    }
    y += box.lineHeight;
  }
  const fits = finished || layoutNextLine(prepared, cursor, box.right - box.left) === null;
  const last = lines.at(-1);
  return { lines, bottom: last ? last.y + box.lineHeight : box.top, fits };
}
