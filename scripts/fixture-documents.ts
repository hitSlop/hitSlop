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

/** Where a script runs the native helper: a relocated helper gets its own folder and a
 * minimal environment. */
type Placement = { cwd?: string; env?: Record<string, string | undefined> };

/** A new document at `document` from a built template, through the native helper. */
export async function createDocument(helper: string, template: string, document: string, placement: Placement = {}) {
  const child = Bun.spawn([helper, "create", "--from", template, "--output", document], { ...placement, stdout: "ignore", stderr: "pipe" });
  const [error, code] = await Promise.all([new Response(child.stderr).text(), child.exited]);
  if (code) throw new Error(`Cannot create ${document}: ${error}`);
  return document;
}

/** One request through the native helper, as the CLI sends it: a `SocketRequest` on
 * standard input, and the `SocketReply` it prints. */
export async function nativeRequest(helper: string, body: Record<string, unknown>, placement: Placement = {}): Promise<any> {
  const child = Bun.spawn([helper, "request"], { ...placement, stdin: "pipe", stdout: "pipe", stderr: "pipe" });
  child.stdin.write(JSON.stringify(body));
  await child.stdin.end();
  const timeout = setTimeout(() => child.kill(), 120_000);
  try {
    const [out, error, code] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
    if (code) throw new Error(`hitslop-native request ${body.method} failed (${code}): ${error.trim()}`);
    return JSON.parse(out);
  } finally {
    clearTimeout(timeout);
  }
}

/** A new document at `document` from a build stage, made the way a user's are: the file
 * engine packs the template, and the native helper creates the document from it. */
export async function documentFromStage(stage: string, document: string, helper: string) {
  const folder = await mkdtemp(join(tmpdir(), "hitslop-fixture-"));
  try {
    const template = join(folder, "template.slop");
    await pack(stage, template);
    return await createDocument(helper, template, document);
  } finally {
    await rm(folder, { recursive: true, force: true });
  }
}
