import { test, expect } from "bun:test";
import { parseManifest, SlopManifestReadSchema } from "../src/index";
import { Check } from "typebox/value";
import { readFile } from "node:fs/promises";
test("manifest requires attribution and valid presentation", async () => {
  const manifest = JSON.parse(
    await readFile("examples/slops/quick-checklist/manifest.json", "utf8"),
  );
  expect(parseManifest(manifest).slug).toBe("quick-checklist");
  for (const value of [
    { ...manifest, runtime: "hitslop-v1" },
    { ...manifest, author: { name: "" } },
    { ...manifest, presentation: { width: 1, height: 1 } },
  ])
    expect(() => parseManifest(value)).toThrow();
});

test("manifest readers tolerate metadata additions but validate known presentation fields", async () => {
  const manifest = JSON.parse(await readFile("examples/slops/quick-checklist/manifest.json", "utf8"));
  const extended = {
    ...manifest, lineage: { template: "future" },
    author: { ...manifest.author, handle: "future" },
    categories: ["productivity", "future-category"],
    presentation: { ...manifest.presentation, shape: "future-shape", background: "future-background", future: true },
  };
  expect(Check(SlopManifestReadSchema, extended)).toBe(true);
  expect(() => parseManifest(extended)).toThrow();
  expect(Check(SlopManifestReadSchema, { ...extended, presentation: { width: 480, height: 620, skin: "assets/skin.png", future: true } })).toBe(true);
  for (const value of [
    { ...extended, $schema: "wrong" },
    { ...extended, author: { name: "" } },
    { ...extended, categories: ["x".repeat(65)] },
    { ...extended, categories: [""] },
    { ...extended, categories: ["same", "same"] },
    ...[
      { width: 1, height: 1 },
      { width: 480, height: 620, skin: "../evil.png" },
      { width: 480, height: 620, skin: null },
      { width: 480, height: 620, shape: null },
      { width: 480, height: 620, skin: "assets/skin.png", resizable: "wrong" },
      { width: 480, height: 620, skin: "assets/skin.png", resizable: true },
      { width: 480, height: 620, skin: "assets/skin.png", shape: 42 },
      { width: 480, height: 620, skin: "assets/skin.png", shape: "rounded" },
      { width: 480, height: 620, skin: "assets/skin.png", background: null },
      { width: 480, height: 620, shape: "x".repeat(65) },
    ].map(presentation => ({ ...extended, presentation })),
  ]) expect(Check(SlopManifestReadSchema, value)).toBe(false);
});
