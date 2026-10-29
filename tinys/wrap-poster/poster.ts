import { refuse, type RowRef, type Tx, type Value } from "@hitslop/tinys";
import { maxShapes, maxSize, minSize, tones, type shapeKinds } from "./schema";
import type schema from "./schema";

export type Data = Value<typeof schema.descriptor>;
type Edit = Tx<typeof schema>;
type Shape = Data["shapes"][number];
export type Kind = (typeof shapeKinds)[number];
export type Tone = (typeof tones)[number];

/** A shape as the poster draws it: the saved fields plus an id, possibly mid-drag. */
export type PosterShape = Pick<Shape, "kind" | "tone" | "x" | "y" | "size"> & { id: string };
/** A live position for one shape while it is dragged; it is saved on release. */
export type Drag = { id: string; x: number; y: number } | null;

export const clamp = (value: number, low = 0, high = 1) => Math.min(high, Math.max(low, value));

/** The editor and the export draw the same shapes from the same snapshot (the editor adds its live drag). */
export function posterView(data: Data, drag: Drag = null) {
  const shapes: PosterShape[] = data.shapes.map((shape) => {
    const live = drag?.id === shape.$id ? drag : null;
    return { id: shape.$id, kind: shape.kind, tone: shape.tone, x: live?.x ?? shape.x, y: live?.y ?? shape.y, size: shape.size };
  });
  return { shapes, full: data.shapes.length >= maxShapes };
}

/** One step of the plus/minus buttons, kept inside the allowed sizes. */
export function nudgedSize(size: number, direction: 1 | -1) {
  return clamp(Math.round((size + direction * 0.04) * 100) / 100, minSize, maxSize);
}

function checkFraction(value: number, name: string) {
  if (!Number.isFinite(value) || value < 0 || value > 1) refuse(`${name} must be between 0 and 1.`);
}
function checkSize(size: number) {
  if (!Number.isFinite(size) || size < minSize || size > maxSize) refuse(`Size must be between ${minSize} and ${maxSize}.`);
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addShape(
  tx: Edit,
  current: Data,
  kind: Kind,
  options: { tone?: Tone; x?: number; y?: number; size?: number } = {},
) {
  if (current.shapes.length >= maxShapes) refuse(`A poster holds at most ${maxShapes} shapes. Remove one first.`);
  const { tone = tones[current.shapes.length % tones.length]!, x = 0.5, y = 0.55, size = kind === "circle" ? 0.28 : 0.22 } = options;
  checkFraction(x, "x");
  checkFraction(y, "y");
  checkSize(size);
  return tx.fields.shapes.insert({ kind, tone, x, y, size }).id;
}
export function moveShape(tx: Edit, shape: RowRef, x: number, y: number) {
  checkFraction(x, "x");
  checkFraction(y, "y");
  const row = tx.fields.shapes.item(shape);
  row.x.set(x);
  row.y.set(y);
}
export function resizeShape(tx: Edit, shape: RowRef, size: number) {
  checkSize(size);
  tx.fields.shapes.item(shape).size.set(size);
}
export function setShapeTone(tx: Edit, shape: RowRef, tone: Tone) {
  tx.fields.shapes.item(shape).tone.set(tone);
}
export function removeShape(tx: Edit, shape: RowRef) {
  tx.fields.shapes.remove(shape);
}
export function setText(tx: Edit, text: { title?: string; body?: string }) {
  if (text.title === undefined && text.body === undefined) refuse("Give a title, a body, or both.");
  if (text.title !== undefined) tx.fields.title.set(text.title);
  if (text.body !== undefined) tx.fields.body.set(text.body);
}
