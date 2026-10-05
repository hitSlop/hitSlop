import { HelperProtocol } from "@hitslop/schema/constants";
import { SocketReplySchema, type HelperRequest, type HelperRequestFor, type SocketMethod, type SocketReply, type SocketReplyFor } from "@hitslop/schema/socket";
import { validate } from "@hitslop/schema/validation";
import { findDocumentEngine } from "./engine";
import { exec } from "./process";

/** A native tool exited without a reply, after printing why; the CLI exits with its status
 * (protocol 1), and `message` adds what that means for the command. */
export class ExitStatus extends Error {
  constructor(readonly code: number, message = "") {
    super(message);
  }
}

/** A native tool's command in this CLI's protocol. A tool that does not serve it refuses
 * with exit status 2 and says which side to update; app updates keep serving older ones. */
export function negotiate(binary: string): string[] {
  return [binary, "--client-protocol", String(HelperProtocol.version)];
}

/** `create`, or `open` in a window, through the document engine. */
export async function runEngine(args: string[]) {
  const { code } = await exec([...negotiate(await findDocumentEngine()), ...args], { inherit: ["stdin", "stdout", "stderr"] });
  if (code) throw new ExitStatus(code);
}

/** The document engine routes requests to the live or in-process owner, and passes an
 * export to the app's renderer. */
export function request<M extends SocketMethod>(body: HelperRequestFor<M> & { method: M }): Promise<SocketReplyFor<M>>;
export async function request(body: HelperRequest): Promise<SocketReply> {
  const { stdout, code } = await exec([...negotiate(await findDocumentEngine()), "request"], { stdin: JSON.stringify(body), inherit: ["stderr"] });
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

