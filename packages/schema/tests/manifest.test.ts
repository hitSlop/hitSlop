import { test, expect } from "bun:test";
import { parseManifest } from "../src/index";
import { readFile } from "node:fs/promises";
test("manifest requires attribution and valid presentation", async () => {
  const manifest = JSON.parse(
    await readFile("examples/slops/quick-checklist/manifest.json", "utf8"),
  );
  expect(parseManifest(manifest).slug).toBe("quick-checklist");
  for (const shape of ["5px", "5.0px", ".5px", ".5%", "0 / 20% 30%"])
    expect(parseManifest({ ...manifest, presentation: { ...manifest.presentation, shape } }).presentation).toMatchObject({ shape });
  for (const value of [
    { ...manifest, runtime: "future" },
    { ...manifest, author: { name: "" } },
    { ...manifest, presentation: { width: 1, height: 1 } },
    // Nothing has shipped: unknown fields and values are refused, never tolerated.
    { ...manifest, lineage: { template: "future" } },
    { ...manifest, categories: ["future-category"] },
    { ...manifest, presentation: { ...manifest.presentation, future: true } },
    { ...manifest, presentation: { ...manifest.presentation, shape: "rounded" } },
    { ...manifest, presentation: { ...manifest.presentation, shape: "5.px" } },
    { ...manifest, presentation: { ...manifest.presentation, shape: "5.%" } },
  ])
    expect(() => parseManifest(value)).toThrow();
});
test("skin paths name a file inside assets with no empty, dot or parent components", async () => {
  const manifest = JSON.parse(
    await readFile("examples/slops/quick-checklist/manifest.json", "utf8"),
  );
  const withSkin = (skin: string) => ({ ...manifest, presentation: { width: 320, height: 240, skin } });
  for (const skin of ["assets/skin.png", "assets/art/skin.v2.PNG", "assets/.hidden.png"])
    expect(parseManifest(withSkin(skin)).presentation).toMatchObject({ skin });
  for (const skin of [
    "assets/../skin.png",
    "assets/a/../skin.png",
    "assets/./skin.png",
    "assets/a/./skin.png",
    "assets//skin.png",
    "assets/a//skin.png",
    "skin.png",
    "assets/skin.png\n",
  ])
    expect(() => parseManifest(withSkin(skin))).toThrow();
});

// The native validator reads the same file (crates/hitslop-core/tests/manifest.rs), so the two
// implementations of this contract cannot drift apart. `schema` is TypeBox's verdict;
// `accept` adds the core's window-shape check.
test("TypeBox agrees with the shared manifest corpus", async () => {
  const cases: { name: string; manifest: unknown; schema: boolean }[] = JSON.parse(
    await readFile("packages/schema/tests/fixtures/manifest-cases.json", "utf8"),
  );
  expect(cases.length).toBeGreaterThan(80);
  for (const { name, manifest, schema } of cases) {
    let accepted = true;
    try {
      parseManifest(manifest);
    } catch {
      accepted = false;
    }
    expect(accepted, name).toBe(schema);
  }
});
