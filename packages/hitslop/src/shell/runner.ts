/** The restricted command process has only ECMAScript and this protocol. */
import "./runner-globals";
import { evaluate, type CommandInput } from "./commands";
import { commandInfo } from "../sdk/commands";
import { validate } from "../schema/validation";

const refuse = () => { throw new Error("Use the host-provided ctx.now and ctx.random()"); };
const OriginalDate = Date;
// Explicit dates remain useful for formatting. Reading the ambient clock is forbidden.
(globalThis as any).Date = new Proxy(OriginalDate, {
  construct(target, args, newTarget) {
    if (!args.length) refuse();
    return Reflect.construct(target, args, newTarget);
  },
  apply: refuse,
  get(target, key, receiver) { return key === "now" ? refuse : Reflect.get(target, key, receiver); },
});
Math.random = refuse;
(globalThis as any).__hitslopRun = (input: string): string => {
  try {
    const request: CommandInput & { name: string } = JSON.parse(input);
    const commands = (globalThis as any).__slopCommands;
    const info = Object.hasOwn(commands ?? {}, request.name) ? commandInfo(commands[request.name]) : undefined;
    if (!info) throw new Error(`No command named ${request.name}`);
    validate(info.spec.args, request.args, `Invalid arguments for ${request.name}`);
    return JSON.stringify({ ok: true, ...evaluate(request, info.spec.run) });
  } catch (error) {
    return JSON.stringify({ ok: false, error: error instanceof Error ? error.message : String(error) });
  }
};
