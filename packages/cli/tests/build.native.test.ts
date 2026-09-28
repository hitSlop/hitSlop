import { test, expect } from "bun:test";
import { runtimePlugin } from "../src/build";
import { build as esbuild } from "esbuild";
import { mkdtemp, cp, readFile, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { buildTemplate, installTemplate } from "../src/template";
import { readdir, mkdir } from "node:fs/promises";
import { copySourceFixture } from "./source-fixture";

// Without the generated theme stylesheet, a missing or late runtime theme would
// leave the first mounted view unstyled. Existing controller tests do not mount apps.
test("plain DOM adapter mounts with theme defaults and renders without Svelte or an embedded engine", async () => {
  const root = await mkdtemp(join(process.cwd(), ".v1-build-test-"));
  try {
    const source = join(root, "source");
    await cp("packages/cli/templates/checklist", source, { recursive: true });
    await writeFile(
      join(source, "theme.ts"),
      'import {defineTheme} from "@hitslop/document/theme"; export default defineTheme({accent: "#123456"});',
    );
    await writeFile(
      join(source, "main.ts"),
      `
      import "./styles.css";
      import type {SlopApp} from "@hitslop/document/abi";
      export default {
        mount(ctx, target) {
          const doc = ctx.document;
          const root = document.createElement("main");
          root.dataset.hitslopRoot = "";
          root.style.backgroundColor = "var(--slop-accent)";
          const render = () => { root.textContent = String(doc.current.title); };
          render();
          target.append(root);
          if (getComputedStyle(root).backgroundColor !== "rgb(18, 52, 86)")
            throw new Error("Theme defaults were not applied before mount");
          const stop = doc.subscribe(render);
          return { rendered() {}, unmount() { stop(); root.remove(); } };
        },
      } satisfies SlopApp;
    `,
    );
    // Only the host-runtime plugin: a non-Svelte app needs no Svelte compiler or runtime.
    const bundle = await esbuild({
      entryPoints: [join(source, "main.ts")],
      bundle: true,
      write: false,
      outdir: join(root, "graph"),
      format: "esm",
      platform: "browser",
      metafile: true,
      plugins: [runtimePlugin],
    });
    for (const input of Object.keys(bundle.metafile!.inputs))
      expect(input).not.toMatch(/svelte|loro-crdt|document\/src\//);
    const renderer = join(
      process.cwd(),
      "apps/apple/Packages/HitSlopApple/.build/debug/hitslop-native",
    );
    const output = await buildTemplate(source, renderer, join(root, "plain.slop"));
    const png = await readFile(join(output, "QuickLook/Preview.png"));
    expect(png.subarray(0, 8).toString("hex")).toBe("89504e470d0a1a0a");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 90000);

test("Slop and child components share the host document", async () => {
  const root = await mkdtemp(join(process.cwd(), ".v1-build-test-"));
  const source = join(root, "source");
  const renderer = join(
    process.cwd(),
    "apps/apple/Packages/HitSlopApple/.build/debug/hitslop-native",
  );
  try {
    await copySourceFixture("examples/slops/quick-checklist", source);
    await writeFile(
      join(source, "Child.svelte"),
      `
      <script lang="ts">
        import {useDocument} from "@hitslop/document/svelte";
        import schema from "./schema";
        const doc = useDocument(schema);
        if (doc.fields !== (globalThis as any).appFields) throw new Error("Child received another document");
      </script>
      <h1>{doc.current.title}</h1>
    `,
    );
    await writeFile(
      join(source, "App.svelte"),
      `
      <script lang="ts">
        import {mount} from "svelte";
        import {Slop, useDocument} from "@hitslop/document/svelte";
        import Child from "./Child.svelte";
        import schema from "./schema";
        (globalThis as any).appFields = useDocument(schema).fields;
        let missingHost = "";
        try { mount(Slop, {target: document.createElement("div"), props: {children: () => {}}, context: new Map()}); }
        catch (error) { missingHost = String(error); }
        if (!missingHost.includes("Slop requires a host document"))
          throw new Error("Missing host document was not diagnosed: " + missingHost);
      </script>
      <Slop>
        <Child />
        {#snippet exportView()}<Child />{/snippet}
        {#snippet icon()}<Child />{/snippet}
      </Slop>
    `,
    );
    const output = await buildTemplate(source, renderer, join(root, "probe.slop"));
    for (const name of ["Preview", "Icon"]) {
      const png = await readFile(join(output, `QuickLook/${name}.png`));
      expect(png.subarray(0, 8).toString("hex")).toBe("89504e470d0a1a0a");
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 90000);

test("native template assets are complete before replacing a registered master", async () => {
  const root = await mkdtemp(join(process.cwd(), ".v1-build-test-"));
  const renderer = join(
    process.cwd(),
    "apps/apple/Packages/HitSlopApple/.build/debug/hitslop-native",
  );
  try {
    const output = await buildTemplate(
      "examples/slops/quick-checklist",
      renderer,
      join(root, "Checklist.slop"),
    );
    for (const name of ["Preview", "Icon"]) {
      const png = await readFile(join(output, `QuickLook/${name}.png`));
      expect(png.subarray(0, 8).toString("hex")).toBe("89504e470d0a1a0a");
      if (name === "Icon") {
        expect(png.readUInt32BE(16)).toBe(512);
        expect(png.readUInt32BE(20)).toBe(512);
      }
    }
    expect(await readdir(output)).not.toContain("state");
    expect(await readdir(output)).not.toContain("Icon\r");
    const templates = join(root, "templates");
    await mkdir(templates);
    const master = join(templates, "quick-checklist.slop");
    await installTemplate(output, master);
    await installTemplate(output, master);
    expect((await readdir(join(root, "template-backups"))).length).toBe(1);
    const before = await readFile(join(output, "QuickLook/Preview.png"));
    await expect(
      buildTemplate("examples/slops/quick-checklist", "/usr/bin/false", output),
    ).rejects.toThrow();
    expect(await readFile(join(output, "QuickLook/Preview.png"))).toEqual(before);
    await mkdir(join(master, "state"));
    await expect(installTemplate(output, master)).rejects.toThrow("writable document state");
    const badSource = join(root, "bad-capture-source");
    await copySourceFixture("examples/slops/quick-checklist", badSource);
    await writeFile(
      join(badSource, "App.svelte"),
      `
      <script lang="ts">
        import {Slop,useDocument} from "@hitslop/document/svelte";
        import schema from "./schema";
        const document=useDocument(schema);
        function broken(){throw new Error("Authored icon failed");}
      </script>
      <Slop>
        <h1>{document.current.title}</h1>
        {#snippet icon()}<span>{broken()}</span>{/snippet}
      </Slop>
    `,
    );
    await expect(buildTemplate(badSource, renderer, output)).rejects.toThrow(
      "Authored icon failed",
    );
    expect(await readFile(join(output, "QuickLook/Preview.png"))).toEqual(before);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 90000);
