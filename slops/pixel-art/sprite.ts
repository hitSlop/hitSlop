import { refuse, type Tx, type Value } from "hitslop";
import type schema from "./schema";
export type Sprite = Value<typeof schema.descriptor>;
type Edit = Tx<typeof schema>;

export const SIZE = 16;
export const CELL_COUNT = SIZE * SIZE;
export const PALETTE_IDS = ["gameboy", "pico8", "cyberpunk", "mono"] as const;
export type PaletteId = (typeof PALETTE_IDS)[number];

export const PALETTES: Record<PaletteId, { label: string; colors: string[] }> = {
  gameboy: { label: "GB", colors: ["#0f380f", "#306230", "#8bac0f", "#9bbc0f"] },
  pico8: {
    label: "PICO",
    colors: [
      "#000000", "#1d2b53", "#7e2553", "#008751", "#ab5236", "#5f574f", "#c2c3c7", "#fff1e8",
      "#ff004d", "#ffa300", "#ffec27", "#00e436", "#29adff", "#83769c", "#ff77a8", "#ffccaa",
    ],
  },
  cyberpunk: { label: "NEON", colors: ["#0b0b12", "#2de2e6", "#ff2bd6", "#f9f002", "#7a3cff", "#ff6b35", "#f8fafc"] },
  mono: { label: "MONO", colors: ["#111111", "#555555", "#999999", "#eeeeee"] },
};

export function isPaletteId(value: string): value is PaletteId {
  return (PALETTE_IDS as readonly string[]).includes(value);
}

/** The editor and its captures read the same palette and cells from the snapshot. */
export function spriteView(data: Sprite) {
  const paletteId: PaletteId = isPaletteId(data.paletteId) ? data.paletteId : "gameboy";
  return { paletteId, palette: PALETTES[paletteId], cells: data.pixels };
}

function checkIndex(index: number) {
  if (!Number.isInteger(index) || index < 0 || index >= CELL_COUNT) refuse(`Cell ${index} is outside the 16 by 16 canvas.`);
}
function checkColor(color: string) {
  if (color !== "" && !/^#[0-9a-fA-F]{6}$/.test(color)) refuse("Use a #rrggbb color, or empty to erase.");
}

/** The cells a flood fill from `start` would recolor: the 4-connected region of the same color. */
export function fillRegion(pixels: readonly string[], start: number, color: string): number[] {
  const target = pixels[start] ?? "";
  if (target === color) return [];
  const queue = [start];
  const seen = new Set<number>([start]);
  const cells: number[] = [];
  while (queue.length > 0) {
    const index = queue.pop()!;
    cells.push(index);
    const x = index % SIZE;
    const y = Math.floor(index / SIZE);
    const neighbors = [
      x > 0 ? index - 1 : -1,
      x < SIZE - 1 ? index + 1 : -1,
      y > 0 ? index - SIZE : -1,
      y < SIZE - 1 ? index + SIZE : -1,
    ];
    for (const next of neighbors) {
      if (next < 0 || seen.has(next) || (pixels[next] ?? "") !== target) continue;
      seen.add(next);
      queue.push(next);
    }
  }
  return cells;
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function paintCells(tx: Edit, current: Sprite, cells: readonly (readonly [number, string])[]) {
  for (const [index, color] of cells) { checkIndex(index); checkColor(color); }
  let changed = 0;
  for (const [index, color] of cells) {
    if ((current.pixels[index] ?? "") === color) continue;
    tx.fields.pixels.set(index, color);
    changed += 1;
  }
  return changed;
}
export function fillCells(tx: Edit, current: Sprite, start: number, color: string) {
  checkIndex(start);
  checkColor(color);
  const cells = fillRegion(current.pixels, start, color);
  for (const index of cells) tx.fields.pixels.set(index, color);
  return cells.length;
}
export function selectColor(tx: Edit, color: string) {
  if (!/^#[0-9a-fA-F]{6}$/.test(color)) refuse("Use a #rrggbb color.");
  tx.fields.selectedColor.set(color);
}
export function setPalette(tx: Edit, current: Sprite, id: string) {
  if (!isPaletteId(id)) refuse(`Unknown palette "${id}". Use ${PALETTE_IDS.join(", ")}.`);
  const colors = PALETTES[id].colors;
  const selected = colors.includes(current.selectedColor) ? current.selectedColor : (colors[0] ?? "#111111");
  if (current.paletteId !== id) tx.fields.paletteId.set(id);
  if (current.selectedColor !== selected) tx.fields.selectedColor.set(selected);
}
