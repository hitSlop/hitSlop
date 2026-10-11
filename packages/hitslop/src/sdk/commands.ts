import type { Definition, ObjectNode, OptionalNode, Scalar, Value } from "./schema";
import type { Scope } from "./abi";
import { commandBrand, type CommandSet } from "./command-set";
export type { CommandSet } from "./command-set";
import { withCommand } from "./app/context";

export interface CommandContext<N extends ObjectNode> {
  readonly current: Value<N>;
  readonly tx: Scope<N>;
  readonly now: number;
  random(): number;
}
export type ArgumentNode = Scalar | OptionalNode<Scalar>;
export type Arguments = Record<string, ArgumentNode>;
/** Arguments after Rust has checked them and filled defaults. */
export type CommandArgs<A extends Arguments> = {
  readonly [K in keyof A as A[K] extends OptionalNode ? never : K]: Value<A[K]>;
} & {
  readonly [K in keyof A as A[K] extends OptionalNode ? K : never]?: Value<A[K]>;
};
export type CommandResult =
  | null
  | boolean
  | number
  | string
  | readonly CommandResult[]
  | { readonly [key: string]: CommandResult };
export type CommandSpec<N extends ObjectNode, A extends Arguments> = {
  description: string;
  args: A;
  run(ctx: CommandContext<N>, args: NoInfer<CommandArgs<A>>): CommandResult | void;
};
/** Agent-only commands. Pages share plain functions over Scope, never command sets. */
export function defineCommands<N extends ObjectNode, A extends Record<string, Arguments>>(
  document: Definition<N>,
  specs: { [K in keyof A]: CommandSpec<N, A[K]> },
): CommandSet {
  const checked = Object.fromEntries(
    Object.entries(specs).map(([name, spec]) => {
      const run = spec?.run;
      if (typeof run !== "function") throw new Error(`commands.${name} needs run(ctx, args)`);
      if (Object.prototype.toString.call(run) !== "[object Function]")
        throw new Error(`commands.${name} must be synchronous (no async or generator functions)`);
      return [
        name,
        Object.freeze({
          ...spec,
          run: (ctx: CommandContext<N>, args: CommandArgs<Arguments>) =>
            withCommand(() => run(ctx, args)),
        }),
      ];
    }),
  );
  return Object.freeze({
    [commandBrand]: Object.freeze({ document, specs: Object.freeze(checked) }),
  });
}
