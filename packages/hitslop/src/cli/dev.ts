import { createServer, type Plugin, type ViteDevServer } from "vite";
import { readFile, mkdtemp, rm, realpath, mkdir } from "node:fs/promises";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { createHash } from "node:crypto";
import { appConfig } from "./vite";
import { previewFrame } from "./preview";
import { previewResizeScript } from "./preview-resize";
import { cliRoot, shellDirectory } from "./paths";
import { stageWorker } from "./build";
import { execute } from "./engine";
import { NativeDevHosts } from "./native-dev";
import { nativeDevClient } from "./native-dev-client";
import type { BuildInput } from "../wire/app.generated";
import { appContentSecurityPolicy } from "../schema/policy";
import { AttachmentIdPattern } from "../schema/constants";

/** Single byte ranges; malformed/multiple ranges are ignored, an empty range is 416. */
export function byteRange(header: string | undefined, size: number): [number, number] | "whole" | "unsatisfiable" {
  if (!header?.startsWith("bytes=") || header.includes(",")) return "whole";
  const parts = /^bytes=(\d*)-(\d*)$/.exec(header.trim());
  if (!parts || (!parts[1] && !parts[2])) return "whole";
  const first = parts[1] ? Number(parts[1]) : undefined, last = parts[2] ? Number(parts[2]) : undefined;
  if (first !== undefined) {
    if (first >= size || (last !== undefined && last < first)) return "unsatisfiable";
    return [first, Math.min(size, last === undefined ? size : last + 1)];
  }
  if (!last || !size) return "unsatisfiable";
  return [Math.max(0,size-last),size];
}

