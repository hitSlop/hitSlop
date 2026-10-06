import { join } from "node:path";
import { homedir } from "node:os";
import { HelperProtocol } from "@hitslop/schema/constants";
import { parseHelperProtocol } from "@hitslop/schema/helper";
import { SocketReplySchema, type HelperRequest, type SocketReply } from "@hitslop/schema/socket";
import { validate } from "@hitslop/schema/validation";
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
    throw new Error("Document commands and export require macOS and hitSlop.app; init, check, dev and build run anywhere.");
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
  const { code } = await exec([...(await negotiate(await findNative())), ...args], { inherit: ["stdin", "stdout", "stderr"] });
  if (code) throw new ExitStatus(code);
}

/** One document request through the helper: the request on its standard input (the helper
 * supplies the owner's epoch), and the `SocketReply` it prints. */
export async function request(body: HelperRequest): Promise<SocketReply> {
  const { stdout, code } = await exec([...(await negotiate(await findNative())), "request"], { stdin: JSON.stringify(body), inherit: ["stderr"] });
  if (code) throw new ExitStatus(code);
  let reply: unknown;
  try {
    reply = JSON.parse(stdout);
  } catch {}
  return validate(SocketReplySchema, reply, "hitSlop.app sent an invalid reply; outcome unknown, run slop get before another edit");
}
