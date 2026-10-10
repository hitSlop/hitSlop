import type { CommandSpec, Arguments } from "./commands";

export const commandBrand = Symbol.for("slop.command-set");
type Specs = Readonly<Record<string, CommandSpec<any, Arguments>>>;
export type CommandSet = { readonly [commandBrand]: { readonly document: object; readonly specs: Specs } };
/** Internal projection used by definition evaluation and the ABI runner. */
export function commandSet(value: unknown): CommandSet[typeof commandBrand] | undefined {
  return value && typeof value === "object" ? (value as CommandSet)[commandBrand] : undefined;
}
