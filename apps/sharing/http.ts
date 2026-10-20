import { SharingHost } from "../../packages/hitslop/src/wire/constants.generated";

type Binding = { fetch(request: Request): Promise<Response> };
/** HTTP/static glue only; Rust authorizes resources and owns the protocol. */
export default {
  async fetch(request: Request, env: { ASSETS: Binding; RELAY: Binding }) {
    const url = new URL(request.url);
    const trusted = url.origin === SharingHost.origin;
    const authored = url.protocol === "https:" && url.hostname.endsWith(`.${SharingHost.domain}`);
    if (!trusted && !authored) return new Response("Unknown origin", { status: 403 });
    if ((trusted && (url.pathname.startsWith("/v1/") || url.pathname === "/health")) ||
        (authored && /^\/(assets|attachments|_resources)\//.test(url.pathname)))
      return env.RELAY.fetch(request);
    if (request.method !== "GET" && request.method !== "HEAD")
      return new Response(null, { status: 405 });
    let path = url.pathname;
    if (authored) {
      if (path === "/") path = "/frame.html";
      else if (path === "/frame.js") path = "/browser/frame.js";
      else if (path === "/service-worker.js") path = "/browser/service-worker.js";
      else return new Response("Not found", { status: 404 });
    } else if (path === "/" || path === "/join") path = "/index.html";
    else if (!/^\/browser\/(host|worker)\.js$/.test(path) &&
             !/^\/browser\/core\/[a-zA-Z0-9_.-]+$/.test(path) && path !== "/openapi.json")
      return new Response("Not found", { status: 404 });
    const result = await env.ASSETS.fetch(new Request(new URL(path, request.url), { method: request.method }));
    const response = new Response(request.method === "HEAD" ? null : result.body, result);
    response.headers.set("Cache-Control", "no-store");
    response.headers.set("X-Content-Type-Options", "nosniff");
    response.headers.set("Referrer-Policy", "no-referrer");
    if (path.endsWith(".html")) response.headers.set("Content-Security-Policy", trusted ? SharingHost.hostPolicy : SharingHost.pagePolicy);
    return response;
  },
};
