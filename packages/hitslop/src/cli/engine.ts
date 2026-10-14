import { json } from "./json";
import { EngineProtocol, SocketLimits } from "../schema/constants";
import type { EngineRequest, EngineRequestFor, EngineMethod, EngineSuccess } from "../wire/engine";
import type { Failure } from "../wire/engine.generated";
import type { OutcomeCode } from "../schema/values";
import { join } from "node:path";
import { cliRoot, cliPackage, isGlobalInstall } from "./paths";
import { findExecutable, exec } from "./process";

/** Failure codes grow additively; an unfamiliar one carries no certainty about the edit. */
type EngineFailure = Omit<Failure, "code"> & { code: OutcomeCode | (string & {}) };
export type EngineReply = EngineSuccess | EngineFailure;
export type EngineReplyFor<M extends EngineMethod> =
  Extract<EngineSuccess, { method: M }> | EngineFailure;

/** The file engine (`crates/slop-engine`): it builds and reads `.slop` files by the rules
 * the app opens them with, on any platform. `HITSLOP_ENGINE` names one; otherwise the
 * build for this platform installed with the CLI, or a checkout's. */
export function findEngine(): Promise<string> {
  return findExecutable(
    "HITSLOP_ENGINE",
    [
      join(cliRoot, "engine", `${process.platform}-${process.arch}`, "slop-engine"),
      join(cliRoot, "../../target", process.env.HITSLOP_CARGO_PROFILE || "release", "slop-engine"),
    ],
    `hitslop has no file engine for ${process.platform}-${process.arch}. Reinstall hitslop.`,
  );
}

/** The permanent engine preflight refusal: exit 2, before touching a document. */
export class ExitStatus extends Error {
  constructor(
    readonly code: number,
    message = "",
  ) {
    super(message);
  }
}

/** A native tool's command in this CLI's protocol. A tool that serves another refuses with
 * exit status 2 and one line naming the side to update; nothing was written. */
export function negotiate(binary: string): string[] {
  return [binary, "--client-protocol", String(EngineProtocol.version)];
}

/** The command that updates this copy of the CLI: the global install, a project's, or bunx. */
function cliUpdate(): string {
  if (isGlobalInstall) return `bun install -g ${cliPackage}@latest`;
  if (
    /[\\/]node_modules[\\/]/.test(cliRoot) &&
    !/[\\/]install[\\/]cache[\\/]|[\\/]bunx-/.test(cliRoot)
  )
    return `bun add -d ${cliPackage}@latest`;
  return `bunx ${cliPackage}@latest`;
}
/** A refusal that tells the CLI to update says how, for the copy that ran. */
function withRemedy(message: string): string {
  return message.replace("update the hitSlop CLI", `update the hitSlop CLI (${cliUpdate()})`);
}

export type EngineOptions = {
  binary?: string;
  cwd?: string;
  env?: Record<string, string | undefined>;
  timeout?: number;
};
/** A completed invocation has exactly one matching acknowledgement. The same-build
 * peer owns payload validation; malformed transport never becomes a successful edit. */
export function request<M extends EngineMethod>(
  body: EngineRequestFor<M> & { method: M },
  options?: EngineOptions,
): Promise<EngineReplyFor<M>>;
export async function request(
  body: EngineRequest,
  { binary, ...options }: EngineOptions = {},
): Promise<EngineReply> {
  const stdin = JSON.stringify(body);
  if (Buffer.byteLength(stdin) > SocketLimits.attachment)
    throw new Error("Engine request is too large");
  const command = negotiate(binary ?? (await findEngine()));
  const unknown =
    body.method === "batch" || body.method === "call"
      ? "Outcome unknown. Run slop get before issuing another edit."
      : "Outcome unknown. Inspect the document and output before retrying.";
  const { stdout, stderr, code } = await exec(command, { ...options, stdin }).catch(
    (error: Error) => {
      throw new Error(`${error.message}\n${unknown}`, { cause: error });
    },
  );
  if (code === 2) throw new ExitStatus(2, withRemedy(stderr.trim()));
  if (code)
    throw new Error([stderr.trim() || `Engine exited with status ${code}`, unknown].join("\n"));
  const reply = json(stdout);
  const message = `Invalid engine reply. ${unknown}`;
  if (!acknowledged(reply, body.method)) throw new Error(message);
  return reply;
}

