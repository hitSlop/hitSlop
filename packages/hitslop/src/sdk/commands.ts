import { Type, type Static, type TSchema } from "typebox";
import { commandArgs, CommandName } from "../schema/commands";
import { validate } from "../schema/validation";
import type { ObjectNode, Value } from "./schema";
import type { Scope } from "./abi";
import { current, documentFor } from "./app/context";

export interface CommandContext<N extends ObjectNode> {
  readonly current: Value<N>;
  readonly tx: Scope<N>;
  readonly now: number;
  random(): number;
}
export type Command<A extends TSchema, R> = ({} extends Static<A> ? (args?: Static<A>) => Promise<R> : (args: Static<A>) => Promise<R>);
export type CommandSpec<N extends ObjectNode, A extends TSchema, R> = {
  description: string;
  args: A;
  run(ctx: CommandContext<N>, args: Static<A>): R;
};
const brand = Symbol.for("hitslop.command");
export type CommandInfo = { definition: object; spec: CommandSpec<any, TSchema, unknown>; name?: string };
export function commandInfo(value: unknown): CommandInfo | undefined {
  return typeof value === "function" ? (value as any)[brand] : undefined;
}
export function bindCommands(commands: Record<string, unknown>, definition: object) {
  if (Object.keys(commands).length > 64) throw new Error("A slop supports at most 64 commands");
  const seen = new Set<CommandInfo>();
  for (const [name, value] of Object.entries(commands)) {
    validate(CommandName, name, "Invalid command name");
    const info = commandInfo(value);
    if (!info || info.definition !== definition) throw new Error(`commands.ts export ${name} must be a command from this schema.ts definition`);
    if (seen.has(info) || (info.name && info.name !== name)) throw new Error(`Command ${name} has multiple export names`);
    seen.add(info);
    info.name = name;
  }
}
export function makeCommand<N extends ObjectNode, A extends TSchema, R>(definition: object, spec: CommandSpec<N, A, R>): Command<A, R> {
  validate(Type.String({ minLength: 1, maxLength: 500 }), spec.description, "Invalid command description");
  commandArgs(spec.args);
  if (typeof spec.run !== "function") throw new Error("A command needs run(ctx, args)");
  const info: CommandInfo = { definition, spec };
  const command = (args: Static<A> = {} as Static<A>): Promise<R> => {
    try {
      documentFor(definition);
      if (!info.name) throw new Error("Export the command from commands.ts before calling it");
      const checked = validate(spec.args, args, `Invalid arguments for ${info.name}`);
      const input = JSON.parse(JSON.stringify(checked));
      return current().document.runCommand(info.name, ctx => spec.run(ctx as CommandContext<N>, freeze(input)));
    } catch (error) { return Promise.reject(error); }
  };
  Object.defineProperty(command, brand, { value: info });
  return command as Command<A, R>;
}
function freeze<T>(value: T): T {
  if (value && typeof value === "object") { Object.values(value).forEach(freeze); Object.freeze(value); }
  return value;
}
