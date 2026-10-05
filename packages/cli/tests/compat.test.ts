import { expect, test } from "bun:test";
import { stable, assertOutput } from "../../../scripts/compat";
import { sourceFingerprint } from "../../../scripts/compat-integrity";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

test("compatibility normalization preserves document fields named version and epoch", () => {
  const value = { version: "authored version", epoch: 42, nested: { version: "kept" } };
  expect(stable(value, ["get", "document.slop"])).toEqual(value);
  expect(stable({ state: { value, version: "session", sequence: 3 } }, ["get", "document.slop", "--snapshot"]))
    .toEqual({ state: { value } });
});

test("capture provenance changes with producing inputs but not the corpus commit", async () => {
  const root = await mkdtemp(join(tmpdir(), "compat-inputs-"));
  try {
    expect(Bun.spawnSync(["git", "init", "-q", root]).exitCode).toBe(0);
    await mkdir(join(root, "packages/document/src"), { recursive: true });
    await writeFile(join(root, "packages/document/src/app.ts"), "original SDK");
    const baseline = await sourceFingerprint(root);
    await mkdir(join(root, "tests/compat/1"), { recursive: true });
    await writeFile(join(root, "tests/compat/1/release.json"), "{}");
    expect(await sourceFingerprint(root)).toBe(baseline);
    await writeFile(join(root, "packages/document/src/app.ts"), "changed SDK");
    expect(await sourceFingerprint(root)).not.toBe(baseline);
  } finally { await rm(root, { recursive: true, force: true }); }
});

test("compatibility accepts added envelope metadata but detects changed document data", () => {
  const args = ["get", "document.slop", "--snapshot"];
  const expected = { schema: { kind: "object" }, state: { value: { version: "v1", epoch: 7 } } };
  expect(() => assertOutput({ ...expected, diagnostic: true, state: { ...expected.state, sequence: 9, extra: true } }, expected, args)).not.toThrow();
  expect(() => assertOutput({ ...expected, state: { ...expected.state, value: { version: "v2", epoch: 7 } } }, expected, args)).toThrow();
  expect(() => assertOutput({ state: expected.state }, expected, args)).toThrow();
});
