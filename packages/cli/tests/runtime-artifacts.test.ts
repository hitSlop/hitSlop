import { test, expect } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  catalog,
  verifyCopies,
  verifyReleasedIdentities,
  releases,
} from "../../../scripts/v1/runtime-artifacts";

test("runtime catalogs compare complete contract sets and bytes per contract", async () => {
  const root = await mkdtemp(join(tmpdir(), "runtime-catalog-"));
  const consumers = [join(root, "host"), join(root, "helper")];
  async function add(consumer: string, contract: number) {
    const folder = join(consumer, String(contract));
    await mkdir(join(folder, "loro"), { recursive: true });
    await writeFile(
      join(folder, "identity.json"),
      JSON.stringify({
        runtimeContract: contract,
        runtimeRevision: 1,
        sdkVersion: "1",
        loroVersion: "1",
        protocolVersion: 1,
      }),
    );
    for (const file of ["index.js", "boot.js", "headless.js", "loro/index.js", "loro/loro_wasm_bg.wasm"])
      await writeFile(join(folder, file), `contract ${contract}`);
  }
  try {
    for (const consumer of consumers) for (const contract of [1, 2]) await add(consumer, contract);
    await verifyCopies(consumers);
    await writeFile(join(consumers[1]!, "1/index.js"), "drift");
    await expect(verifyCopies(consumers)).rejects.toThrow("differ");
    await add(consumers[1]!, 1);
    await rm(join(consumers[1]!, "2"), { recursive: true });
    await expect(verifyCopies(consumers)).rejects.toThrow("differ");
    await rm(join(consumers[0]!, "1/headless.js"));
    await expect(catalog(consumers[0]!)).rejects.toThrow();
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("released contracts cannot disappear, regress or silently change bytes", async () => {
  const published = [{ runtimeContract: 1, runtimeRevision: 1, sha256: "a".repeat(64) }];
  const values: Awaited<ReturnType<typeof catalog>> = {};
  for (const release of published)
    values[String(release.runtimeContract)] = {
      identity: { ...release, sdkVersion: "1", loroVersion: "1", protocolVersion: 1 },
      sha256: release.sha256,
    };
  await verifyReleasedIdentities(values, published);
  const first = published[0]!;
  values[String(first.runtimeContract)]!.sha256 = "0".repeat(64);
  await expect(verifyReleasedIdentities(values, published)).rejects.toThrow("increment runtimeRevision");
  delete values[String(first.runtimeContract)];
  await expect(verifyReleasedIdentities(values, published)).rejects.toThrow("removed");
});
