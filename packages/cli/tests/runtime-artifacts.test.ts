import { test, expect } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { verifyShellCopies } from "../../../scripts/v1/runtime-artifacts";

// Failure: the app and the CLI dev server serve different page shells.
test("page shell copies must be byte-identical across consumers", async () => {
  const root = await mkdtemp(join(tmpdir(), "page-shell-"));
  const consumers = [join(root, "app"), join(root, "cli")];
  try {
    for (const consumer of consumers) {
      await mkdir(consumer, { recursive: true });
      for (const file of ["boot.js", "index.js"]) await writeFile(join(consumer, file), file);
    }
    // The CLI copy also carries the dev-only WASM core; it is not part of the shell digest.
    await mkdir(join(consumers[1]!, "core"));
    await writeFile(join(consumers[1]!, "core/hitslop_core_wasm_bg.wasm"), "wasm");
    await verifyShellCopies(consumers);
    await writeFile(join(consumers[1]!, "index.js"), "drift");
    await expect(verifyShellCopies(consumers)).rejects.toThrow("differ");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
