import type { SlopCategory, SlopManifest, SlopSkinPresentation, SlopStandardPresentation } from "../schema/index";
import type { Definition, Input, ObjectNode } from "./schema";

/** `A`, refusing every key only `B` has: the two window forms never mix. */
type Only<A, B> = A & { [K in Exclude<keyof B, keyof A>]?: never };

/**
 * A slop's app as `slop.ts` declares it: everything a `.slop` file stores about the app
 * except its code and assets. The manifest's fields come from the internal TypeBox contracts; the
 * project folder's name is the slug. `slop build` evaluates `slop.ts` in Bun and never
 * ships it in `app.js`. Types only assist: the build checks every value by the rules the
 * app opens files with.
 */
export type Slop<S extends Definition<ObjectNode>> = Omit<SlopManifest, "slug" | "categories" | "presentation"> & {
  /** One or two catalog categories. */
  categories: readonly [SlopCategory] | readonly [SlopCategory, SlopCategory];
  /** The initial window: drawn by the host (with an optional `shape`), or by a PNG `skin`. */
  presentation: Only<SlopStandardPresentation, SlopSkinPresentation> | Only<SlopSkinPresentation, SlopStandardPresentation>;
  /** The colors a person may change, as lowercase `#rrggbb` or `#rrggbbaa` (opaque colors
   * omit `ff`). Each becomes `--slop-<name>`; fonts, sizes and colors derived from these
   * belong in the app's CSS. */
  theme: Readonly<Record<string, `#${string}`>>;
  /** `schema.ts`'s default export: the live document the app imports. */
  schema: S;
  /** A new document's values. */
  initial: Input<S["descriptor"]>;
};

/** Declares a slop's app in `slop.ts`: `export default defineSlop({ … })`. */
export function defineSlop<S extends Definition<ObjectNode>>(slop: Slop<S>): Slop<S> {
  return slop;
}
