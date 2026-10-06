import { test, expect } from "bun:test";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { createHash } from "node:crypto";
import { acceptanceContracts, currentAppAcceptance, currentStorageAcceptance } from "../../../../scripts/build/acceptance";

test("released acceptance survives tighter authoring and refuses changed records", async () => {
  const root = await mkdtemp(join(tmpdir(), "hitslop-acceptance-"));
  try {
    await mkdir(join(root, "tests/compat/released"), { recursive: true });
    await mkdir(join(root, "packages/hitslop/acceptance"), { recursive: true });
    const app = structuredClone(currentAppAcceptance);
    // A historical reader permitted a larger title and attachment than authoring now does.
    app.manifest.properties.title.maxLength = 1000;
    app.commands.maxProperties = 128;
    const storage = structuredClone(currentStorageAcceptance);
    storage.AttachmentLimits.file = 20 * 1024 * 1024 as typeof storage.AttachmentLimits.file;
    const records = {
      "packages/hitslop/acceptance/packageFormat-1.json": JSON.stringify(app),
      "packages/hitslop/acceptance/storage-1.json": JSON.stringify(storage),
    };
    for (const [path, text] of Object.entries(records)) await writeFile(join(root, path), text);
    await writeFile(join(root, "tests/compat/released/release.json"), JSON.stringify({
      release: "released", frozen: true, markers: { packageFormat: 1, storage: 1 },
      acceptance: Object.fromEntries(Object.entries(records).map(([path, text]) => [path, createHash("sha256").update(text).digest("hex")])),
    }));
    const accepted = await acceptanceContracts(root);
    expect(accepted.app.manifest.properties.title.maxLength).toBe(1000);
    expect(JSON.parse(accepted.outputs["packages/hitslop/generated/commands-format-1.schema.json"]!).maxProperties).toBe(128);
    expect(accepted.storage.AttachmentLimits.file).toBe(20 * 1024 * 1024);
    await writeFile(join(root, "packages/hitslop/acceptance/storage-1.json"), JSON.stringify(currentStorageAcceptance));
    await expect(acceptanceContracts(root)).rejects.toThrow("Released acceptance changed");
  } finally { await rm(root, { recursive: true, force: true }); }
});
