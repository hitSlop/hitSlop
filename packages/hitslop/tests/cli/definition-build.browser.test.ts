import { test, expect } from "bun:test";
import { webkit } from "playwright";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { buildDefinition } from "../../src/cli/definition-build";
import { definitionFixture } from "./definition-fixture";

test("conventional UI entry renders emitted CSS and writes through document handles", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-definition-"));
  const browser = await webkit.launch();
  let server: ReturnType<typeof Bun.serve> | undefined;
  try {
    const source = join(root, "source"), stage = join(root, "stage");
    const options = await definitionFixture(source);
    await writeFile(join(source, "Widget.svelte"), '<script lang="ts">import doc from "./schema"; import type { WidgetProps } from "hitslop/svelte"; let {family,width,height}: WidgetProps = $props();</script><article>{doc.current.title}: {family} {width}×{height}</article>');
    const {input} = await buildDefinition(source, stage, options);
    const controller = await Bun.build({entrypoints:[new URL("../../src/shell/capture.ts", import.meta.url).pathname],target:"browser"});
    const captureSource = await controller.outputs[0]!.text();
    const writes: unknown[] = [];
    server = Bun.serve({hostname:"127.0.0.1", port:0, async fetch(request) {
      const path = new URL(request.url).pathname;
      if (path === "/capture.js") return new Response(captureSource, {headers:{"content-type":"text/javascript"}});
      if (path === "/write" && request.method === "POST") {
        writes.push(await request.json());
        return Response.json({ok:true});
      }
      const resource = input.resources.find(r => r.kind === "app" && path === `/assets/${r.key}`);
      if (resource) return new Response(Bun.file(join(stage, resource.path)), {headers:{"content-type":resource.mediaType}});
      if (path !== "/") return new Response("Missing", {status:404});
      return new Response(`<!doctype html><link rel="stylesheet" href="/assets/ui.css"><main></main><script type="module">
        import app from '/assets/ui.js';
        import {createCaptureController} from '/capture.js';
        const capture = createCaptureController();
        window.capture = capture;
        const renderer = new URL(location.href).searchParams.has('widget');
        const ctx = {
          document: {current:{title:'Original'}, subscribe:()=>()=>{}, observe:()=>()=>{}, flush:async()=>{},
            fields:{title:{set:async value => { await fetch('/write',{method:'POST',body:JSON.stringify({title:value})}); }}}},
          capture:{...capture,isRenderer:()=>renderer},
          reportError(error){throw error;}
        };
        await app.mount(ctx,document.querySelector('main')).rendered();
        window.captureReady = true;
      </script>`, {headers:{"content-type":"text/html"}});
    }});
    const page = await browser.newPage();
    const errors: string[] = [];
    page.on("pageerror", error => errors.push(error.message));
    await page.goto(`http://127.0.0.1:${server.port}/`);
    await page.getByRole("heading", {name:"Original"}).waitFor();
    expect(await page.locator("h1").evaluate(el => getComputedStyle(el).color)).toBe("rgb(20, 40, 60)");
    expect(await page.evaluate(async () => {await document.fonts.ready; return document.fonts.check('16px Fixture');})).toBe(true);
    await page.getByRole("button", {name:"Run", exact:true}).click();
    await page.locator("output").filter({hasText:"Accepted"}).waitFor();
    expect(writes).toEqual([{title:"New title"}]);
    await page.goto(`http://127.0.0.1:${server.port}/?widget`);
    await page.waitForFunction(() => !!(window as any).captureReady);
    await page.evaluate(async () => {
      await (window as any).capture.begin("widget", "widget", {family:"medium",width:344,height:164});
    });
    await expect(page.getByRole("article").textContent()).resolves.toBe("Original: medium 344×164");
    expect(await page.getByRole("button", {name:"Run",exact:true}).count()).toBe(0);
    await page.evaluate(() => (window as any).capture.restore("widget"));
    expect(await page.getByRole("article").count()).toBe(0);
    expect(errors).toEqual([]);
  } finally {
    server?.stop(true);
    await browser.close();
    await rm(root, {recursive:true,force:true});
  }
}, 60000);
