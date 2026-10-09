import { expect, test } from "bun:test";
import { webkit } from "playwright";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { createFixture } from "../../packages/hitslop/tests/cli/dev-fixture";
import { startDev } from "../../packages/hitslop/src/cli/dev";
import { overrideSlop } from "../../packages/hitslop/tests/cli/source-fixture";

test("4000 rendered rows retain identity through command publications", async () => {
  const root = await mkdtemp(join(process.cwd(), ".large-preview-test-"));
  const browser = await webkit.launch();
  let dev: Awaited<ReturnType<typeof startDev>> | undefined;
  try {
    const source = join(root, "source");
    await createFixture(process.cwd(), source);
    await writeFile(join(source, "seed.ts"), `export default ${JSON.stringify({ title: "4000 rows", tasks:
      Array.from({ length: 4000 }, (_, i) => ({ $id: `row-${i}`, text: `Task ${i}`, done: false })) })};\n`);
    await overrideSlop(source, { initial: "seed" }, 'import seed from "./seed";');
    const file = join(source, "App.svelte");
    await writeFile(file, (await readFile(file, "utf8"))
      .replace('<script lang="ts">', `<script lang="ts">
import {onMount} from 'svelte';
const mountStart = performance.now();
onMount(() => { requestAnimationFrame(() => requestAnimationFrame(() => {
  (globalThis as any).__firstFrame = performance.now() - mountStart;
})); });`)
      .replace("</main>", '<ul data-rows>{#each doc.current.tasks as row (row.$id)}<li data-id={row.$id}>{row.text}</li>{/each}</ul></main>'));
    dev = await startDev(source);
    const page = await browser.newPage();
    await page.goto(dev.url);
    await page.frameLocator("iframe").locator("[data-rows] li").last().waitFor();
    const frame = page.frames().find(frame => frame.url().includes("/app.html"))!;
    await frame.waitForFunction(() => typeof (globalThis as any).__firstFrame === "number");
    const benchmark = process.env.HITSLOP_BENCH_PREVIEW === "1";
    const measured = await frame.evaluate(async benchmark => {
      const probe = (globalThis as any).__devProbe;
      const first = document.querySelector("[data-id='row-0']");
      const samples: number[] = [];
      for (let i = 0; i < (benchmark ? 6 : 1); i++) {
        const start = performance.now();
        await probe.addRow({ text: `Added ${i}` }); await probe.doc.flush();
        await new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
        if (i) samples.push(performance.now() - start);
      }
      samples.sort((a, b) => a - b);
      return { rows: document.querySelectorAll("[data-rows] li").length,
        firstRowPreserved: first === document.querySelector("[data-id='row-0']"),
        firstRenderedFrameMs: (globalThis as any).__firstFrame as number,
        commandThroughNextFrame: { p50Ms: samples[2], p95Ms: samples[4], samplesMs: samples },
        scope: "Svelte component creation through two animation frames; excludes Vite compilation. Command samples include flush and two frames; one warmup, five samples." };
    }, benchmark);
    expect(measured.rows).toBe(benchmark ? 4006 : 4001);
    expect(measured.firstRowPreserved).toBe(true);
    if (benchmark) console.log("Large preview", JSON.stringify(measured));
    if (benchmark && process.env.HITSLOP_TEST_EVIDENCE) await writeFile(join(process.env.HITSLOP_TEST_EVIDENCE, "large-preview-timings.json"), JSON.stringify(measured, null, 2) + "\n");
  } finally {
    await browser.close(); await dev?.close(); await rm(root, { recursive: true, force: true });
  }
}, 90000);
