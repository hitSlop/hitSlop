import type { Input, ListNode, Node, ObjectNode, OptionalNode, RowNode, Scalar, Synchronous, Value } from "./schema";
import type { Scope } from "./abi";
import { current, documentFor } from "./app/context";

export interface CommandContext<N extends ObjectNode> {
  readonly current: Value<N>;
  readonly tx: Scope<N>;
  readonly now: number;
  random(): number;
}
/** The portable object-descriptor subset the owner checks before evaluation, plus rows. */
export interface Arguments { [name: string]: ArgumentNode | RowNode }
export interface NestedArguments { [name: string]: ArgumentNode }
export type ArgumentNode = Scalar | ObjectNode<NestedArguments> | OptionalNode<Scalar | ObjectNode<NestedArguments>> | ListNode<Scalar>;
type RowKeys<A extends Arguments> = { [K in keyof A]: A[K] extends RowNode ? K : never }[keyof A];
type Plain<A extends Arguments> = { [K in Exclude<keyof A, RowKeys<A>>]: Extract<A[K], Node> };
/** What a caller passes: a row argument takes the row itself or its `$id`. */
export type CommandInput<A extends Arguments> = Input<ObjectNode<Plain<A>>> & {
  [K in RowKeys<A>]: string | { readonly $id: string };
};
/** What `run` receives: defaults filled, and each row argument resolved from `current`. */
export type CommandArgs<N extends ObjectNode, A extends Arguments> = Received<ObjectNode<Plain<A>>> & {
  readonly [K in RowKeys<A>]: A[K] extends RowNode<infer L> ? RowOf<N, L> : never;
};
type RowOf<N extends ObjectNode, L> = L extends keyof N["properties"]
  ? Value<N["properties"][L]> extends ReadonlyArray<infer R> ? R : never
  : never;
type Received<N extends Node> = N extends ObjectNode<infer P>
  ? { readonly [K in keyof P as P[K] extends OptionalNode ? never : K]: Received<P[K]> } & {
      readonly [K in keyof P as P[K] extends OptionalNode ? K : never]?: Received<P[K]>;
    }
  : N extends OptionalNode<infer S>
    ? Received<S>
    : N extends ListNode<infer I>
      ? ReadonlyArray<Received<I>>
      : Value<N>;
export type Command<A extends Arguments, R> = ({} extends CommandInput<A> ? (args?: CommandInput<A>) => Promise<R> : (args: CommandInput<A>) => Promise<R>);
export type CommandSpec<N extends ObjectNode, A extends Arguments, R> = {
  description: string;
  args: A;
  run(ctx: CommandContext<N>, args: CommandArgs<N, A>): Synchronous<R>;
};
const brand = Symbol.for("slop.command");
export type CommandInfo = { definition: object; spec: CommandSpec<any, Arguments, unknown>; name?: string };
export function commandInfo(value: unknown): CommandInfo | undefined {
  return typeof value === "function" ? (value as { [brand]?: CommandInfo })[brand] : undefined;
}
export function bindCommands(commands: Record<string, unknown>, definition: object) {
  const seen = new Set<CommandInfo>();
  for (const [name, value] of Object.entries(commands)) {
    const info = commandInfo(value);
    if (!info || info.definition !== definition) throw new Error(`commands.${name} must be a command from this document`);
    if (seen.has(info) || (info.name && info.name !== name)) throw new Error(`Command ${name} has multiple export names`);
    seen.add(info);
    info.name = name;
  }
}
/** Each row argument as its `$id`, as the owner checks it. */
function rowIds(args: Record<string, unknown>, spec: Arguments) {
  const sent: Record<string, unknown> = { ...args };
  for (const [name, node] of Object.entries(spec)) {
    const value = sent[name];
    if (node.kind === "row" && value && typeof value === "object" && "$id" in value) sent[name] = value.$id;
  }
  return sent;
}
/** A page call sends the registered name and arguments to the owner, which runs the
 * stored program; the UI build removes `run` from declarations it recognizes. */
export function makeCommand<N extends ObjectNode, A extends Arguments, R>(definition: object, spec: CommandSpec<N, A, R>): Command<A, R> {
  const info: CommandInfo = { definition, spec: spec as CommandInfo["spec"] };
  const command = (args: CommandInput<A> = {} as CommandInput<A>): Promise<R> => {
    try {
      documentFor(definition);
      if (!info.name) throw new Error("Register the command in defineSlop({ commands })");
      return current().document.runCommand(info.name, JSON.parse(JSON.stringify(rowIds(args, spec.args))));
    } catch (error) { return Promise.reject(error); }
  };
  Object.defineProperty(command, brand, { value: info });
  return command as Command<A, R>;
}
