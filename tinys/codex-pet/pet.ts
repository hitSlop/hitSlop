import { refuse, type Tx, type Value } from "@hitslop/tinys";
import type schema from "./schema";
import type { PetMetadata } from "./pet-package";

export type PetDocument = Value<typeof schema.descriptor>;
export type StoredPackage = NonNullable<PetDocument["package"]>;
type Edit = Tx<typeof schema>;
/** An attachment reference as `attachments.import` hands it to the collector. */
type PackageRef = { id: string; name: string; mimeType: string; byteLength: number };

export type PetAction = "idle" | "running-right" | "running-left" | "waving" | "jumping" | "failed" | "waiting" | "running" | "review";
type ActionSpec = { row: number; durations: number[] };

export const CELL_WIDTH = 192, CELL_HEIGHT = 208;
const NAME_LIMIT = 80;

/** The bundled starter pet; an imported ZIP replaces it until Restore Bubu. */
export const STARTER: PetMetadata = {
  id: "bubu--gbn666",
  displayName: "Bubu",
  description: "A soft, sleepy milk-tea-brown chibi bear desktop pet.",
  spriteVersionNumber: 1,
  spritesheetPath: "bubu-spritesheet.webp",
};

/** Codex pet spritesheet rows: one action per row, one frame per 192 x 208 cell. */
export const ACTIONS: Record<PetAction, ActionSpec> = {
  idle: { row: 0, durations: [280, 110, 110, 140, 140, 320] },
  "running-right": { row: 1, durations: [120, 120, 120, 120, 120, 120, 120, 220] },
  "running-left": { row: 2, durations: [120, 120, 120, 120, 120, 120, 120, 220] },
  waving: { row: 3, durations: [140, 140, 140, 280] },
  jumping: { row: 4, durations: [140, 140, 140, 140, 280] },
  failed: { row: 5, durations: [140, 140, 140, 140, 140, 140, 140, 240] },
  waiting: { row: 6, durations: [150, 150, 150, 150, 150, 260] },
  running: { row: 7, durations: [120, 120, 120, 120, 120, 220] },
  review: { row: 8, durations: [150, 150, 150, 150, 150, 280] },
};

function ambientPool(hour: number): PetAction[] {
  if (hour < 6 || hour >= 22) return ["waiting", "idle", "idle"];
  if (hour < 10) return ["waving", "jumping", "waving", "waiting"];
  if (hour < 17) return ["running", "review", "running", "waiting"];
  return ["waving", "review", "jumping", "waiting"];
}

/** The next unprompted animation for the local hour: rarer overnight. View-only, never saved. */
export function nextAmbient(hour: number, random: () => number): { delay: number; action: PetAction } {
  const overnight = hour < 6 || hour >= 22;
  const delay = overnight ? 30_000 + random() * 60_000 : 12_000 + random() * 18_000;
  const pool = ambientPool(hour);
  return { delay, action: pool[Math.floor(random() * pool.length)] ?? "idle" };
}

/** What the editor, export and icon read from the snapshot. */
export function petView(data: PetDocument) {
  const stored = data.package;
  return {
    imported: Boolean(stored),
    displayName: data.displayName.trim() || STARTER.displayName,
    stored,
    /** Changes only when a different package is stored, so the sheet reloads only then. */
    key: stored ? `${stored.id}\0${stored.mimeType}` : "",
  };
}

// Each edit is written once: the page runs it with doc.change or an attachment import,
// a command with its tx.
export function adoptPackage(tx: Edit, ref: PackageRef, displayName: string) {
  const name = displayName.trim();
  if (!name || name.length > NAME_LIMIT) refuse("The pet needs a name of 1 to 80 characters.");
  tx.fields.package.$set({ id: ref.id, name: ref.name, mimeType: ref.mimeType, byteLength: ref.byteLength });
  tx.fields.displayName.set(name);
}

export function restoreStarter(tx: Edit, data: PetDocument) {
  if (!data.package) refuse("Bubu is already the pet.");
  tx.fields.package.$clear();
  tx.fields.displayName.set(STARTER.displayName);
}
