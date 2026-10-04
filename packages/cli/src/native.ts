import { join } from "node:path";
import { homedir } from "node:os";
import { HelperProtocol } from "@hitslop/schema/constants";
import { parseHelperProtocol } from "@hitslop/schema/helper";
import { SocketReplySchema, type SocketReply } from "@hitslop/schema/socket";
import { validate } from "@hitslop/schema/validation";
import { findExecutable } from "./process";

/** The app's helper: `HITSLOP_NATIVE_CLI` names one; otherwise the installed app's. */
export function findNative(): Promise<string> {
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
  const probe = Bun.spawn([binary, "--protocol"], {
    stdin: "ignore", stdout: "pipe", stderr: "inherit",
  });
  const [reported, status] = await Promise.all([
    new Response(probe.stdout).text(), probe.exited,
  ]);
  if (status) process.exit(status);
  checkProtocol(reported);
  return [binary, "--client-protocol", String(HelperProtocol.version)];
}

export async function runNative(args: string[]) {
  const child = Bun.spawn([...(await negotiate(await findNative())), ...args], {
    stdin: "inherit",
    stdout: "inherit",
    stderr: "inherit",
  });
  const code = await child.exited;
  if (code) process.exit(code);
}

/** One document request through the helper: the `SocketRequest` (the helper supplies the
 * owner's epoch) on its standard input, and the `SocketReply` it prints. */
export async function request(body: Record<string, unknown>): Promise<SocketReply> {
  const child = Bun.spawn([...(await negotiate(await findNative())), "request"], {
    stdin: "pipe",
    stdout: "pipe",
    stderr: "inherit",
  });
  child.stdin.write(JSON.stringify(body));
  await child.stdin.end();
  const [text, status] = await Promise.all([new Response(child.stdout).text(), child.exited]);
  if (status) process.exit(status);
  let reply: unknown;
  try {
    reply = JSON.parse(text);
  } catch {}
  return validate(SocketReplySchema, reply, "hitSlop.app sent an invalid reply; outcome unknown, run slop get before another edit");
}
