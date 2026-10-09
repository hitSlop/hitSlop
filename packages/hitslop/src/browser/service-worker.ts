/// <reference lib="webworker" />
import { BrowserHost } from "../schema/constants";
declare const self: ServiceWorkerGlobalScope;
self.addEventListener("install", () => { void self.skipWaiting(); });
self.addEventListener("activate", event => event.waitUntil(self.clients.claim()));
self.addEventListener("fetch", event => {
  const url = new URL(event.request.url);
  const match = /^\/(assets|attachments)\/(.+)$/.exec(url.pathname);
  if (url.origin !== self.location.origin || !match) return;
  event.respondWith((async () => {
    try {
      if (!["GET", "HEAD"].includes(event.request.method)) return new Response(null, { status: 405 });
      const client = await self.clients.get(event.clientId);
      if (!client) return new Response("The browser copy is no longer open", { status: 503 });
      const route = match[1] === "assets" ? "app" : "attachment";
      const key = decodeURIComponent(match[2]!);
      const request = (offset: number, length: number) => new Promise<any>((resolve, reject) => {
        const channel = new MessageChannel();
        const timeout = setTimeout(() => { channel.port1.close(); reject(new Error("Resource request timed out")); }, 30_000);
        channel.port1.onmessage = ({ data }) => {
          clearTimeout(timeout); channel.port1.close();
          if (data.type === "error") reject(new Error(data.error)); else resolve(data);
        };
        client.postMessage({ type: "resource", id: crypto.randomUUID(), route, key, offset, length }, [channel.port2]);
      });
      const info = await request(0, 0);
      const range = event.request.headers.get("Range");
      const bounds = range?.match(/^bytes=(\d+)-(\d*)$/);
      const start = bounds ? Number(bounds[1]) : 0;
      const end = bounds?.[2] ? Math.min(Number(bounds[2]) + 1, info.size) : info.size;
      if (range && (!bounds || !Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start >= info.size || end <= start))
        return new Response(null, { status: 416, headers: { "Content-Range": `bytes */${info.size}` } });
      const headers = new Headers({ "Content-Type": info.media_type, "Content-Length": String(end - start), "Accept-Ranges": "bytes", "Cache-Control": "no-store", "X-Content-Type-Options": "nosniff" });
      if (route === "attachment") headers.set("Content-Security-Policy", "sandbox");
      if (range) headers.set("Content-Range", `bytes ${start}-${end - 1}/${info.size}`);
      let at = start, cancelled = false;
      const body = new ReadableStream<Uint8Array>({
        async pull(controller) {
          if (at === end) { controller.close(); return; }
          const length = Math.min(BrowserHost.chunkBytes, end - at);
          try {
            const result = await request(at, length);
            if (cancelled) return;
            if (result.size !== info.size || result.offset !== at || result.bytes?.byteLength !== length) throw new Error("Incomplete resource transfer");
            at += length; controller.enqueue(result.bytes);
          } catch (error) { if (!cancelled) controller.error(error); }
        },
        cancel() { cancelled = true; },
      }, { highWaterMark: 0 });
      return new Response(event.request.method === "HEAD" ? null : body, { status: range ? 206 : 200, headers });
    } catch (error) { return new Response(String(error), { status: 404 }); }
  })());
});
export {};
