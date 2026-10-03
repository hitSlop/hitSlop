import type { Shape } from "./schema";

/** A shape as the poster draws it: the saved fields plus an id, possibly mid-drag. */
export type PosterShape = Pick<Shape, "kind" | "tone" | "x" | "y" | "size"> & { id: string };
