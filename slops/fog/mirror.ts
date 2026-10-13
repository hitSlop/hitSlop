import { dateLabel as formattedDate } from "../_shared/format";
import { refuse, type Tx } from "hitslop";
import type doc from "./schema";
import { BRUSH } from "./schema";

export type Mirror = typeof doc.current;
type Edit = Tx<typeof doc>;
/** An attachment reference as `attachments.import` returns it. */
export type FileRef = { id: string; name: string; mimeType: string; byteLength: number };

/** One attachment holds at most 10 MiB. */
export const MAX_SNAPSHOT_BYTES = 10 * 1024 * 1024;

/** The editor's start screen and the export read the same view of the snapshot. */
export function mirrorView(data: Mirror) {
  const snapshot = data.snapshot ?? null;
  return { brush: data.brush, snapshot, caption: snapshot ? `saved ${stamp(snapshot.at)}` : "" };
}

export function stamp(iso: string): string {
  const date = new Date(iso);
  if (!Number.isFinite(date.getTime())) return "";
  return formattedDate(date, { day: "numeric", month: "short", year: "numeric", hour: "numeric", minute: "2-digit" });
}

/** A descriptive name such as `fog-2026-10-10-1920.png`, in local time. */
export function snapshotName(now: Date, ext: string): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `fog-${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}-${pad(now.getHours())}${pad(now.getMinutes())}.${ext}`;
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function setBrush(tx: Edit, radius: number) {
  if (!Number.isInteger(radius) || radius < BRUSH.min || radius > BRUSH.max) refuse(`The brush is ${BRUSH.min} to ${BRUSH.max} pixels.`);
  tx.fields.brush.set(radius);
}

/** Keeps a new snapshot; the previous one is dropped from the file when it closes. */
export function keepSnapshot(tx: Edit, ref: FileRef, width: number, height: number, at: string) {
  if (!Number.isFinite(Date.parse(at))) refuse("That time is not a valid date.");
  tx.fields.snapshot.set({
    id: ref.id, name: ref.name, mimeType: ref.mimeType, byteLength: ref.byteLength,
    width: Math.max(1, Math.round(width)), height: Math.max(1, Math.round(height)), at,
  });
}

export function clearSnapshot(tx: Edit, data: Mirror) {
  if (!data.snapshot) refuse("There is no saved snapshot to clear.");
  tx.fields.snapshot.clear();
}
