import { chmod, copyFile, mkdir, readdir, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { run } from "../../packages/hitslop/src/cli/process";
import { publishFolder } from "../lib/artifacts";


/** Stages the file engines the CLI carries at `engines`, where `findEngine` looks: `fresh`,
 * this machine's build, and prebuilt ones for other platforms (`prebuilt/<platform>-<arch>/`,
 * from the Engines workflow). Each ships `engine.json`, the commit and core build it came
 * from; a prebuilt engine from another commit or core is refused, so every platform
 * validates by the same rules. The Engines workflow's build of this machine's own platform
 * ships as built when it matches (a frozen corpus entry records that build as its writer,
 * and this machine's own build is not byte-identical to it); otherwise `fresh` ships.
 * Stages beside `engines` and renames, so a refusal leaves nothing a later build could
 * pick up. */
export async function stageEngines(engines: string, prebuilt: string, fresh: string, commit: string) {
  const expected = { commit, buildId: (await run([fresh, "--build-id"])).trim() };
  await publishFolder(engines, async (staging) => {
    const stage = async (platform: string, binary: string) => {
      await mkdir(join(staging, platform), { recursive: true });
      await copyFile(binary, join(staging, platform, "slop-engine"));
      await chmod(join(staging, platform, "slop-engine"), 0o755);
      await writeFile(join(staging, platform, "engine.json"), JSON.stringify(expected) + "\n");
    };
    const host = `${process.platform}-${process.arch}`;
    const hostBinary = join(prebuilt, host, "slop-engine");
    const hostRecorded = existsSync(hostBinary)
      ? await Bun.file(join(prebuilt, host, "engine.json")).json().catch(() => ({}))
      : {};
    const hostMatches = hostRecorded.commit === expected.commit && hostRecorded.buildId === expected.buildId;
    await stage(host, hostMatches ? hostBinary : fresh);
    for (const platform of await readdir(prebuilt).catch(() => [] as string[])) {
      const binary = join(prebuilt, platform, "slop-engine");
      if (platform === host || !existsSync(binary)) continue;
      const recorded = await Bun.file(join(prebuilt, platform, "engine.json")).json().catch(() => ({}));
      if (recorded.commit !== expected.commit || recorded.buildId !== expected.buildId)
        throw new Error(
          `${join(prebuilt, platform)} was built from ${recorded.commit ?? "an unrecorded commit"} (core ${recorded.buildId ?? "unknown"}), not this checkout (${expected.commit}, core ${expected.buildId}). Download this commit's Engines artifacts, or remove ${prebuilt}.`,
        );
      await stage(platform, binary);
    }
  });
}
