import { HelperProtocol } from "../schema/constants";
import { SocketReplySchema, type HelperRequest, type HelperRequestFor, type SocketMethod, type SocketReply, type SocketReplyFor } from "../schema/socket";
import { validate } from "../schema/validation";
import { findDocumentEngine } from "./engine";
import { cliPackage, cliRoot, isGlobalInstall } from "./paths";
import { exec } from "./process";

/** A native tool exited without a reply, after printing why; the CLI exits with its status
 * (protocol 1), and `message` adds what that means for the command. */
export class ExitStatus extends Error {
  constructor(readonly code: number, message = "") {
    super(message);
  }
}

/** A native tool's command in this CLI's protocol. A tool that serves another refuses with
 * exit status 2 and one line naming the side to update; nothing was written. */
export function negotiate(binary: string): string[] {
  return [binary, "--client-protocol", String(HelperProtocol.version)];
}

/** The command that updates this copy of the CLI: the global install, a project's, or bunx. */
export function cliUpdate(): string {
  if (isGlobalInstall) return `bun install -g ${cliPackage}@latest`;
  if (/[\\/]node_modules[\\/]/.test(cliRoot) && !/[\\/]install[\\/]cache[\\/]|[\\/]bunx-/.test(cliRoot))
    return `bun add -d ${cliPackage}@latest`;
  return `bunx ${cliPackage}@latest`;
}
/** A refusal that tells the CLI to update says how, for the copy that ran. */
export function withRemedy(message: string): string {
  return message.replace("update the hitSlop CLI", `update the hitSlop CLI (${cliUpdate()})`);
}
/** A native tool's failure: its exit status, and what it printed with the remedy named. */
function failed(code: number, stderr: string): ExitStatus {
  return new ExitStatus(code, withRemedy(stderr.trim()));
}

/** `create`, or `open` in a window, through the document engine. */
export async function runEngine(args: string[]) {
  const { code, stderr } = await exec([...negotiate(await findDocumentEngine()), ...args], { inherit: ["stdin", "stdout"] });
  if (code) throw failed(code, stderr);
  if (stderr) process.stderr.write(stderr);
}

/** The document engine routes requests to the live or in-process owner, and passes an
 * export to the app's renderer. */
export function request<M extends SocketMethod>(body: HelperRequestFor<M> & { method: M }): Promise<SocketReplyFor<M>>;
export async function request(body: HelperRequest): Promise<SocketReply> {
  const { stdout, stderr, code } = await exec([...negotiate(await findDocumentEngine()), "request"], { stdin: JSON.stringify(body) });
  if (code) throw failed(code, stderr);
  if (stderr) process.stderr.write(stderr);
  let reply: unknown;
  try {
    reply = JSON.parse(stdout);
  } catch {}
  const message = "Document owner sent an invalid reply; outcome unknown, run slop get before another edit";
  const checked = validate(SocketReplySchema, reply, message);
  if (checked.ok && checked.method !== body.method) throw new Error(message);
  return checked;
}

