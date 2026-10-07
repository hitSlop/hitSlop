import { HelperProtocol, OutcomeCodes, SocketLimits } from "../schema/constants";
import type { EngineRequest, EngineRequestFor, EngineMethod, EngineReply, EngineReplyFor, EngineSuccess } from "../wire/engine";
import { join } from "node:path";
import { cliRoot, cliPackage, isGlobalInstall } from "./paths";
import { findExecutable, exec } from "./process";

/** The file engine (`crates/slop-engine`): it builds and reads `.slop` files by the rules
 * the app opens them with, on any platform. `HITSLOP_ENGINE` names one; otherwise the
 * build for this platform installed with the CLI, or a checkout's. */
export function findEngine(): Promise<string> {
  return findExecutable(
    "HITSLOP_ENGINE",
    [join(cliRoot, "engine", `${process.platform}-${process.arch}`, "slop-engine"), join(cliRoot, "../../target", process.env.HITSLOP_CARGO_PROFILE || "release", "slop-engine")],
    `hitslop has no file engine for ${process.platform}-${process.arch}. Reinstall hitslop.`,
  );
}

/** Document commands always use the engine shipped with the invoked CLI. */

/** The permanent engine preflight refusal: exit 2, before touching a document. */
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

export type EngineOptions = { binary?: string; cwd?: string; env?: Record<string, string | undefined>; timeout?: number };
/** A completed invocation has exactly one matching acknowledgement. The same-build
 * peer owns payload validation; malformed transport never becomes a successful edit. */
export function request<M extends EngineMethod>(body: EngineRequestFor<M> & { method: M }, options?: EngineOptions): Promise<EngineReplyFor<M>>;
export async function request(body: EngineRequest, { binary, ...options }: EngineOptions = {}): Promise<EngineReply> {
  const stdin = JSON.stringify(body);
  if (Buffer.byteLength(stdin) > SocketLimits.attachment) throw new Error("Engine request is too large");
  const command = negotiate(binary ?? await findEngine());
  const unknown = body.method === "batch" || body.method === "call"
    ? "Outcome unknown. Run slop get before issuing another edit."
    : "Outcome unknown. Inspect the document and output before retrying.";
  const { stdout, stderr, code } = await exec(command, { ...options, stdin }).catch((error: Error) => {
    throw new Error(`${error.message}\n${unknown}`, { cause: error });
  });
  if (code === 2) throw new ExitStatus(2, withRemedy(stderr.trim()));
  if (code) throw new Error([stderr.trim() || `Engine exited with status ${code}`, unknown].join("\n"));
  let reply: unknown;
  try { reply = JSON.parse(stdout); } catch {}
  const message = `Invalid engine reply. ${unknown}`;
  if (!acknowledged(reply, body.method)) throw new Error(message);
  return reply;
}

const replyFields = {
  get: ["state"], batch: ["ids"], export: ["output"], "theme.export": ["state"],
  "attachments.list": ["state"], "attachments.read": ["state"], call: ["result", "ids"],
  templates: ["catalog"], create: ["documentPath"], inspect: ["info"], schema: ["schema"],
  pack: [], validateApp: [], validateMetadata: [], describe: ["state"], open: ["documentPath"], screenshot: ["output"],
} satisfies Record<EngineMethod, readonly string[]>;
function acknowledged(value: unknown, method: EngineMethod): value is EngineReply {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const reply = value as Record<string, unknown>;
  if (reply.ok === false) {
    return typeof reply.error === "string" && typeof reply.code === "string"
      && (OutcomeCodes as readonly string[]).includes(reply.code)
      && (reply.reason === undefined || typeof reply.reason === "string")
      && (reply.opIndex === undefined || (Number.isSafeInteger(reply.opIndex) && (reply.opIndex as number) >= 0))
      && Object.keys(reply).every(key => ["ok", "error", "code", "reason", "opIndex"].includes(key));
  }
  if (reply.ok !== true || reply.method !== method) return false;
  const fields = ["ok", "method", ...replyFields[method]];
  if (fields.some(key => !Object.hasOwn(reply, key)) || Object.keys(reply).some(key => !fields.includes(key))) return false;
  if (method === "batch" || method === "call") return Array.isArray(reply.ids) && reply.ids.every(id => typeof id === "string");
  if (method === "export") return typeof reply.output === "string";
  if (method === "screenshot") return reply.output === null || typeof reply.output === "string";
  if (method === "create" || method === "open") return typeof reply.documentPath === "string";
  return true;
}
/** Callers that do not present edit outcomes can unwrap a successful typed result. */
export async function execute<M extends EngineMethod>(body: EngineRequestFor<M> & { method: M }, options?: EngineOptions): Promise<Extract<EngineSuccess, { method: M }>> {
  const reply = await request<M>(body, options);
  if (!reply.ok) {
    const error = reply.reason === "requires_update" ? withRemedy(reply.error) : reply.error;
    const message = reply.reason ? `${error} (${reply.reason})` : error;
    throw new Error(reply.code === "unknown_outcome" ? `${message}\nOutcome unknown. Inspect the document and output before retrying.` : message);
  }
  return reply as Extract<EngineSuccess, { method: M }>;
}
