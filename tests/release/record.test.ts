import { expect, test } from "bun:test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { sha256 } from "../../scripts/lib/artifacts";
import { advancesBuild, appcastBuild, preservesLatest, validateRecord, verifyArtifacts, type ReleaseRecord } from "../../scripts/release/record";

test("resumed candidates require their original identity and complete, unchanged artifacts", async () => {
  const folder = await mkdtemp(join(tmpdir(), "hitslop-release-record-"));
  const saved: ReleaseRecord = { tag: "v7.2.0", macVersion: "7.2.0", macBuild: "42", commit: "candidate", run: "123", artifacts: {} };
  try {
    for (const name of ["hitslop-7.2.0.tgz", "hitSlop.dmg", "hitSlop.zip", "appcast.xml"]) {
      await writeFile(join(folder, name), name);
      saved.artifacts[name] = sha256(Buffer.from(name));
    }
    validateRecord(saved, saved.tag, saved.commit);
    await verifyArtifacts(saved, folder);
    expect(() => validateRecord(saved, saved.tag, "another-commit")).toThrow("different tag or commit");
    expect(() => validateRecord({ ...saved, artifacts: {} }, saved.tag, saved.commit)).toThrow("missing");
    expect(() => validateRecord({ ...saved, artifacts: { ...saved.artifacts, "../outside": "a".repeat(64) } }, saved.tag, saved.commit)).toThrow("Invalid artifact");
    await writeFile(join(folder, "hitSlop.dmg"), "rebuilt");
    await expect(verifyArtifacts(saved, folder)).rejects.toThrow("Artifact differs");
  } finally { await rm(folder, { recursive: true, force: true }); }
});

test("resuming an older release cannot move latest backwards", () => {
  expect(advancesBuild("10", "9")).toBe(true);
  expect(advancesBuild("9", "10")).toBe(false);
  expect(advancesBuild("10", "10")).toBe(false);
  expect(() => advancesBuild("NaN", "10")).toThrow("Invalid Mac build");
  expect(appcastBuild('<rss><sparkle:version>29</sparkle:version><enclosure sparkle:version="30"/></rss>')).toBe("30");
  expect(() => appcastBuild("<rss/>")).toThrow("no Mac build");
  expect(preservesLatest("1.2.0", "1.10.0")).toBe(false);
  expect(preservesLatest("1.10.0", "1.2.0")).toBe(true);
  expect(preservesLatest("1.2.0", "1.2.0")).toBe(true);
});
