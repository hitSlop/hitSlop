import { boot } from "../shell/boot";
import { installBrowserBridge } from "../shell/bridge";
import { BrowserHost } from "../schema/constants";
import type { HostRequest } from "../wire/page";

const host = `http://127.0.0.1:${BrowserHost.port}`;
await navigator.serviceWorker.register("/service-worker.js", { scope: "/", type: "module" });
await navigator.serviceWorker.ready;
if (!navigator.serviceWorker.controller) await new Promise<void>(resolve => {
  navigator.serviceWorker.addEventListener("controllerchange", () => resolve(), { once: true });
});
const ports = await new Promise<MessagePort[]>(resolve => {
  const receive = (event: MessageEvent) => {
    if (event.source !== parent || event.origin !== host || event.data?.type !== "connect" || event.ports.length !== 2) return;
    removeEventListener("message", receive);
    resolve([...event.ports]);
  };
  addEventListener("message", receive);
  parent.postMessage({ type: "frameReady" }, host);
});
const page = ports[0]!;
const waiting = new Map<string, { resolve: (reply: string) => void; reject: (error: Error) => void }>();
installBrowserBridge(request => new Promise<string>((resolve, reject) => {
  const id = crypto.randomUUID();
  waiting.set(id, { resolve, reject });
  page.postMessage({ type: "page", id, request: JSON.stringify(request) });
}));
page.onmessage = async ({ data }) => {
  if (data.type === "reply") { waiting.get(data.id)?.resolve(data.json); waiting.delete(data.id); }
  if (data.type === "error") { waiting.get(data.id)?.reject(new Error(data.error)); waiting.delete(data.id); }
  if (data.type === "publication") globalThis.__slop?.publish([{ type: "publication", publication: JSON.parse(data.json) }]);
  if (data.type === "host") {
    try {
      if (!globalThis.__slop) throw new Error("Document is still opening");
      await globalThis.__slop.dispatch(data.request as HostRequest);
      page.postMessage({ type: "hostReply", id: data.id });
    } catch (error) { page.postMessage({ type: "hostReply", id: data.id, error: String(error) }); }
  }
};
// Keep the host channel in the frame: an idle Service Worker can be terminated
// without disconnecting this copy. Every fetch carries its own reply channel.
const resources = ports[1]!;
const resourceReplies = new Map<string, { port: MessagePort; timeout: ReturnType<typeof setTimeout> }>();
navigator.serviceWorker.addEventListener("message", event => {
  if (event.source !== navigator.serviceWorker.controller || event.data?.type !== "resource" || !event.ports[0]) return;
  const { id } = event.data;
  const port = event.ports[0];
  const timeout = setTimeout(() => { resourceReplies.delete(id); port.close(); }, 30_000);
  resourceReplies.set(id, { port, timeout });
  resources.postMessage(event.data);
});
resources.onmessage = ({ data }) => {
  const reply = resourceReplies.get(data.id);
  if (!reply) return;
  resourceReplies.delete(data.id); clearTimeout(reply.timeout);
  reply.port.postMessage(data, data.bytes ? [data.bytes.buffer] : []);
  reply.port.close();
};
await boot();
