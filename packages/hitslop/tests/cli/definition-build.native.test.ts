import { test, expect } from "bun:test";
import { webkit } from "playwright";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { buildDefinition } from "../../src/cli/definition-build";
import { findEngine } from "../../src/cli/engine";
import { exec } from "../../src/cli/process";
import { definitionFixture } from "./definition-fixture";

test("explicit entry renders its emitted CSS and dispatches a stub to the restricted runner", async () => {
  const root = await mkdtemp(join(process.cwd(), ".build-test-definition-"));
  const browser = await webkit.launch();
  let server: ReturnType<typeof Bun.serve> | undefined;
  try {
    const source = join(root, "source"), stage = join(root, "stage");
    const {input} = await buildDefinition(source, stage, await definitionFixture(source));
    const bundle = await readFile(join(stage, "resources/commands.js"), "utf8");
    const engine = await findEngine();
    const calls: unknown[] = [], outcomes: any[] = [];
    server = Bun.serve({hostname:"127.0.0.1", port:0, async fetch(request) {
      const path = new URL(request.url).pathname;
      if (path === "/command" && request.method === "POST") {
        const call = await request.json();
        calls.push(call);
        const reply = await exec([engine, "--evaluate-command"], {cwd:"/", env:{}, timeout:4000,
          stdin:JSON.stringify({runtimeABI:1,bundle, request:JSON.stringify({...call,
            descriptor:input.declaration.document, value:input.declaration.initial, now:0, seed:[1,2,3,4]})})});
        const outcome = JSON.parse(reply.stdout);
        outcomes.push(outcome);
        return Response.json(outcome);
      }
      const resource = input.resources.find(r => r.kind === "app" && path === `/assets/${r.key}`);
      if (resource) return new Response(Bun.file(join(stage, resource.path)), {headers:{"content-type":resource.mediaType}});
      if (path !== "/") return new Response("Missing", {status:404});
      return new Response(`<!doctype html><link rel="stylesheet" href="/assets/ui.css"><main></main><script type="module">
        import app from '/assets/ui.js';
        const ctx = {
          document: {current:{title:'Original'}, subscribe:()=>()=>{}, observe:()=>()=>{}, flush:async()=>{},
            async runCommand(name,args) {
              const outcome = await fetch('/command',{method:'POST',body:JSON.stringify({name,args})}).then(r=>r.json());
              if (!outcome.ok) throw new Error(outcome.error);
              return outcome.result;
            }},
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
    await page.locator("output").filter({hasText:"BODY_ONLY_CHANGE"}).waitFor();
    expect(calls).toEqual([{name:"rename",args:{title:"New title"}}]);
    expect(outcomes).toEqual([{ok:true,result:"BODY_ONLY_CHANGE",intents:[{type:"set",path:["title"],value:"New title"}]}]);
    expect(errors).toEqual([]);
  } finally {
    server?.stop(true);
    await browser.close();
    await rm(root, {recursive:true,force:true});
  }
}, 60000);
