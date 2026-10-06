import { expect, test } from "bun:test";
import { sourceFingerprint } from "../../../../scripts/compat/integrity";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

test("capture provenance changes with producing inputs but not the corpus commit", async () => {
  const root = await mkdtemp(join(tmpdir(), "compat-inputs-"));
  try {
    expect(Bun.spawnSync(["git", "init", "-q", root]).exitCode).toBe(0);
    await mkdir(join(root, "packages/hitslop/src/sdk"), { recursive: true });
    await writeFile(join(root, "packages/hitslop/src/sdk/app.ts"), "original SDK");
    const baseline = await sourceFingerprint(root);
    await mkdir(join(root, "tests/compat/1"), { recursive: true });
    await writeFile(join(root, "tests/compat/1/release.json"), "{}");
    expect(await sourceFingerprint(root)).toBe(baseline);
    await writeFile(join(root, "packages/hitslop/src/sdk/app.ts"), "changed SDK");
    expect(await sourceFingerprint(root)).not.toBe(baseline);
  } finally { await rm(root, { recursive: true, force: true }); }
});