const replyFields = {
  copy: ["output"],
  get: ["state"],
  batch: ["ids"],
  export: ["output"],
  "theme.export": ["state"],
  "attachments.ref": ["reference"],
  "attachments.list": ["state"],
  "attachments.read": ["state"],
  call: ["result", "ids"],
  "artwork.export": ["output"],
  templates: ["catalog"],
  create: ["documentPath"],
  inspect: ["info"],
  schema: ["schema"],
  pack: ["artwork"],
  validateApp: ["window", "capabilities"],
  validateMetadata: [],
  describe: ["state"],
  open: ["documentPath"],
} satisfies Record<EngineMethod, readonly string[]>;
/** A reply must carry the members its method needs; members it does not know are ignored, so a
 * newer engine may add some without making an applied mutation read as an unknown outcome. */
function acknowledged(value: unknown, method: EngineMethod): value is EngineReply {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const reply = value as Record<string, unknown>;
  if (reply.ok === false) {
    return (
      typeof reply.error === "string" &&
      typeof reply.code === "string" &&
      (reply.reason === undefined || typeof reply.reason === "string") &&
      (reply.opIndex === undefined ||
        (Number.isSafeInteger(reply.opIndex) && (reply.opIndex as number) >= 0))
    );
  }
  if (reply.ok !== true || reply.method !== method) return false;
  if (replyFields[method].some((key) => !Object.hasOwn(reply, key))) return false;
  if (method === "batch" || method === "call")
    return Array.isArray(reply.ids) && reply.ids.every((id) => typeof id === "string");
  if (method === "export" || method === "copy") return typeof reply.output === "string";
  if (method === "artwork.export") return reply.output === null || typeof reply.output === "string";
  if (method === "create" || method === "open") return typeof reply.documentPath === "string";
  return true;
}
/** What a failed edit means for the next one. */
const outcomes = {
  rejected: "Not applied.",
  owner_replaced: "Not applied. Run slop get before issuing another edit.",
  closing: "Not applied. Run slop get before issuing another edit.",
  owner_invalidated: "Not applied.",
  save_failed: "Applied, but not yet saved. Run slop get before issuing another edit.",
  unknown_outcome: "Outcome unknown. Run slop get before issuing another edit.",
} satisfies Record<OutcomeCode, string>;

/** One refusal format for authoring and document commands; transport validation stays above. */
export function unwrap<M extends EngineMethod>(
  reply: EngineReplyFor<M>,
  method: M,
): Extract<EngineSuccess, { method: M }> {
  if (!reply.ok) {
    const error = reply.reason === "requires_update" ? withRemedy(reply.error) : reply.error;
    const reason = reply.reason ? ` (${reply.reason})` : "";
    const message =
      reply.opIndex === undefined
        ? `${error}${reason}`
        : `Refused ops[${reply.opIndex}]${reason}: ${error}`;
    const edit = method === "batch" || method === "call";
    const outcome = !Object.hasOwn(outcomes, reply.code)
      ? edit
        ? "Outcome not recognized. Run slop get before issuing another edit."
        : "Outcome not recognized. Inspect the document and output before retrying."
      : edit
        ? outcomes[reply.code as OutcomeCode]
        : reply.code === "unknown_outcome"
          ? "Outcome unknown. Inspect the document and output before retrying."
          : undefined;
    throw new Error([message, outcome].filter(Boolean).join("\n"));
  }
  return reply as Extract<EngineSuccess, { method: M }>;
}

/** Invoke once and unwrap the method's acknowledged result. */
export async function execute<M extends EngineMethod>(
  body: EngineRequestFor<M> & { method: M },
  options?: EngineOptions,
): Promise<Extract<EngineSuccess, { method: M }>> {
  return unwrap<M>(await request<M>(body, options), body.method);
}
