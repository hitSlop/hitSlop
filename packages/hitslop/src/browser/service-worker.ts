/// <reference lib="webworker" />
import { BrowserHost } from "../schema/constants";
declare const self: ServiceWorkerGlobalScope;
self.addEventListener("install", () => {
  void self.skipWaiting();
});
self.addEventListener("activate", (event) => event.waitUntil(self.clients.claim()));
self.addEventListener("fetch", (event) => {
  const url = new URL(event.request.url);
  const match = /^\/(assets|attachments)\/(.+)$/.exec(url.pathname);
  if (url.origin !== self.location.origin || !match) return;
  event.respondWith(
    (async () => {
      try {
        if (!["GET", "HEAD"].includes(event.request.method))
          return new Response(null, { status: 405 });
        const client = await self.clients.get(event.clientId);
        if (!client) return new Response("The browser copy is no longer open", { status: 503 });
        if (url.protocol === "https:" && new URL(client.url).searchParams.get("local") !== "1")
          return fetch(event.request);
        const route = match[1] === "assets" ? "app" : "attachment";
        const key = decodeURIComponent(match[2]!);
        const request = (fields: Record<string, unknown>) =>
          new Promise<any>((resolve, reject) => {
            const channel = new MessageChannel();
            const timeout = setTimeout(() => {
              channel.port1.close();
              reject(new Error("Resource request timed out"));
            }, 30_000);
            channel.port1.onmessage = ({ data }) => {
              clearTimeout(timeout);
              channel.port1.close();
              if (data.type === "error") reject(new Error(data.error));
              else resolve(data);
            };
            client.postMessage({ id: crypto.randomUUID(), route, key, ...fields }, [channel.port2]);
          });
        const { plan, size } = await request({
          type: "resourcePlan",
          range: event.request.headers.get("Range"),
        });
        const headers = new Headers(plan.headers);
        if (!plan.span || event.request.method === "HEAD")
          return new Response(null, { status: plan.status, headers });
        const start = plan.span.offset,
          end = start + plan.span.length;
        let at = start,
          cancelled = false;
        const body = new ReadableStream<Uint8Array>(
          {
            async pull(controller) {
              if (at === end) {
                controller.close();
                return;
              }
              const length = Math.min(BrowserHost.chunkBytes, end - at);
              try {
                const result = await request({ type: "resource", offset: at, length });
                if (cancelled) return;
                if (
                  result.size !== size ||
                  result.offset !== at ||
                  result.bytes?.byteLength !== length
                )
                  throw new Error("Incomplete resource transfer");
                at += length;
                controller.enqueue(result.bytes);
              } catch (error) {
                if (!cancelled) controller.error(error);
              }
            },
            cancel() {
              cancelled = true;
            },
          },
          { highWaterMark: 0 },
        );
        return new Response(body, { status: plan.status, headers });
      } catch (error) {
        return new Response(String(error), { status: 404 });
      }
    })(),
  );
});
