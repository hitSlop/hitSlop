import { test, expect } from "bun:test";
import { webkit } from "playwright";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { buildDefinition } from "../../src/cli/definition-build";
import { definitionFixture } from "./definition-fixture";

test("conventional UI entry renders emitted CSS and writes through document handles", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-definition-"));
  const browser = await webkit.launch();
  let server: ReturnType<typeof Bun.serve> | undefined;
  try {
    const source = join(root, "source"), stage = join(root, "stage");
    const {input} = await buildDefinition(source, stage, await definitionFixture(source));
    const writes: unknown[] = [];
    server = Bun.serve({hostname:"127.0.0.1", port:0, async fetch(request) {
      const path = new URL(request.url).pathname;
      if (path === "/write" && request.method === "POST") {
        writes.push(await request.json());
        return Response.json({ok:true});
      }
      const resource = input.resources.find(r => r.kind === "app" && path === `/assets/${r.key}`);
      if (resource) return new Response(Bun.file(join(stage, resource.path)), {headers:{"content-type":resource.mediaType}});
      if (path !== "/") return new Response("Missing", {status:404});
      return new Response(`<!doctype html><link rel="stylesheet" href="/assets/ui.css"><main></main><script type="module">
        import app from '/assets/ui.js';
        const ctx = {
          document: {current:{title:'Original'}, subscribe:()=>()=>{}, observe:()=>()=>{}, flush:async()=>{},
            fields:{title:{set:async value => { await fetch('/write',{method:'POST',body:JSON.stringify({title:value})}); }}}},
          capture:{isRenderer:()=>false,onPrepare:()=>()=>{},registerTarget:()=>()=>{}},
          reportError(error){throw error;}
        };
        await app.mount(ctx,document.querySelector('main')).rendered();
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
    expect(errors).toEqual([]);
  } finally {
    server?.stop(true);
    await browser.close();
    await rm(root, {recursive:true,force:true});
  }
}, 60000);
