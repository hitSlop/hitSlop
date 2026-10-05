// The native helper as scripts run it: requests as the CLI sends them, and documents
// created from templates or build stages.
import { strict as assert } from "node:assert";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pack } from "../packages/cli/src/engine";
import { exec, run } from "../packages/cli/src/process";
import { SocketReplySchema, type SocketSuccessFor, type HelperRequestFor, type SocketMethod, type SocketReply } from "../packages/schema/src/socket";
import { validate } from "../packages/schema/src/validation";
import { repository } from "./runtime-artifacts";

/** The helper `bun run build` compiles. */
export const debugHelper = join(repository, "apps/apple/Packages/HitSlopApple/.build/debug/hitslop-native");
/** Which helper a script runs (the debug build unless named) and, for a relocated or
 * installed one, the folder and environment it runs in. */
export type Helper = { helper?: string; cwd?: string; env?: Record<string, string | undefined> };
type Method = SocketMethod;
type Request<M extends Method> = HelperRequestFor<M> & { method: M };

/** One request as the CLI sends it, and the reply the helper prints; a refusal is a reply. */
export async function helperReply<M extends Method>(body: Request<M>, { helper = debugHelper, ...placement }: Helper = {}): Promise<SocketReply> {
  const { stdout, stderr, code } = await exec([helper, "request"], { ...placement, stdin: JSON.stringify(body), timeout: 120_000 });
  if (code) throw new Error(`hitslop-native request ${body.method} failed (${code}): ${stderr.trim()}`);
  const reply = validate(SocketReplySchema, JSON.parse(stdout), `hitslop-native ${body.method} reply`);
  if (reply.ok && reply.method !== body.method) throw new Error(`hitslop-native ${body.method} returned ${reply.method}`);
  return reply;
}
/** A request's result, as its method's contract requires it; a refusal fails. */
export async function helperRequest<M extends Method>(body: Request<M>, options?: Helper): Promise<SocketSuccessFor<M>> {
  const reply = await helperReply(body, options);
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
export async function createDocument(template: string, document: string, { helper = debugHelper, ...placement }: Helper = {}) {
  await run([helper, "create", "--from", template, "--output", document], { ...placement, failure: `Cannot create ${document}` });
  return document;
}
/** A new document at `document` from a build stage, made the way a user's are: the file
 * engine packs the template, and the helper creates the document from it. */
export async function documentFromStage(stage: string, document: string, options?: Helper) {
  const folder = await mkdtemp(join(tmpdir(), "hitslop-fixture-"));
  try {
    const template = join(folder, "template.slop");
    await pack(stage, template);
    return await createDocument(template, document, options);
  } finally {
    await rm(folder, { recursive: true, force: true });
  }
}
