// The native tools as scripts run them: requests as the CLI sends them to the document
// engine, which passes an export to the selected rendering helper, and documents created
// from templates or build stages.
import { strict as assert } from "node:assert";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pack, findDocumentEngine } from "../../packages/hitslop/src/cli/engine";
import { negotiate } from "../../packages/hitslop/src/cli/native";
import { exec, run } from "../../packages/hitslop/src/cli/process";
import { SocketReplySchema, type SocketSuccessFor, type HelperRequestFor, type SocketMethod, type SocketReply } from "../../packages/hitslop/src/schema/socket";
import { validate } from "../../packages/hitslop/src/schema/validation";
import { repository } from "./artifacts";

/** The helper `bun run build` compiles, and this checkout's independent document engine. */
export const debugHelper = join(repository, "apps/apple/Packages/HitSlopApple/.build/debug/hitslop-native");
export const debugEngine = () => findDocumentEngine();
/** Which engine a script runs (the debug build's unless named) and, for a relocated or
 * installed one, the folder and environment it runs in. */
export type Placement = { engine?: string; cwd?: string; env?: Record<string, string | undefined> };
type Method = SocketMethod;
type Request<M extends Method> = HelperRequestFor<M> & { method: M };

/** One request as the CLI sends it, and the reply the engine prints; a refusal is a reply. */
export async function engineReply<M extends Method>(body: Request<M>, { engine, ...placement }: Placement = {}): Promise<SocketReply> {
  engine ??= await debugEngine();
  const { stdout, stderr, code } = await exec([...negotiate(engine), "request"], { ...placement, stdin: JSON.stringify(body), timeout: 120_000 });
  if (code) throw new Error(`slop-engine request ${body.method} failed (${code}): ${stderr.trim()}`);
  const reply = validate(SocketReplySchema, JSON.parse(stdout), `slop-engine ${body.method} reply`);
  if (reply.ok && reply.method !== body.method) throw new Error(`slop-engine ${body.method} returned ${reply.method}`);
  return reply;
}
/** A request's result, as its method's contract requires it; a refusal fails. */
export async function engineRequest<M extends Method>(body: Request<M>, options?: Placement): Promise<SocketSuccessFor<M>> {
  const reply = await engineReply(body, options);
  if (!reply.ok) throw new Error(`${body.method}: ${reply.error}`);
  return reply as SocketSuccessFor<M>;
}
/** An export at `path` that is a real file of its format: past 100 bytes, with the PNG or PDF
 * signature. */
export async function assertExport(path: string, format: "png" | "pdf") {
  const bytes = await readFile(path);
  const signature = format === "png" ? "89504e470d0a1a0a" : "25504446";
  assert.ok(bytes.length > 100, `${path}: an empty ${format}`);
  assert.equal(bytes.subarray(0, signature.length / 2).toString("hex"), signature, `${path}: not a ${format}`);
}
/** A new document at `document` from a built template. */
export async function createDocument(template: string, document: string, { engine, ...placement }: Placement = {}) {
  engine ??= await debugEngine();
  await run([...negotiate(engine), "create", "--from", template, "--output", document], { ...placement, failure: `Cannot create ${document}` });
  return document;
}
/** A new document at `document` from a build stage, made the way a user's are: the CLI's
 * file engine packs the template, and the document engine creates the document from it. */
export async function documentFromStage(stage: string, document: string, options?: Placement) {
  const folder = await mkdtemp(join(tmpdir(), "hitslop-fixture-"));
  try {
    const template = join(folder, "template.slop");
    await pack(stage, template);
    return await createDocument(template, document, options);
  } finally {
    await rm(folder, { recursive: true, force: true });
  }
}
