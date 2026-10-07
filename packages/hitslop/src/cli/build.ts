import { mkdtemp, mkdir, rename, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { cliRoot } from "./paths";
import { start } from "./process";
import { execute } from "./engine";
import { assertProjectPackage } from "./project";
import type { BuildInput } from "../wire/app.generated";

/** Suggested project name for init and repository inventory, never a stored app slug. */
export function projectSlug(source: string): string { return basename(resolve(source)); }
export const metadataFiles = ["slop.ts"] as const;
export function stageWorker(args: string[], failure: string) {
  return start([process.execPath, join(cliRoot, "src/cli/stage-worker.ts"), ...args], { cwd: cliRoot, failure });
}
/** The worker returns an explicit inventory through IPC. No runtime discovers stage files. */
export async function stageProject(source: string, stage: string): Promise<BuildInput> {
  return JSON.parse(await stageWorker([resolve(source), resolve(stage)], "Authoring build failed").done).input;
}
export async function checkProject(source: string) {
  const stage = await mkdtemp(join(tmpdir(), "hitslop-check-"));
  try { await stageProject(source, stage); }
  finally { await rm(stage, { recursive: true, force: true }); }
}
/** One compiler and one core acceptance path for checks, builds and development. */
export async function stageProjectInBun(source: string, stage: string) {
  source = resolve(source); stage = resolve(stage);
  assertProjectPackage(source);
  await mkdir(dirname(stage), {recursive:true});
  const ready = await mkdtemp(stage + ".building-");
  try {
    const result = await (await import("./definition-build")).buildDefinition(source, ready);
    await execute({method:"validateApp", stage:ready, app:result.input});
    await rm(stage, {recursive:true, force:true});
    await rename(ready, stage);
    return result;
  } catch (error) {
    await rm(ready, {recursive:true, force:true});
    throw error;
  }
}
