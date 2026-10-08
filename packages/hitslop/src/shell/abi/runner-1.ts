/** The restricted command process has only ECMAScript and this protocol. */
import { evaluate, type CommandInput } from "../commands";
import { commandInfo } from "../../sdk/commands";
import { isRefused } from "../../sdk/errors";

declare global {
  var __slopRun: (input: string) => string;
  var __slopCommands: Record<string, unknown> | undefined;
}

const refuse = () => { throw new Error("Use the host-provided ctx.now and ctx.random()"); };
const OriginalDate = Date;
// Explicit dates remain useful for formatting. Reading the ambient clock is forbidden.
const RestrictedDate = new Proxy(OriginalDate, {
  construct(target, args, newTarget) {
    if (!args.length) refuse();
    return Reflect.construct(target, args, newTarget);
  },
  apply: refuse,
  get(target, key, receiver) { return key === "now" ? refuse : Reflect.get(target, key, receiver); },
});
globalThis.Date = RestrictedDate;
// Every date reaches the constructor through its prototype; that path is restricted too.
Object.defineProperty(OriginalDate.prototype, "constructor", { value: RestrictedDate });
Math.random = refuse;
globalThis.__slopRun = (input: string): string => {
  try {
    const request = JSON.parse(input) as CommandInput & { name: string };
    const commands = globalThis.__slopCommands;
    const info = commands && Object.hasOwn(commands, request.name) ? commandInfo(commands[request.name]) : undefined;
    if (!info) throw new Error(`No command named ${request.name}`);
    return JSON.stringify({ ok: true, ...evaluate(request, info.spec.run, info.spec.args) });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return JSON.stringify({ ok: false, error: message, ...(isRefused(error) ? { refused: true } : {}) });
  }
};
