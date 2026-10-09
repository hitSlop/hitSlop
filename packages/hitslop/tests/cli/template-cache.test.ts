import { expect, test } from "bun:test";
import { copyFile, mkdir, mkdtemp, readFile, readdir, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import {
  sharedTemplatePaths,
  inputs,
  TemplateCache,
  validateTemplate,
} from "../../../../scripts/templates/cache";

import { writeTemplate } from "./template-fixture";

test("template cache reuses matching artifacts and rebuilds changed or damaged entries", async () => {
  const root = await mkdtemp(join(tmpdir(), "hitslop-template-cache-"));
  try {
    const source = join(root, "source"),
      output = join(root, "output.slop"),
      directory = join(root, "cache");
    await mkdir(source);
    await writeFile(join(source, "main.ts"), "first");
    let builds = 0;
    const build = async () => {
      builds++;
      await writeTemplate(output);
    };
    const cache = new TemplateCache(directory, { "@swift": "a" });
    const run = async (current = cache, artwork = true) => {
      await rm(output, { force: true });
      return current.build(source, "fixture-one", output, build, artwork);
    };
    expect(await run()).toBe("built");
    expect(await run()).toBe("hit");
    expect(builds).toBe(1);
    expect(await run(cache, false)).toBe("built");
    expect(await run(cache, false)).toBe("hit");
    // A portable cache entry cannot satisfy a request for rendered artwork.
    expect(await run()).toBe("built");
    await mkdir(join(source, "dist"));
    await writeFile(join(source, "dist/ignored.js"), "generated");
    expect(await run()).toBe("hit");
    await writeFile(join(source, "main.ts"), "changed");
    expect(await run()).toBe("built");
    expect(cache.misses.get("fixture-one")).toEqual(["template main.ts"]);
    // Asset directories named like build outputs are still copied into the package.
    await mkdir(join(source, "assets/dist"), { recursive: true });
    await writeFile(join(source, "assets/dist/theme.css"), "changed asset");
    expect(await run()).toBe("built");
    const toolchain = new TemplateCache(directory, { "@swift": "b" });
    expect(await run(toolchain)).toBe("built");
    expect(toolchain.misses.get("fixture-one")).toEqual(["shared @swift"]);
    expect(await run()).toBe("built");
    const cached = join(directory, "fixture-one/template.slop");
    const bytes = await readFile(cached);
    bytes[bytes.length - 1] ^= 1;
    await writeFile(cached, bytes);
    expect(await run()).toBe("built");
    await writeFile(join(directory, "fixture-one/entry.json"), "interrupted");
    expect(await run()).toBe("built");
    await mkdir(join(directory, "removed-template"));
    await writeFile(
      join(directory, "removed-template/entry.json"),
      JSON.stringify({ key: "old", checksum: "old" }),
    );
    await cache.prune(["fixture-one"]);
    expect(await readdir(directory)).toEqual(["fixture-one"]);
    expect(await Bun.file(join(directory, "fixture-one/entry.json")).exists()).toBe(true);
    await mkdir(join(directory, "unrelated"));
    await cache.prune(["fixture-one"]);
    expect(await readdir(directory)).toContain("unrelated");
    await cache.prune([]);
    expect(await run()).toBe("built");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("changing one template preserves another template's cache entry", async () => {
  const root = await mkdtemp(join(tmpdir(), "hitslop-template-cache-"));
  try {
    const cache = new TemplateCache(join(root, "cache"), {});
    const builds: string[] = [];
    for (const slug of ["fixture-one", "fixture-two"]) {
      await mkdir(join(root, slug));
      await writeFile(join(root, slug, "main.ts"), "original");
    }
    const run = async (slug: string) => {
      const output = join(root, slug + ".slop");
      await rm(output, { force: true });
      return cache.build(join(root, slug), slug, output, async () => {
        builds.push(slug);
        await writeTemplate(output, slug);
      });
    };
    await run("fixture-one");
    await run("fixture-two");
    await writeFile(join(root, "fixture-one/main.ts"), "changed");
    expect(await run("fixture-one")).toBe("built");
    expect(await run("fixture-two")).toBe("hit");
    expect(builds).toEqual(["fixture-one", "fixture-two", "fixture-one"]);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("failed builds never publish cache entries; cached templates are template files", async () => {
  const root = await mkdtemp(join(tmpdir(), "hitslop-template-cache-"));
  try {
    const source = join(root, "source"),
      output = join(root, "output.slop"),
      directory = join(root, "cache");
    await mkdir(source);
    const cache = new TemplateCache(directory, {});
    await expect(
      cache.build(source, "fixture-one", output, async () => {
        throw new Error("render failed");
      }),
    ).rejects.toThrow("render failed");
    expect(await Bun.file(join(directory, "fixture-one/entry.json")).exists()).toBe(false);
    await writeTemplate(output);
    await validateTemplate(output, "fixture-one");
    await expect(validateTemplate(output, "fixture-two")).rejects.toThrow("Template slug mismatch");
    const link = join(root, "link.slop");
    await symlink(output, link);
    await expect(validateTemplate(link, "fixture-one")).rejects.toThrow("Invalid template file");
    const damaged = join(root, "damaged.slop");
    await copyFile(output, damaged);
    await writeFile(damaged, (await readFile(damaged)).subarray(0, 512));
    await expect(validateTemplate(damaged, "fixture-one")).rejects.toThrow();
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test.each(["crates/hitslop-core/src/file/mod.rs", "bun.lock"])(
  "changing shared input %s rebuilds every cached template",
  async (changed) => {
    const root = await mkdtemp(join(tmpdir(), "hitslop-shared-input-"));
    try {
      // Minimal repository: input discovery still uses the production compiler closure.
      for (const entry of ["template.ts", "stage-worker.ts"]) {
        const path = join(root, "packages/hitslop/src/cli", entry);
        await mkdir(dirname(path), { recursive: true });
        await writeFile(path, "export {};");
      }
      await mkdir(join(root, "examples/slops"), { recursive: true });
      await mkdir(join(root, "crates/hitslop-core/src/file"), { recursive: true });
      await writeFile(join(root, "crates/hitslop-core/src/file/mod.rs"), "// original format");
      await writeFile(join(root, "bun.lock"), "{}");
      const paths = await sharedTemplatePaths(root, []);
      for (const path of paths) {
        if (await Bun.file(join(root, path)).exists()) continue;
        if (["crates/hitslop-core/src", "crates/slop-engine"].includes(path)) {
          await mkdir(join(root, path), { recursive: true });
          continue;
        }
        await mkdir(dirname(join(root, path)), { recursive: true });
        await writeFile(join(root, path), "");
      }
      const slugs = ["fixture-one", "fixture-two"];
      for (const slug of slugs) await mkdir(join(root, slug));
      const run = async (expected: "built" | "hit", cause?: string) => {
        const cache = new TemplateCache(join(root, "cache"), await inputs(root, paths));
        for (const slug of slugs) {
          const output = join(root, `${slug}.slop`);
          await rm(output, { force: true });
          expect(await cache.build(join(root, slug), slug, output, () => writeTemplate(output, slug))).toBe(expected);
          if (cause) expect(cache.misses.get(slug)).toEqual([`shared ${cause}`]);
        }
      };
      await run("built");
      await run("hit");
      await writeFile(join(root, "packages/hitslop/src/cli/app.ts"), "changed CLI help");
      await run("hit");
      await writeFile(
        join(root, changed),
        changed === "bun.lock" ? '{"lockfileVersion":2}' : "// a new format",
      );
      await run("built", changed);
      await run("hit");
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  },
);
