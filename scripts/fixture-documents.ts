import { cp, mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { pack } from "../packages/cli/src/engine";

/** A copy of a build stage (`tests/fixtures/<name>/document`, for example) for a script to
 * change before `documentFromStage` packs it. */
export async function copyStage(source: string, folder: string) {
  const stage = join(folder, "stage-" + crypto.randomUUID());
  await cp(source, stage, { recursive: true });
  return stage;
}

/** A new document at `document` from a build stage, made the way a user's are: the file
 * engine packs the template, and the native helper creates the document from it. */
export async function documentFromStage(stage: string, document: string, helper: string) {
  const folder = await mkdtemp(join(tmpdir(), "hitslop-fixture-"));
  try {
    const template = join(folder, "template.slop");
    await pack(stage, template);
    const child = Bun.spawn([helper, "create", "--from", template, "--output", document], { stdout: "ignore", stderr: "pipe" });
    const [error, code] = await Promise.all([new Response(child.stderr).text(), child.exited]);
    if (code) throw new Error(`Cannot create ${document}: ${error}`);
    return document;
  } finally {
    await rm(folder, { recursive: true, force: true });
  }
}
