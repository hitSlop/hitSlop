import type * as Wire from "../wire/core.generated";
import type { Code } from "../wire/engine.generated";

export type Batch = Omit<Wire.Batch, "intents"> & { intents: OwnerIntent[] };
export type CoreErrorCode = Code;
export type Segment = Wire.Segment;
/** Where a row goes: before or after the row with this `$id`. */
export type Anchor = Wire.Anchor;
export type OwnerState = Wire.OwnerState;
export type OwnerPublication = Wire.Publication;
export type OwnerIntent = Exclude<Wire.Intent, PaletteIntent>;
export type PaletteIntent = Extract<Wire.Intent, { type: "setTheme" | "importTheme" }>;
export type OwnerPath = Segment[];
export type OwnerPatchOp = Wire.PatchOp;
export type TextHunk = Wire.Hunk;
