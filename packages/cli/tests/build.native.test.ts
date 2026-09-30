import { test, expect } from "bun:test";
import { mkdtemp, cp, readFile, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { buildTemplate, installTemplate } from "../src/template";
import { compileAppWithVite } from "../src/vite";
import { readdir, mkdir } from "node:fs/promises";
import { copySourceFixture } from "./source-fixture";

// Without the generated theme stylesheet, a missing or late runtime theme would
// leave the first mounted view unstyled. Existing controller tests do not mount apps.
test("plain DOM adapter mounts with theme defaults and renders without Svelte or an embedded engine", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
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
    // A non-Svelte app bundles neither the Svelte compiler/runtime nor SDK internals.
    const stage = join(root, "graph");
    await mkdir(join(stage, "assets"), { recursive: true });
    const inputs = await compileAppWithVite(source, stage);
    expect(inputs.some((input) => input.endsWith("main.ts"))).toBe(true);
    for (const input of inputs) expect(input).not.toMatch(/svelte|loro-crdt|document\/src\//);
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

test("discovered capture components share the document and receive preview/export mode", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
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
        import {useDocument} from "@hitslop/document/svelte";
        import Child from "./Child.svelte";
        import schema from "./schema";
        (globalThis as any).appFields = useDocument(schema).fields;
      </script>
      <Child />
    `,
    );
    await writeFile(join(source, "Icon.svelte"), '<script>import Child from "./Child.svelte";</script><Child />');
    await writeFile(join(source, "Export.svelte"), `<script>
      import Child from "./Child.svelte";
      let {mode} = $props();
      if (mode !== "preview") throw new Error("Expected preview mode, got " + mode);
    </script><Child />`);
    const output = await buildTemplate(source, renderer, join(root, "probe.slop"));
    for (const name of ["Preview", "Icon"]) {
      const png = await readFile(join(output, `QuickLook/${name}.png`));
      expect(png.subarray(0, 8).toString("hex")).toBe("89504e470d0a1a0a");
    }
    await writeFile(join(source, "Export.svelte"), `<script>
      import Child from "./Child.svelte";
      let {mode} = $props();
      if (mode !== "export") throw new Error("Expected export mode, got " + mode);
    </script><Child />`);
    const { buildProject } = await import("../src/build");
    const exported = await buildProject(source, join(root, "export.slop"));
    const child = Bun.spawn([renderer, "export", exported, "--format", "png", "--output", join(root, "export.png")], {stdout: "ignore", stderr: "pipe"});
    const [error, code] = await Promise.all([new Response(child.stderr).text(), child.exited]);
    if (code) throw new Error(error);
    expect(code).toBe(0);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 90000);

test("native template assets are complete before replacing a registered master", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
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
        import {useDocument} from "@hitslop/document/svelte";
        import schema from "./schema";
        const document=useDocument(schema);
      </script>
      <h1>{document.current.title}</h1>
    `,
    );
    await writeFile(join(badSource, "Icon.svelte"), '<script>function broken(){throw new Error("Authored icon failed");}</script><span>{broken()}</span>');
    await expect(buildTemplate(badSource, renderer, output)).rejects.toThrow(
      "Authored icon failed",
    );
    expect(await readFile(join(output, "QuickLook/Preview.png"))).toEqual(before);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 90000);
