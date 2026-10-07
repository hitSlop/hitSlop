import { current, documentFor } from "./app/context";

/** Emitted by the explicit-entry UI compiler. No command body crosses this boundary.
 * Both entry points call the owner with the registered name and arguments. */
export function commandStub(definition: object, spec: { description: string; args: unknown }) {
  const info = { definition, spec, name: undefined as string | undefined };
  const command = (args: unknown = {}) => {
    try {
      documentFor(definition);
      if (!info.name) throw new Error("Register the command in defineSlop({ commands })");
      const input = JSON.parse(JSON.stringify(args));
      const owner = current().document as unknown as { runCommand(name: string, args: unknown): Promise<unknown> };
      return owner.runCommand(info.name, input);
    } catch (error) { return Promise.reject(error); }
  };
  Object.defineProperty(command, Symbol.for("hitslop.command"), { value: info });
  return command;
}
