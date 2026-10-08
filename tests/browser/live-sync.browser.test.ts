import { expect, test } from "bun:test";
import { webkit, type Frame } from "playwright";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { startLiveSync } from "../../scripts/dev/live-sync";
import { createFixture } from "../../packages/hitslop/tests/cli/dev-fixture";
import { execute, request } from "../../packages/hitslop/src/cli/engine";

// Runs when `bun run verify browser live-sync` built the opt-in engine and `slop-room`.
const engine = process.env.HITSLOP_DEV_SYNC_ENGINE;
test.skipIf(!engine)("two live views and CLI share durable authority edits; snapshots, duplicate delivery and disconnects stay safe", async () => {
  const root = await mkdtemp(join(process.cwd(), ".live-sync-test-"));
  const browser = await webkit.launch();
  let harness: Awaited<ReturnType<typeof startLiveSync>> | undefined;
  try {
    const source = join(root, "source");
    await createFixture(process.cwd(), source);
    const schema = join(source, "schema.ts");
    await writeFile(schema, (await readFile(schema, "utf8")).replace("title: s.text", "hits: s.counter(),\n  title: s.text"));
    const commands = join(source, "commands.ts");
    await writeFile(commands, (await readFile(commands, "utf8")) + `
export const bump = doc.command({description: "Add to the shared count", args: {}, run({tx}) { tx.fields.hits.increment(); }});
`);
    const app = join(source, "App.svelte");
    await writeFile(app, (await readFile(app, "utf8"))
      .replace("import {addRow}", "import {addRow, bump}")
      .replace("{doc, addRow, attachments}", "{doc, addRow, bump, attachments}")
      .replace("<main data-probe>", '<main data-probe><input aria-label="Local draft" /><output data-hits>{doc.current.hits}</output>'));
    harness = await startLiveSync(source, { engine, directory: join(root, "room") });
    const pages = await Promise.all([browser.newPage(), browser.newPage()]);
    await Promise.all(pages.map((page, index) => page.goto(index ? harness!.urls.b : harness!.urls.a)));
    const frames: Frame[] = [];
    for (const page of pages) {
      page.setDefaultTimeout(15000);
      await page.frameLocator("iframe").locator("[data-hits]").waitFor();
      frames.push(page.frames().find(frame => frame.url().includes("/app.html"))!);
    }
    const [a, b] = frames as [Frame, Frame];
    const count = async (frame: Frame, value: number) => frame.waitForFunction(expected =>
      document.querySelector("[data-hits]")?.textContent === String(expected), value);
    const bump = (frame: Frame) => frame.evaluate(async () => {
      const probe = (globalThis as any).__devProbe;
      await probe.bump({}); await probe.doc.flush();
    });
    const get = (path: string) => execute({ method: "get", documentPath: path }, { binary: engine });
    const initial = await get(harness.paths.authority);
    await a.getByRole("button", { name: "Add row", exact: true }).click();
    await b.waitForFunction(() => document.querySelector("[data-count]")?.textContent === "4");
    expect(await a.locator("[data-row-ids]").textContent()).toBe(await b.locator("[data-row-ids]").textContent());
    // The unchanged command path is serialized by the authority. Use alternating
    // bursts: the existing one-command-at-a-time rule can refuse concurrent commands.
    await bump(a); await bump(b);
    await execute({ method: "batch", documentPath: harness.paths.b,
      batch: { intents: [{ type: "increment", path: ["hits"], by: 3 }] } }, { binary: engine });
    await Promise.all(frames.map(frame => count(frame, 5)));
    await harness.control("duplicateNext", "b");
    await bump(a);
    await Promise.all(frames.map(frame => count(frame, 6)));
    await harness.control("pauseDelivery", "b");
    await bump(a);
    await count(a, 7);
    expect(await b.locator("[data-hits]").textContent()).toBe("6");
    await harness.control("snapshot", "b");
    await count(b, 7);
    await harness.control("resumeDelivery", "b");
    await bump(a);
    await count(b, 8);
    await bump(b);
    await Promise.all(frames.map(frame => count(frame, 9)));
    const benchmark = process.env.HITSLOP_BENCH_SYNC === "1";
    const sampleCount = benchmark ? 11 : 1;
    const timings = await a.evaluate(async sampleCount => {
      const probe = (globalThis as any).__devProbe;
      const samples = { commands: [] as number[], text: [] as number[] };
      const input = document.querySelector<HTMLInputElement>('input[aria-label="Document title"]')!;
      for (let i = 0; i < sampleCount; i++) {
        let start = performance.now();
        await probe.bump({}); await probe.doc.flush();
        if (i) samples.commands.push(performance.now() - start);
        input.focus(); input.value = `Shared draft ${i} 😀`;
        start = performance.now(); input.dispatchEvent(new Event("input"));
        await probe.doc.flush();
        if (i) samples.text.push(performance.now() - start);
      }
      return samples;
    }, sampleCount);
    const afterText = 9 + sampleCount;
    await count(b, afterText);
    await b.waitForFunction(text => document.querySelector("[data-title]")?.textContent === text, `Shared draft ${sampleCount - 1} 😀`);
    await Promise.all(Array.from({ length: 8 }, (_, index) => execute({ method: "batch",
      documentPath: index % 2 ? harness!.paths.a : harness!.paths.b,
      batch: { intents: [{ type: "increment", path: ["hits"], by: 1 }] } }, { binary: engine })));
    await Promise.all(frames.map(frame => count(frame, afterText + 8)));
    // A missing broadcast is detected from the next update's vector and requests a
    // checkpoint automatically; the mutation itself is never replayed.
    await harness.control("skipNext", "b");
    await bump(a); await count(a, afterText + 9);
    expect(await b.locator("[data-hits]").textContent()).toBe(String(afterText + 8));
    await bump(a);
    await count(b, afterText + 10);
    const concurrentCommands = await Promise.all([harness.paths.a, harness.paths.b].map(documentPath =>
      request({ method: "call", documentPath, command: "bump", args: {} }, { binary: engine })));
    const acceptedCommands = concurrentCommands.filter(reply => reply.ok).length;
    expect(acceptedCommands).toBeGreaterThan(0);
    for (const reply of concurrentCommands) if (!reply.ok) expect(reply.code).toBe("rejected");
    await Promise.all(frames.map(frame => count(frame, afterText + 10 + acceptedCommands)));
    await bump(a);
    await count(b, afterText + 11 + acceptedCommands);
    await Promise.all(frames.map((frame, index) => frame.evaluate(async suffix => {
      const probe = (globalThis as any).__devProbe;
      const input = document.querySelector<HTMLInputElement>('input[aria-label="Document title"]')!;
      input.focus(); input.value = probe.doc.current.title + suffix;
      input.dispatchEvent(new Event("input")); await probe.doc.flush();
    }, index ? " B" : " A")));
    await Promise.all(frames.map(frame => frame.waitForFunction(() => {
      const title = document.querySelector("[data-title]")?.textContent ?? "";
      return title.includes(" A") && title.includes(" B");
    })));
    await execute({ method: "batch", documentPath: harness.paths.a,
      batch: { intents: [{ type: "setTheme", values: { accent: "#336699" } }] } }, { binary: engine });
    await b.waitForFunction(() => getComputedStyle(document.documentElement).getPropertyValue("--slop-accent").trim() === "#336699");
    const summary = (samples: number[]) => {
      const sorted = [...samples].sort((a, b) => a - b);
      return { p50Ms: sorted[Math.floor(sorted.length / 2)], p95Ms: sorted[Math.ceil(sorted.length * .95) - 1], samplesMs: samples };
    };
    const measurements = { profile: "release", warmups: 1, commands: summary(timings.commands), text: summary(timings.text),
      concurrentCommands: { attempts: concurrentCommands.length, accepted: acceptedCommands, refused: concurrentCommands.length - acceptedCommands } };
    if (benchmark) console.log("Live sync round trips", JSON.stringify(measurements));
    const evidence = process.env.HITSLOP_TEST_EVIDENCE;
    if (benchmark && evidence) {
      await writeFile(join(evidence, "live-sync-timings.json"), JSON.stringify(measurements, null, 2) + "\n");
      await Promise.all(pages.map((page, index) => page.screenshot({ path: join(evidence, `live-sync-${index ? "b" : "a"}.png`) })));
    }
    // Normal CLI discovery reaches each live replica owner and observes the same
    // accepted result. Acknowledgement follows the SQLite save on both sides.
    const states = await Promise.all(Object.values(harness.paths).map(get));
    expect(states[0]!.state.version).not.toBe(initial.state.version);
    for (const state of states) {
      expect(state.state.value).toEqual(states[0]!.state.value);
      expect(state.state.version).toBe(states[0]!.state.version);
    }
    await b.getByRole("textbox", { name: "Local draft" }).fill("Keep this draft");
    await harness.control("disconnect", "b");
    const refused = await request({ method: "batch", documentPath: harness.paths.b,
      batch: { intents: [{ type: "increment", path: ["hits"], by: 100 }] } }, { binary: engine });
    expect(refused.ok).toBe(false);
    expect(await b.getByRole("textbox", { name: "Local draft" }).inputValue()).toBe("Keep this draft");
    const disconnectedText = await b.evaluate(async () => {
      const probe = (globalThis as any).__devProbe;
      const input = document.querySelector<HTMLInputElement>('input[aria-label="Document title"]')!;
      input.focus(); input.value = "Keep this disconnected text draft";
      input.dispatchEvent(new Event("input"));
      await probe.doc.flush().catch(() => {});
      return { draft: input.value, accepted: probe.doc.current.title };
    });
    expect(disconnectedText.draft).toBe("Keep this disconnected text draft");
    expect(disconnectedText.accepted).toBe((states[0]!.state.value as { title: string }).title);
    expect((await get(harness.paths.b)).state.value).toEqual(states[0]!.state.value);
    const durable = states[0]!.state;
    await browser.close();
    await harness.close(); harness = undefined;
    // Reopen independently through the normal owner after all locks are released.
    for (const name of ["authority", "replica-a", "replica-b"]) {
      const saved = await get(join(root, "room", `${name}.slop`));
      expect(saved.state.value).toEqual(durable.value);
      expect(saved.state.version).toBe(durable.version);
    }
  } finally {
    await browser.close();
    await harness?.close();
    await rm(root, { recursive: true, force: true });
  }
}, 180000);
