import { SharingHost } from "../../packages/hitslop/src/wire/constants.generated";

const appleAppSiteAssociation = JSON.stringify({
  applinks: { details: [{
    appIDs: ["78UAXU8QG8.com.hitslop.app"],
    components: [{ "/": "/join", comment: "Live sharing links" }],
  }] },
  appclips: { apps: ["78UAXU8QG8.com.hitslop.app.Clip"] },
});

// Keep the callback fragment in the browser: it never reaches the host or its logs.
const appleCallbackScript = 'if (location.hash) location.replace("hitslop://auth/apple" + location.hash);';
const appleCallbackPage = `<!doctype html><html lang="en"><meta charset="utf-8"><title>Sign in with Apple</title><p>Returning to hitSlop…</p><script>${appleCallbackScript}</script></html>`;
const appleCallbackPolicy = "default-src 'none'; script-src 'sha256-dK5bMXUEDI+WHBPUTX228sBNAIgNZlMMmLz0Wi7JLV0='; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

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
    if (trusted && url.pathname === "/auth/apple") {
      return new Response(request.method === "HEAD" ? null : appleCallbackPage, {
        headers: {
          "Content-Type": "text/html; charset=utf-8",
          "Content-Security-Policy": appleCallbackPolicy,
          "Cache-Control": "no-store",
          "Referrer-Policy": "no-referrer",
          "X-Content-Type-Options": "nosniff",
        },
      });
    }
    if (trusted && url.pathname === "/.well-known/apple-app-site-association") {
      return new Response(request.method === "HEAD" ? null : appleAppSiteAssociation, {
        headers: {
          "Content-Type": "application/json",
          "Cache-Control": "public, max-age=3600",
          "X-Content-Type-Options": "nosniff",
        },
      });
    }
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
