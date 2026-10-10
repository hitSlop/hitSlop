import type { AppMetadata, Category, WindowInput } from "../wire/app.generated";

/** `A`, refusing every key only `B` has: the two window forms never mix. */
type Only<A, B> = A & { [K in Exclude<keyof B, keyof A>]?: never };

/** App metadata; the CLI discovers schema and components by filename. */
type StandardWindowInput = Extract<WindowInput, { kind: "standard" }>;
type SkinWindowInput = Extract<WindowInput, { kind: "skin" }>;
export type AppDeclaration = Omit<AppMetadata, "categories"> & {
  categories: readonly [Category] | readonly [Category, Category];
  window: Only<StandardWindowInput, SkinWindowInput> | Only<SkinWindowInput, StandardWindowInput>;
  theme: Readonly<Record<string, `#${string}`>>;
  artwork?: { preview?: string; icon?: string };
};
export function defineSlop(slop: AppDeclaration): AppDeclaration {
  return slop;
}
