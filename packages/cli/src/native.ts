import { join } from "node:path";
import { homedir } from "node:os";
import { HelperProtocol } from "@hitslop/schema/constants";
import { parseHelperProtocol } from "@hitslop/schema/helper";
import { SocketReplySchema, type HelperRequest, type HelperRequestFor, type SocketMethod, type SocketReply, type SocketReplyFor } from "@hitslop/schema/socket";
import { validate } from "@hitslop/schema/validation";
import { findDocumentEngine } from "./engine";
import { exec, findExecutable } from "./process";

/** The helper exited without a reply, after printing why; the CLI exits with its status
 * (protocol 1), and `message` adds what that means for the command. */
export class ExitStatus extends Error {
  constructor(readonly code: number, message = "") {
    super(message);
  }
}

/** The app's helper: `HITSLOP_NATIVE_CLI` names one; otherwise the installed app's. */
export function findNative(): Promise<string> {
  if (process.platform !== "darwin")
    throw new Error("Opening windows and exporting require macOS and hitSlop.app; document edits and authoring run anywhere.");
  return findExecutable(
    "HITSLOP_NATIVE_CLI",
    ["/Applications/hitSlop.app/Contents/Helpers/hitslop-native", join(homedir(), "Applications/hitSlop.app/Contents/Helpers/hitslop-native")],
    "Install hitSlop.app in /Applications or ~/Applications to use native document commands",
  );
}

/** Refuses a helper that no longer serves, or does not yet serve, this CLI's protocol.
 * Any compatible app build works: app updates keep serving older protocols. */
function checkProtocol(reported: string) {
  let served: { version?: unknown; minimum?: unknown } | undefined;
  try {
    served = JSON.parse(reported);
  } catch {}
  const { version, minimum } = parseHelperProtocol(served);
  if (HelperProtocol.version < minimum)
    throw new Error("hitSlop.app no longer supports this @hitslop/cli; update @hitslop/cli");
  if (HelperProtocol.version > version)
    throw new Error("This @hitslop/cli needs a newer hitSlop.app; update hitSlop");
}

/** The helper's arguments for this CLI's protocol, once the helper confirms it serves it. */
export async function negotiate(binary: string): Promise<string[]> {
  const { stdout, code } = await exec([binary, "--protocol"], { inherit: ["stderr"] });
  if (code) throw new ExitStatus(code);
  checkProtocol(stdout);
  return [binary, "--client-protocol", String(HelperProtocol.version)];
}

export async function runNative(args: string[]) {
  const { code } = await exec([...(await negotiate(await (args[0] === "create" ? findDocumentEngine() : findNative()))), ...args], { inherit: ["stdin", "stdout", "stderr"] });
  if (code) throw new ExitStatus(code);
}

/** The Rust engine routes document requests to the live or in-process owner. Export
 * uses the native renderer. Both serve the same request/reply protocol. */
export function request<M extends SocketMethod>(body: HelperRequestFor<M> & { method: M }): Promise<SocketReplyFor<M>>;
export async function request(body: HelperRequest): Promise<SocketReply> {
  const binary = await (body.method === "export" ? findNative() : findDocumentEngine());
  const { stdout, code } = await exec([...(await negotiate(binary)), "request"], { stdin: JSON.stringify(body), inherit: ["stderr"] });
  if (code) throw new ExitStatus(code);
  let reply: unknown;
  try {
    reply = JSON.parse(stdout);
  } catch {}
  const message = "Document owner sent an invalid reply; outcome unknown, run slop get before another edit";
  const checked = validate(SocketReplySchema, reply, message);
  if (checked.ok && checked.method !== body.method) throw new Error(message);
  return checked;
}

/** Saved metadata uses the selected document engine without loading an authored app. */
export async function readDocument(command: "schema" | "inspect", path: string): Promise<string> {
  const { stdout, code } = await exec([...(await negotiate(await findDocumentEngine())), command, path], { inherit: ["stderr"] });
  if (code) throw new ExitStatus(code);
  return stdout;
}