export async function startDev(source: string, port = 0, signal?: AbortSignal) {
  source = await realpath(resolve(source));
  const temporary = await mkdtemp(join(tmpdir(), "hitslop-preview-"));
  let stage = "", template = "", input: BuildInput;
  const hosts = new NativeDevHosts(temporary, () => template);
  let dependencies = new Set([join(source,"slop.ts")]);
  let closed = false, failed = false;
  let server: ViteDevServer | undefined, child: {kill():void} | undefined;
  let rebuilding: Promise<void> | undefined, wanted = false;
  const build = async () => {
    await hosts.invalidate();
    const next = await mkdtemp(join(temporary,"build-"));
    try {
      const worker = stageWorker(source, join(next,"stage"), "Definition build failed");
      child = worker;
      const built = await worker.done;
      await execute({method:"pack",app:built.input,stage:join(next,"stage"),file:join(next,"preview.slop")});
      const old = stage;
      input = built.input; stage = next; template = join(next,"preview.slop");
      dependencies = new Set(built.dependencies.definition.filter(path => !/\.(?:svelte|css)$/.test(path)));
      server?.watcher.add([...dependencies]);
      failed = false; hosts.activate();
      if (old) await rm(old,{recursive:true,force:true});
    } catch (error) { failed = true; await rm(next,{recursive:true,force:true}); throw error; }
    finally { child = undefined; }
  };
  const rebuild = () => {
    wanted = true;
    rebuilding ??= (async () => {
      while (wanted && !closed) {
        wanted = false;
        try { await build(); if (!closed) server?.ws.send({type:"full-reload",path:"*"}); }
        catch (error) { if (!closed) server?.ws.send({type:"error",err:{message:String(error),stack:"",plugin:"hitslop-definition"}}); }
      }
      rebuilding = undefined;
    })();
  };
  let closing: Promise<void> | undefined;
  const close = () => closing ??= (async () => {
    closed = true; signal?.removeEventListener("abort",onAbort); child?.kill();
    await hosts.close(); await server?.close(); await rebuilding;
    await rm(temporary,{recursive:true,force:true});
  })();
  const onAbort = () => { void close().catch(console.error); };
  signal?.addEventListener("abort",onAbort,{once:true});
  try {
    signal?.throwIfAborted(); await build(); signal?.throwIfAborted();
    const host: Plugin = {
      name:"hitslop-preview-host",
      configureServer(current) {
        hosts.attach(current);
        current.watcher.add([...dependencies]);
        current.watcher.on("all",(event,file) => {
          if (!closed && ["add","unlink","change"].includes(event) && (dependencies.has(file) || (failed && /\.[cm]?[jt]s$/.test(file)))) rebuild();
        });
        current.middlewares.use(async (req,res,next) => {
          let path = "";
          try {
            path = decodeURIComponent(new URL(req.url!,"http://localhost").pathname);
            const origin = new URL(current.resolvedUrls!.local[0]!).origin;
            let content: string | Buffer | undefined, type = "text/javascript";
            if (path === "/") { content = previewFrame({title:input.declaration.metadata.title,window:input.declaration.window}); type="text/html"; }
            else if (path === "/app.html") {
              type="text/html";
              content=`<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head><body><script type="module" src="/__preview__/native.js?token=${hosts.token}"></script></body></html>`;
            }
            else if (path === "/__preview__/resize.js") content=previewResizeScript;
            else if (path === "/__preview__/native.js") content=nativeDevClient;
            else if (path.startsWith("/__shell__/")) {
              const file=resolve(shellDirectory,path.slice(11));
              if (!file.startsWith(shellDirectory+"/") || !file.endsWith(".js")) throw new Error("Resource not exposed");
              content=await readFile(file);
            }
            else if (path.startsWith("/assets/")) {
              const key=path.slice(8), resource=input.resources.find(r=>r.kind==="app" && r.key===key);
              if (!resource) { res.statusCode=404;res.end();return; }
              content=await readFile(join(stage,"stage",resource.path));type=resource.mediaType;
            }
            else if (path.startsWith("/attachments/")) {
              const [, ,token,id,...rest]=path.split("/");
              if (rest.length || !token || !new RegExp(AttachmentIdPattern).test(id??"")) throw new Error("Invalid attachment URL");
              const metadata=await hosts.resource(token,id!,0,0);
              if (!metadata.info || metadata.error) { res.statusCode=404;res.end();return; }
              const size=metadata.info.size, range=byteRange(req.headers.range,size);
              res.setHeader("Accept-Ranges","bytes");
              res.setHeader("Content-Type",metadata.info.mimeType);
              res.setHeader("Content-Security-Policy","sandbox");
              res.setHeader("X-Content-Type-Options","nosniff");
              res.setHeader("Cache-Control","no-store");
              if (range==="unsatisfiable") {res.statusCode=416;res.setHeader("Content-Range",`bytes */${size}`);res.end();return;}
              const [start,end]=range==="whole"?[0,size]:range;
              if (range!=="whole") {res.statusCode=206;res.setHeader("Content-Range",`bytes ${start}-${end-1}/${size}`);}
              res.setHeader("Content-Length",end-start);
              if (req.method==="HEAD") {res.end();return;}
              const data=await hosts.resource(token,id!,start,end-start);
              if (data.error || !data.bytes) throw new Error(data.error??"Missing attachment");
              res.end(Buffer.from(data.bytes,"base64"));return;
            }
            if (content===undefined) {next();return;}
            res.setHeader("Content-Type",type);res.setHeader("Cache-Control","no-store");res.setHeader("X-Content-Type-Options","nosniff");
            res.setHeader("Content-Security-Policy",path==="/" ? `default-src 'none'; script-src ${origin}/__preview__/resize.js; style-src 'unsafe-inline'; img-src 'self'; frame-src 'self'` : appContentSecurityPolicy("browser",origin));
            res.end(content);
          } catch(error) {
            if (path.startsWith("/attachments/")) { res.statusCode=404; res.end(); }
            else next(error as Error);
          }
        });
      },
      handleHotUpdate(ctx) {if (dependencies.has(ctx.file)) return [];},
    };
    const config=await appConfig(source);
    server=await createServer({...config,plugins:[host,...(config.plugins??[])],appType:"custom",cacheDir:join(tmpdir(),"hitslop-vite-cache",createHash("sha256").update(cliRoot+source).digest("hex")),server:{host:"127.0.0.1",port,strictPort:port!==0,fs:{strict:true,allow:[source,cliRoot]}}});
    signal?.throwIfAborted();await server.listen();signal?.throwIfAborted();
    const origin = new URL(server.resolvedUrls!.local[0]!).origin;
    return {url:origin+"/",close,diagnostics:()=>hosts.diagnostics()};
  } catch(error) {await close();throw error;}
}
