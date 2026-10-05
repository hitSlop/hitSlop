import { test, expect } from "bun:test";
import { webkit } from "playwright";
import { mkdtemp, readFile, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { startDev } from "../src/dev";
import { createFixture } from "./dev-fixture";
import { overrideSlop } from "./source-fixture";

/** A module slop.ts takes its initial values from: three tasks under `title`. */
const seed = (title: string) =>
  `export default ${JSON.stringify({ title, tasks: ["One", "Two", "Three"].map((text) => ({ text, done: false })) })};\n`;

test("HMR keeps one owner, accepted edits and row identity; metadata resets and recovers", async () => {
  const root = await mkdtemp(join(process.cwd(), ".dev-test-"));
  let dev: Awaited<ReturnType<typeof startDev>> | undefined;
  const browser = await webkit.launch();
  try {
    const source = join(root, "source");
    await createFixture(process.cwd(), source);
    // Metadata reloads when a module slop.ts imports changes.
    await writeFile(join(source, "seed.ts"), seed("A little room to think"));
    await overrideSlop(source, { initial: "seed" }, 'import seed from "./seed";');
    dev = await startDev(source);
    const page = await browser.newPage();
    page.setDefaultTimeout(15000);
    await page.goto(dev.url);
    const frame = page.frameLocator("iframe");
    await frame.getByRole("heading", { name: "Revision zero" }).waitFor();
    await frame.getByRole("button", { name: "Add row", exact: true }).click();
    await frame.locator("[data-count]").filter({ hasText: "4" }).waitFor();
    const rows = await frame.locator("[data-row-ids]").textContent();
    await frame.getByRole("textbox", { name: "Document title" }).fill("Accepted title");
    await frame.locator("[data-title]").filter({ hasText: "Accepted title" }).waitFor();
    const app = await readFile(join(source, "App.svelte"), "utf8");
    await writeFile(join(source, "App.svelte"), app.replace("Revision zero", "Hot revision"));
    await frame.getByRole("heading", { name: "Hot revision" }).waitFor();
    expect(await frame.locator("[data-row-ids]").textContent()).toBe(rows);
    expect(await frame.locator("[data-title]").textContent()).toBe("Accepted title");
    expect(await frame.locator("[data-probe]").count()).toBe(1);
    await writeFile(
      join(source, "App.svelte"),
      app.replace("rgb(20, 40, 60)", "rgb(80, 100, 120)"),
    );
    await page
      .frames()
      .find((frame) => frame.url().includes("/app.html"))!
      .waitForFunction(
        () => getComputedStyle(document.querySelector("h1")!).color === "rgb(80, 100, 120)",
      );
    await writeFile(join(source, "App.svelte"), "<script>let = ;</script>");
    await frame.locator("vite-error-overlay").waitFor();
    await writeFile(join(source, "App.svelte"), app.replace("Revision zero", "Recovered"));
    await frame.getByRole("heading", { name: "Recovered" }).waitFor();
    expect(await frame.locator("[data-count]").textContent()).toBe("4");
    await writeFile(join(source, "seed.ts"), "export default {title: 42};");
    await frame.locator("vite-error-overlay").waitFor();
    await writeFile(join(source, "seed.ts"), seed("Reset seed"));
    await frame.locator("[data-title]").filter({ hasText: "Reset seed" }).waitFor();
    expect(await frame.locator("[data-count]").textContent()).toBe("3");
    expect(await frame.locator("[data-probe]").count()).toBe(1);
    // Adding a conventional stylesheet refreshes the generated entry.
    await rm(join(source, "styles.css"));
    await page.frames().find((frame) => frame.url().includes("/app.html"))!
      .waitForFunction(() => ![...document.querySelectorAll("style")].some(style => style.dataset.viteDevId?.endsWith("/styles.css")));
    await writeFile(join(source, "styles.css"), "body { --entry-probe: ready; }");
    await page.frames().find((frame) => frame.url().includes("/app.html"))!
      .waitForFunction(() => getComputedStyle(document.body).getPropertyValue("--entry-probe").trim() === "ready");
    expect(await frame.locator("[data-probe]").count()).toBe(1);
    const response = await fetch(new URL("/@fs/etc/passwd", dev.url));
    expect(response.status).toBe(403);
    // slop.ts is build-only: the preview never serves it as an app module.
    const metadata = await fetch(new URL("/slop.ts", dev.url));
    expect(metadata.ok).toBe(false);
    expect(await metadata.text()).toContain("slop.ts is build-only");
  } finally {
    await browser.close();
    await dev?.close();
    await rm(root, { recursive: true, force: true });
  }
}, 90000);

test("cancelling startup terminates an authored metadata worker", async () => {
  const root = await mkdtemp(join(process.cwd(), ".dev-test-"));
  const source = join(root, "source");
  const controller = new AbortController();
  try {
    await createFixture(process.cwd(), source);
    const pidFile = join(root, "metadata.pid");
    await overrideSlop(source, {}, `await Bun.write(${JSON.stringify(pidFile)}, String(process.pid)); await Bun.sleep(60000);`);
    const opening = startDev(source, 0, controller.signal);
    const result = opening.then(
      (server) => {
        void server.close();
        return "unexpectedly opened";
      },
      () => "cancelled",
    );
    let pid: number | undefined;
    const deadline = Date.now() + 10000;
    while (!pid && Date.now() < deadline) {
      pid = await readFile(pidFile, "utf8").then(Number, () => undefined);
      if (!pid) await Bun.sleep(20);
    }
    expect(pid).toBeDefined();
    controller.abort();
    expect(await result).toBe("cancelled");
    expect(() => process.kill(pid!, 0)).toThrow();
  } finally {
    controller.abort();
    await rm(root, { recursive: true, force: true });
  }
}, 20000);
