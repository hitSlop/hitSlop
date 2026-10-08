import type { Definition, Input, ObjectNode } from "./schema";
import type { Component } from "svelte";
import type { CaptureMode } from "../wire/page.generated";
import type { AppMetadata, Category, WindowInput } from "../wire/app.generated";

/** `A`, refusing every key only `B` has: the two window forms never mix. */
type Only<A, B> = A & { [K in Exclude<keyof B, keyof A>]?: never };

/** Explicit app entry. Vite resolves every imported role; Rust accepts the result. */
type StandardWindowInput = Extract<WindowInput, { kind: "standard" }>;
type SkinWindowInput = Extract<WindowInput, { kind: "skin" }>;
export type AppDeclaration<S extends Definition<ObjectNode>> = Omit<AppMetadata, "categories"> & {
  categories: readonly [Category] | readonly [Category, Category];
  document: S;
  initial: Input<S["descriptor"]>;
  window: Only<StandardWindowInput, SkinWindowInput> | Only<SkinWindowInput, StandardWindowInput>;
  theme: Readonly<Record<string, `#${string}`>>;
  view: Component;
  /** A dedicated composition derived from the saved document, independent of editor state. */
  /** A capture layout, given `mode` (`preview` or `export`) when it declares it. */
  export?: Component<{ mode: Exclude<CaptureMode, "icon"> }> | Component<Record<string, never>>;
  icon?: Component;
  commands?: Record<string, unknown>;
  artwork?: { preview?: string; icon?: string };
};
export function defineSlop<S extends Definition<ObjectNode>>(slop: AppDeclaration<S>): AppDeclaration<S> {
  return slop;
}
