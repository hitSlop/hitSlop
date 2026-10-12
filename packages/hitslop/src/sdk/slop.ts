import type {
  AppMetadata,
  Capability,
  Category,
  WindowInput,
  WidgetOptions,
} from "../wire/app.generated";

/** `A`, refusing every key only `B` has: the two window forms never mix. */
type Only<A, B> = A & { [K in Exclude<keyof B, keyof A>]?: never };

/** App metadata; the CLI discovers schema and components by filename. */
type StandardWindowInput = Extract<WindowInput, { kind: "standard" }>;
type SkinWindowInput = Extract<WindowInput, { kind: "skin" }>;
export type AppDeclaration = Omit<AppMetadata, "categories"> & {
  categories: readonly [Category] | readonly [Category, Category];
  window: Only<StandardWindowInput, SkinWindowInput> | Only<SkinWindowInput, StandardWindowInput>;
  theme: Readonly<Record<string, `#${string}`>>;
  /** Optional approximate Mac refresh interval, 15–1440 minutes; requires Widget.svelte. */
  widget?: WidgetOptions;
  artwork?: { preview?: string; icon?: string };
  /** Devices and host services the app uses. The host admits a camera or microphone
   * request only when it is declared; the person's OS or browser permission still applies. */
  capabilities?: readonly Capability[];
};
export function defineSlop(slop: AppDeclaration): AppDeclaration {
  return slop;
}
