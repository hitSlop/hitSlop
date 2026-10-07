import { mkdtemp, mkdir, readFile, rename, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { cliRoot } from "./paths";
import { start } from "./process";
import { execute } from "./engine";
import { assertProjectPackage } from "./project";
import type { BuildInput } from "../wire/app.generated";

/** Suggested project name for init and repository inventory, never a stored app slug. */
export function projectSlug(source: string): string { return basename(resolve(source)); }
/** Builds `source` into `stage` in a separate Bun process. It writes its explicit
 * inventory to a result file, never stdout, which the author's build tools may print to. */
export function stageWorker(source: string, stage: string, failure: string) {
  const result = `${resolve(stage)}.result.json`;
  const { done, kill } = start([process.execPath, join(cliRoot, "src/cli/stage-worker.ts"), resolve(source), resolve(stage), result], { cwd: cliRoot, failure });
  return {
    kill,
    done: done.then(async (): Promise<{ input: BuildInput; dependencies: { ui: string[]; definition: string[] } }> => {
      try { return JSON.parse(await readFile(result, "utf8")); }
      finally { await rm(result, { force: true }); }
    }),
  };
}
/** No runtime discovers stage files: the inventory is explicit. */
export async function stageProject(source: string, stage: string): Promise<BuildInput> {
  return (await stageWorker(source, stage, "Authoring build failed").done).input;
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
