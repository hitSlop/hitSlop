import { refuse, type RowRef, type Tx } from "hitslop";
import { maxCaption, maxTiles } from "./schema";
import type doc from "./schema";

export type Board = typeof doc.current;
export type Tile = Board["tiles"][number];
type Edit = Tx<typeof doc>;

export const acceptedTypes = ["image/jpeg", "image/png", "image/webp"];
export const maxImageBytes = 10 * 1024 * 1024;

/** The editor and the export read the title, tiles and counts from the same snapshot. */
export function boardView(data: Board) {
  const count = data.tiles.length;
  return {
    title: data.title.trim() || "Moodboard",
    tiles: data.tiles,
    count,
    countLabel: `${count} ${count === 1 ? "image" : "images"}`,
    full: count >= maxTiles,
  };
}

/** Why a picked file can't join the board, checked before the host stores it; "" when it can. */
export function fileProblem(file: { name: string; type: string; size: number }) {
  if (!acceptedTypes.includes(file.type)) return "Choose JPEG, PNG or WebP images.";
  if (file.size > maxImageBytes) return `${file.name} is over 10 MB.`;
  return "";
}

/** A caption from a file name: no extension, at most the caption length. */
export function captionFromName(name: string) {
  return name.replace(/\.[^.]+$/, "").trim().slice(0, maxCaption);
}

function cleanCaption(caption: string) {
  const text = caption.trim();
  if (text.length > maxCaption) refuse(`A caption holds at most ${maxCaption} characters.`);
  return text;
}

// Each edit is written once: the page runs it with doc.change or an attachment import, a command with its tx.
export function addTile(tx: Edit, current: Board, image: { id: string; mimeType: string }, caption: string) {
  if (current.tiles.length >= maxTiles) refuse(`The board holds ${maxTiles} images.`);
  if (!acceptedTypes.includes(image.mimeType)) refuse("Choose JPEG, PNG or WebP images.");
  return tx.fields.tiles.insert({ image: { id: image.id, mimeType: image.mimeType }, caption: cleanCaption(caption) });
}
export function setCaption(tx: Edit, current: Board, tile: RowRef, caption: string) {
  const id = typeof tile === "string" ? tile : tile.$id;
  if (!current.tiles.some((row) => row.$id === id)) refuse("That image is no longer on the board.");
  tx.fields.tiles.item(tile).caption.set(cleanCaption(caption));
}
export function removeTile(tx: Edit, current: Board, tile: RowRef) {
  const id = typeof tile === "string" ? tile : tile.$id;
  if (!current.tiles.some((row) => row.$id === id)) refuse("That image is already gone.");
  tx.fields.tiles.remove(tile);
}
export function setTitle(tx: Edit, title: string) {
  tx.fields.title.set(title.trim());
}
