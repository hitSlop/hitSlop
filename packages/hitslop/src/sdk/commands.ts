import type { Input, ListNode, ObjectNode, OptionalNode, Scalar, Value } from "./schema";
import type { Scope } from "./abi";
import { current, documentFor } from "./app/context";

export interface CommandContext<N extends ObjectNode> {
  readonly current: Value<N>;
  readonly tx: Scope<N>;
  readonly now: number;
  random(): number;
}
/** The portable object-descriptor subset the owner checks before evaluation. */
export interface Arguments { [name: string]: ArgumentNode }
export type ArgumentNode = Scalar | ObjectNode<Arguments> | OptionalNode<Scalar | ObjectNode<Arguments>> | ListNode<Scalar>;
export type CommandInput<A extends Arguments> = Input<ObjectNode<A>>;
export type Command<A extends Arguments, R> = ({} extends CommandInput<A> ? (args?: CommandInput<A>) => Promise<R> : (args: CommandInput<A>) => Promise<R>);
export type CommandSpec<N extends ObjectNode, A extends Arguments, R> = {
  description: string;
  args: A;
  run(ctx: CommandContext<N>, args: CommandInput<A>): R;
};
const brand = Symbol.for("hitslop.command");
export type CommandInfo = { definition: object; spec: CommandSpec<any, Arguments, unknown>; name?: string };
export function commandInfo(value: unknown): CommandInfo | undefined {
  return typeof value === "function" ? (value as any)[brand] : undefined;
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
/** A page call sends the registered name and arguments to the owner, which runs the
 * stored program; the UI build removes `run` from declarations it recognizes. */
export function makeCommand<N extends ObjectNode, A extends Arguments, R>(definition: object, spec: CommandSpec<N, A, R>): Command<A, R> {
  const info: CommandInfo = { definition, spec };
  const command = (args: CommandInput<A> = {} as CommandInput<A>): Promise<R> => {
    try {
      documentFor(definition);
      if (!info.name) throw new Error("Register the command in defineSlop({ commands })");
      return current().document.runCommand(info.name, JSON.parse(JSON.stringify(args)));
    } catch (error) { return Promise.reject(error); }
  };
  Object.defineProperty(command, brand, { value: info });
  return command as Command<A, R>;
}
