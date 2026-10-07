// The native tools as scripts run them: requests as the CLI sends them to the document
// engine, which passes an export to the selected rendering helper, and documents created
// from templates or build stages.
import { strict as assert } from "node:assert";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { execute, request, findDocumentEngine } from "../../packages/hitslop/src/cli/engine";
import type { EngineMethod, EngineRequestFor, EngineSuccess, EngineReplyFor } from "../../packages/hitslop/src/wire/engine";
import { repository } from "./artifacts";

/** The helper `bun run build` compiles, and this checkout's independent document engine. */
export const debugHelper = join(repository, "apps/apple/Packages/HitSlopApple/.build/debug/hitslop-native");
export const debugEngine = () => findDocumentEngine();
/** Which engine a script runs (the debug build's unless named) and, for a relocated or
 * installed one, the folder and environment it runs in. */
export type Placement = { engine?: string; cwd?: string; env?: Record<string, string | undefined> };
/** One request as the CLI sends it; a refusal is a reply. */
export function engineReply<M extends EngineMethod>(body: EngineRequestFor<M> & { method: M }, { engine, ...options }: Placement = {}): Promise<EngineReplyFor<M>> {
  return request<M>(body, { binary: engine, timeout: 120_000, ...options });
}
export function engineRequest<M extends EngineMethod>(body: EngineRequestFor<M> & { method: M }, { engine, ...options }: Placement = {}): Promise<Extract<EngineSuccess, { method: M }>> {
  return execute<M>(body, { binary: engine, timeout: 120_000, ...options });
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
  await engineRequest({ method: "create", from: template, output: document }, { engine, ...placement });
  return document;
}
/** A new document at `document` from a build stage, made the way a user's are: the CLI's
 * file engine packs the template, and the document engine creates the document from it. */
export async function documentFromStage(stage: string, document: string, options?: Placement) {
  const folder = await mkdtemp(join(tmpdir(), "hitslop-fixture-"));
  try {
    const template = join(folder, "template.slop");
    await engineRequest({ method: "pack", stage, file: template, app: JSON.parse(await readFile(join(stage, "input.json"), "utf8")) }, options);
    return await createDocument(template, document, options);
  } finally {
    await rm(folder, { recursive: true, force: true });
  }
}
