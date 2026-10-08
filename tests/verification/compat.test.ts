import { expect, test } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { run } from "../../packages/hitslop/src/cli/process";
import { assertFrozenCorpus } from "../../scripts/compat/check";

test("released fixtures cannot be rewritten, unfrozen or deleted; new releases can be added", async () => {
  const root = await mkdtemp(join(tmpdir(), "hitslop-frozen-"));
  const git = (...args: string[]) => run(["git", ...args], { cwd: root });
  const entry = join(root, "tests/compat/1.0.0");
  const record = join(entry, "release.json");
  const document = join(entry, "document.slop");
  try {
    await git("init", "-q");
    await mkdir(entry, { recursive: true });
    await writeFile(record, '{"frozen":true}');
    await writeFile(document, "original released bytes");
    await git("add", ".");
    await git("-c", "user.name=Test", "-c", "user.email=test@example.invalid", "commit", "-qm", "release");
    const base = (await git("rev-parse", "HEAD")).trim();
    await assertFrozenCorpus(root, base);
    await mkdir(join(root, "tests/compat/1.1.0"));
    await writeFile(join(root, "tests/compat/1.1.0/release.json"), '{"frozen":true}');
    await git("add", ".");
    await assertFrozenCorpus(root, base);
    await writeFile(document, "replacement bytes");
    await expect(assertFrozenCorpus(root, base)).rejects.toThrow("Frozen compatibility corpus entries changed");
    await writeFile(document, "original released bytes");
    await writeFile(record, '{"frozen":false}');
    await expect(assertFrozenCorpus(root, base)).rejects.toThrow("Frozen compatibility corpus entries changed");
    await writeFile(record, '{"frozen":true}');
    await rm(document);
    await expect(assertFrozenCorpus(root, base)).rejects.toThrow("Frozen compatibility corpus entries changed");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
