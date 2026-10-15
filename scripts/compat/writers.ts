import { strict as assert } from "node:assert";
import { chmod, copyFile, mkdir, mkdtemp, rename, rm, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { fileDigest, sha256 } from "../lib/artifacts";
import { run } from "../../packages/hitslop/src/cli/process";
import { historicalPackage } from "./corpora";
import { documentEngine, releases, type Release } from "./corpus";

export const writerCache = () =>
  resolve(process.env.HITSLOP_COMPAT_WRITERS ?? join(homedir(), ".cache/hitslop/compat-writers"));

/** Read only the packaged writer, without unpacking any archive paths onto disk. */
async function packedWriter(tarball: string): Promise<Uint8Array> {
  const child = Bun.spawn(
    ["/usr/bin/tar", "-xOzf", tarball, "package/engine/darwin-arm64/slop-engine"],
    { stdout: "pipe", stderr: "pipe" },
  );
  const [bytes, error, code] = await Promise.all([
    new Response(child.stdout).arrayBuffer(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  assert.equal(code, 0, `Cannot extract packaged writer: ${error}`);
  return new Uint8Array(bytes);
}

export async function verifyPackedWriter(tarball: string, writer: Release["writer"]) {
  assert.equal(
    sha256(await packedWriter(tarball)),
    writer.sha256,
    "The packed darwin-arm64 engine differs from the captured writer",
  );
}

/** Cached and local writers are checked by bytes too: a build ID identifies producing
 * sources, but does not establish that the binary is the shipped differential oracle. */
export async function resolveWriter(
  writer: Release["writer"],
  options: {
    cache?: string;
    local?: string;
    package?: () => Promise<string>;
  } = {},
): Promise<string | undefined> {
  assert.equal(writer?.platform, "darwin-arm64", "Missing writer metadata; recapture dev");
  assert.match(writer.sha256, /^[a-f0-9]{64}$/, "Invalid writer SHA-256");
  assert.ok(writer.buildId, "Missing writer build ID");
  assert.match(
    writer.version,
    /^(dev|\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?)$/,
    "Invalid writer version",
  );
  const cache = options.cache ?? writerCache(),
    destination = join(cache, writer.sha256);
  const cached = await fileDigest(destination).catch((error) => {
    if (error.code === "ENOENT") return undefined;
    throw error;
  });
  if (cached !== undefined) {
    assert.equal(cached, writer.sha256, `Corrupt cached writer: ${destination}`);
    await chmod(destination, 0o755);
    return destination;
  }
  await mkdir(cache, { recursive: true });
  const stage = await mkdtemp(join(cache, ".writer-"));
  try {
    const binary = join(stage, "slop-engine"),
      local = options.local ?? documentEngine();
    let localID: string | undefined;
    try {
      localID = (await run([local, "--build-id"])).trim();
    } catch {
      // Missing or unrunnable local builds use the published writer instead.
    }
    if (localID === writer.buildId && (await fileDigest(local)) === writer.sha256) {
      await copyFile(local, binary);
    } else {
      // An unpublished pre-launch entry's writer is the engine that captured it, and another
      // build cannot reproduce its bytes: its differential replay is skipped, visibly, until the
      // entry is frozen and published (then the writer resolves from the npm package).
      if (writer.version === "dev") return undefined;
      const tarball = await (options.package ?? (() => historicalPackage(writer.version)))();
      await writeFile(binary, await packedWriter(tarball));
    }
    assert.equal(
      await fileDigest(binary),
      writer.sha256,
      "Downloaded/local writer SHA-256 mismatch",
    );
    await chmod(binary, 0o755);
    if (process.platform === "darwin" && process.arch === "arm64")
      assert.equal(
        (await run([binary, "--build-id"])).trim(),
        writer.buildId,
        "Writer build ID mismatch",
      );
    await rename(binary, destination);
    return destination;
  } finally {
    await rm(stage, { recursive: true, force: true });
  }
}

/** Linux cannot run the shipped Mac writers; the Rust differential test has the same gate. */
export async function resolveWriters() {
  if (process.platform !== "darwin" || process.arch !== "arm64") return;
  for (const { name, release } of await releases()) {
    const writer = await resolveWriter(release.writer);
    console.log(
      writer
        ? `Compatibility writer ${name}: ${writer}`
        : `Compatibility writer ${name}: skipped (unpublished pre-launch entry; its writer is the engine that captured it)`,
    );
  }
}

if (import.meta.main) await resolveWriters();
