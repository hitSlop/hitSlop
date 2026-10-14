import { expect, test } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { run } from "../../packages/hitslop/src/cli/process";
import { assertFrozenCorpus } from "../../scripts/compat/check";
import { compatibilityMode, replayedEntries, type Release } from "../../scripts/compat/corpus";

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

test("smoke samples marker generations; full replay retains older apps and saved scenarios too", () => {
  const markers = (runtimeABI: number) => ({ packageFormat: 1, runtimeABI, storage: 1, layout: 1, protocol: 1 });
  const entry = (name: string, captured: string, runtimeABI: number) =>
    ({ name, release: { captured, markers: markers(runtimeABI) } as Release });
  const entries = [entry("1.0.0", "2026-11-01", 1), entry("1.1.0", "2026-12-01", 1), entry("2.0.0", "2027-02-01", 2), entry("2.1.0", "2027-03-01", 2)];
  expect(replayedEntries(entries, false).map(({ name }) => name)).toEqual(["1.1.0", "2.1.0"]);
  expect(replayedEntries(entries, true)).toEqual(entries);
  expect(replayedEntries(entries.slice(0, 1), false)).toEqual(entries.slice(0, 1));
});

test("full, nightly and required-release replay cannot inherit smoke entry restrictions", () => {
  const smoke = { HITSLOP_COMPAT_MODE: "smoke", HITSLOP_COMPAT_ENTRIES: "newest-only" };
  expect(compatibilityMode(smoke)).toBe("smoke");
  expect(compatibilityMode({ ...smoke, HITSLOP_COMPAT_MODE: "full" })).toBe("full");
  expect(compatibilityMode({ ...smoke, HITSLOP_NIGHTLY: "1" })).toBe("full");
  expect(compatibilityMode({ ...smoke, HITSLOP_COMPAT_RELEASE: "1.0.0" })).toBe("full");
  expect(compatibilityMode({})).toBe("full");
  expect(() => compatibilityMode({ HITSLOP_COMPAT_MODE: "typo" })).toThrow("Invalid HITSLOP_COMPAT_MODE");
});
