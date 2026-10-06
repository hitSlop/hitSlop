import { test, expect } from "bun:test";
import { parseManifest } from "../../src/schema/index";
import { readFile } from "node:fs/promises";

const manifest = {
  author: { name: "hitSlop", url: "https://hitslop.com" },
  slug: "quick-checklist",
  title: "Quick Checklist",
  description: "A blush pocket utility for capturing, finishing, and filing short task lists.",
  categories: ["productivity", "personal"],
  presentation: { width: 480, height: 620 },
};
test("manifest requires attribution and valid presentation", async () => {
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
    // The schema bounds a shape; the core's shape parser owns its grammar.
    { ...manifest, presentation: { ...manifest.presentation, shape: "x".repeat(257) } },
  ])
    expect(() => parseManifest(value)).toThrow();
});
test("skin paths name a file inside assets with no empty, dot or parent components", async () => {
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
    await readFile("packages/hitslop/tests/schema/fixtures/manifest-cases.json", "utf8"),
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

// Authors fix what the message names: the field, an unknown key, or a union as a whole.
test("manifest errors name the failing field", () => {
  const message = (value: unknown) => {
    try {
      parseManifest(value);
    } catch (error) {
      return (error as Error).message;
    }
  };
  expect(message({ ...manifest, lineage: 1 })).toBe('Invalid manifest at /: unknown field "lineage"');
  expect(message({ ...manifest, title: "" })).toStartWith("Invalid manifest at /title:");
  expect(message({ ...manifest, presentation: { width: 320, height: 240, skin: "assets/x.png", resizable: true } })).toBe(
    "Invalid manifest at /presentation: matches none of its allowed forms",
  );
});
