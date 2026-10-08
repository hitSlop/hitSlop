// Capture hooks registered by a newly mounted target must run before measurement;
// cancellation must release the page even if an author's preparation never resolves.
import { expect, test } from "bun:test";
import { webkit, type Page } from "playwright";

async function capturePage(run: (page: Page) => Promise<void>) {
  const built = await Bun.build({ entrypoints: [new URL("../../src/shell/capture.ts", import.meta.url).pathname], target: "browser" });
  if (!built.success) throw new Error("Could not bundle the capture controller");
  const source = await built.outputs[0]!.text();
  const server = Bun.serve({ hostname: "127.0.0.1", port: 0, fetch(request) {
    return new URL(request.url).pathname === "/capture.js"
      ? new Response(source, { headers: { "content-type": "text/javascript" } })
      : new Response('<script type="module">import {createCaptureController} from "/capture.js"; window.capture = createCaptureController();</script>', { headers: { "content-type": "text/html" } });
  } });
  const browser = await webkit.launch();
  try {
    const page = await browser.newPage();
    await page.goto(`http://127.0.0.1:${server.port}`);
    await page.waitForFunction(() => !!(window as any).capture);
    await run(page);
  } finally {
    await browser.close();
    server.stop(true);
  }
}

test("preparations installed while mounting a capture target settle its content", () => capturePage(async page => {
  const result = await page.evaluate(async () => {
    const capture = (window as any).capture;
    const element = document.createElement("main");
    document.body.append(element);
    let stop: (() => void) | undefined;
    capture.registerTarget("export", {
      element,
      prepare() { stop = capture.onPrepare(() => { element.textContent = "Prepared mounted content"; }); },
      restore() { stop?.(); element.textContent = ""; },
    });
    await capture.begin("mounted", "export");
    const text = element.textContent;
    await capture.restore("mounted");
    return { text, restored: element.textContent, capturing: document.documentElement.hasAttribute("data-slop-capture") };
  });
  expect(result).toEqual({ text: "Prepared mounted content", restored: "", capturing: false });
}));

test("cancelling a stalled preparation rejects promptly and restores the target", () => capturePage(async page => {
  const result = await page.evaluate(async () => {
    const capture = (window as any).capture;
    const element = document.createElement("main");
    document.body.append(element);
    let restored = 0;
    capture.registerTarget("export", { element, prepare() {}, restore() { restored++; } });
    const stop = capture.onPrepare(() => new Promise(() => {}));
    const begun = capture.begin("cancel", "export").then(() => "accepted", () => "cancelled");
    await capture.restore("cancel");
    const outcome = await Promise.race([begun, new Promise(resolve => setTimeout(() => resolve("still waiting"), 250))]);
    stop();
    await capture.begin("again", "export");
    await capture.restore("again");
    return { outcome, restored, capturing: document.documentElement.hasAttribute("data-slop-capture") };
  });
  expect(result).toEqual({ outcome: "cancelled", restored: 2, capturing: false });
}));

test("a failed target preparation restores host state and permits a later capture", () => capturePage(async page => {
  const result = await page.evaluate(async () => {
    const capture = (window as any).capture;
    const element = document.createElement("main");
    document.body.append(element);
    let fail = true, restored = 0;
    capture.registerTarget("export", {
      element,
      prepare() { if (fail) throw new Error("Authored preparation failed"); element.textContent = "Ready"; },
      restore() { restored++; element.textContent = ""; },
    });
    const error = await capture.begin("failure", "export").then(() => "accepted", (error: Error) => error.message);
    const cleaned = !document.documentElement.hasAttribute("data-slop-capture");
    fail = false;
    await capture.begin("retry", "export");
    const text = element.textContent;
    await capture.restore("retry");
    return { error, cleaned, text, restored };
  });
  expect(result).toEqual({ error: "Authored preparation failed", cleaned: true, text: "Ready", restored: 2 });
}));
