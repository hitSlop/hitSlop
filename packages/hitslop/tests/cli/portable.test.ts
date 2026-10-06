// Portable builds: a template file from any platform, with no helper, packed by the rules
// the app opens files with; and the starter project `slop init` creates.
import { test, expect } from "bun:test";
import { Database } from "bun:sqlite";
import { engine } from "../../src/cli/engine";
import { mkdtemp, cp, readFile, writeFile, rm, readdir, mkdir, symlink } from "node:fs/promises";
import { join, resolve } from "node:path";
import { overrideSlop } from "./source-fixture";
import { parseManifest, PackageFormat, RuntimeABI } from "../../src/schema/index";
import { buildTemplate } from "../../src/cli/template";

/** What the engine reads in a built file. */
const inspect = async (file: string) => JSON.parse(await engine(["inspect", file]));
/** One value from a built file, read outside the engine. */
function read<T>(file: string, sql: string): T {
  const database = new Database(file, { readonly: true });
  try {
    return database.query(sql).get() as T;
  } finally {
    database.close();
  }
}
test("init creates a buildable source and refuses to overwrite it", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  const source = join(root, "starter");
  try {
    const run = () =>
      Bun.spawn([process.execPath, "packages/hitslop/src/cli/cli.ts", "init", source], {
        stdout: "ignore",
        stderr: "ignore",
      }).exited;
    expect(await run()).toBe(0);
    expect(await run()).toBe(1);
    const metadata = JSON.parse(await readFile(join(source, "package.json"), "utf8"));
    expect(metadata.devDependencies["hitslop"]).not.toContain("__HITSLOP");
    const built = await inspect(await buildTemplate(source, undefined, join(root, "starter.slop")));
    expect(built.kind).toBe("template");
    expect(built.manifest.slug).toBeTruthy();
    // The file names the levels it needs beside its authored manifest, so an older app
    // refuses it up front.
    expect(built.packageFormat).toBe(PackageFormat);
    expect(built.runtimeABI).toBe(RuntimeABI);
    expect(() => parseManifest(built.manifest)).not.toThrow();
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 60000);

/** A minimal RGBA (colour type 6) PNG header: enough for the build's checks. */
function pngHeader(width: number, height: number, colorType = 6) {
  const bytes = Buffer.alloc(33);
  Buffer.from("89504e470d0a1a0a0000000d49484452", "hex").copy(bytes);
  bytes.writeUInt32BE(width, 16);
  bytes.writeUInt32BE(height, 20);
  bytes[24] = 8;
  bytes[25] = colorType;
  return bytes;
}

// Authoring builds on any platform: artwork is supplied or absent, and no helper runs.
test("a portable build copies supplied artwork and needs no helper", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  const previous = process.env.HITSLOP_NATIVE_CLI;
  // A helper lookup would fail on this path.
  process.env.HITSLOP_NATIVE_CLI = join(root, "missing-helper");
  try {
    const source = join(root, "source");
    await cp("examples/slops/quick-checklist", source, { recursive: true });
    const bare = await buildTemplate(source, undefined, join(root, "bare.slop"));
    expect((await inspect(bare)).artwork).toEqual([]);
    await mkdir(join(source, "artwork"));
    await writeFile(join(source, "artwork/preview.png"), pngHeader(640, 480));
    await writeFile(join(source, "artwork/icon.png"), pngHeader(512, 512));
    const output = await buildTemplate(source, undefined, join(root, "art.slop"));
    expect((await inspect(output)).artwork.map((artwork: { name: string }) => artwork.name)).toEqual(["icon", "preview"]);
    const icon = read<{ png: Uint8Array }>(output, "SELECT png FROM artwork WHERE name = 'icon'").png;
    expect(Buffer.from(icon)).toEqual(pngHeader(512, 512));
  } finally {
    if (previous === undefined) delete process.env.HITSLOP_NATIVE_CLI;
    else process.env.HITSLOP_NATIVE_CLI = previous;
    await rm(root, { recursive: true, force: true });
  }
}, 60000);

// The engine packs by the rules the app opens files with, on any platform.
test("a portable build refuses templates the app would refuse and keeps the previous output", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const fixture = join(root, "fixture");
    await cp("examples/slops/quick-checklist", fixture, { recursive: true });
    const output = await buildTemplate(fixture, undefined, join(root, "out.slop"));
    const before = await readFile(output);
    const cases: [string, (source: string) => Promise<unknown>, string][] = [
      ["artwork", async (source) => {
        await mkdir(join(source, "artwork"));
        await writeFile(join(source, "artwork/preview.png"), "not a png");
      }, "artwork/preview.png must be a valid PNG"],
      ["symlink", async (source) => {
        await mkdir(join(source, "assets"), { recursive: true });
        await symlink("/etc/hosts", join(source, "assets/hosts"));
      }, "without symbolic links"],
      ["entries", async (source) => {
        await mkdir(join(source, "assets/many"), { recursive: true });
        for (let index = 0; index < 260; index++) await writeFile(join(source, "assets/many", `${index}.txt`), "");
      }, "exceeds 256 assets"],
      ["skin", async (source) => {
        // Quick Checklist's window is 480 × 620.
        await overrideSlop(source, { presentation: '{ width: 480, height: 620, skin: "assets/skin.png" }' });
        await mkdir(join(source, "assets"), { recursive: true });
        await writeFile(join(source, "assets/skin.png"), pngHeader(481, 620));
      }, "window skin must match the window's width and height"],
    ];
    for (const [name, damage, error] of cases) {
      const source = join(root, name);
      await cp(fixture, source, { recursive: true });
      await damage(source);
      await expect(buildTemplate(source, undefined, output)).rejects.toThrow(error);
      expect(await readFile(output)).toEqual(before);
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 120000);

// The CLI validates and previews only the runtimes it knows, so it refuses a newer SDK
// before building anything.
test("build refuses a different project package and directs pinned authoring", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  try {
    const source = join(root, "source");
    await cp("examples/slops/quick-checklist", source, { recursive: true });
    const sdk = join(source, "node_modules/hitslop");
    await mkdir(sdk, { recursive: true });
    await writeFile(join(sdk, "package.json"), JSON.stringify({ name: "hitslop", version: "2.0.0", type: "module", exports: { ".": "./index.js", "./package.json": "./package.json" } }));
    await writeFile(join(sdk, "index.js"), `export * from ${JSON.stringify(resolve("packages/hitslop/src/sdk/schema.ts"))};`);
    await writeFile(join(sdk, "abi.js"), `export const RuntimeABI = ${RuntimeABI + 1};`);
    const output = join(root, "out.slop");
    await expect(buildTemplate(source, undefined, output)).rejects.toThrow("This project pins hitslop 2.0.0");
    expect(await readdir(root)).toEqual(["source"]);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 60000);
