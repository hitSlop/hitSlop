import { boot } from "../shell/boot";
import { installBrowserBridge } from "../shell/bridge";
import { resourceDelivery } from "./resource-delivery";
import { PendingRequests, copyDeadline, type FrameMessage, type HostReply } from "./requests";

const hosted =
  resourceDelivery.kind === "network" && new URL(location.href).searchParams.get("local") !== "1";
const host = resourceDelivery.host;
if (!hosted) {
  await navigator.serviceWorker.register("/service-worker.js", { scope: "/", type: "module" });
  await navigator.serviceWorker.ready;
  if (!navigator.serviceWorker.controller)
    await new Promise<void>((resolve) => {
      navigator.serviceWorker.addEventListener("controllerchange", () => resolve(), { once: true });
    });
}
const ports = await new Promise<MessagePort[]>((resolve) => {
  const receive = (event: MessageEvent) => {
    if (
      event.source !== parent ||
      event.origin !== host ||
      event.data?.type !== "connect" ||
      event.ports.length !== (hosted ? 1 : 2)
    )
      return;
    removeEventListener("message", receive);
    resolve([...event.ports]);
  };
  addEventListener("message", receive);
  parent.postMessage({ type: "frameReady" }, host);
});
const page = ports[0]!;
// The page's requests wait here for the copy's JSON reply. A write that ran out the deadline has
// an unknown outcome and may still be applied, so the whole connection fails and nothing is
// retried; the host's `fatal` message ends the wait sooner, when the worker is known to be gone.
const waiting = new PendingRequests<string>();
installBrowserBridge((request) =>
  waiting.start((id) => page.postMessage({ type: "page", id, request: JSON.stringify(request) }), {
    timeout: {
      after: copyDeadline,
      message: "The document did not respond. Reload to recover its last confirmed save.",
      terminal: true,
    },
  }),
);
page.onmessage = async ({ data }: MessageEvent<FrameMessage>) => {
  if (data.type === "reply") waiting.settle(data.id, data.json);
  if (data.type === "error") waiting.settle(data.id, undefined, new Error(data.error));
  if (data.type === "fatal") {
    waiting.fail(new Error(data.error));
    page.close();
  }
  if (data.type === "publication") globalThis.__slop?.publish([JSON.parse(data.json)]);
  if (data.type === "host") {
    try {
      if (!globalThis.__slop) throw new Error("Document is still opening");
      await globalThis.__slop.dispatch(data.request);
      page.postMessage({ type: "hostReply", id: data.id } satisfies HostReply);
    } catch (error) {
      page.postMessage({
        type: "hostReply",
        id: data.id,
        error: String(error),
      } satisfies HostReply);
    }
  }
};
// Keep the host channel in the frame: an idle Service Worker can be terminated
// without disconnecting this copy. Every fetch carries its own reply channel.
if (!hosted) {
  const resources = ports[1]!;
  const resourceReplies = new Map<
    string,
    { port: MessagePort; timeout: ReturnType<typeof setTimeout> }
  >();
  navigator.serviceWorker.addEventListener("message", (event) => {
    if (
      event.source !== navigator.serviceWorker.controller ||
      !["resource", "resourcePlan"].includes(event.data?.type) ||
      !event.ports[0]
    )
      return;
    const { id } = event.data;
    const port = event.ports[0];
    const timeout = setTimeout(() => {
      resourceReplies.delete(id);
      port.close();
    }, 30_000);
    resourceReplies.set(id, { port, timeout });
    resources.postMessage(event.data);
  });
  resources.onmessage = ({ data }) => {
    const reply = resourceReplies.get(data.id);
    if (!reply) return;
    resourceReplies.delete(data.id);
    clearTimeout(reply.timeout);
    reply.port.postMessage(data, data.bytes ? [data.bytes.buffer] : []);
    reply.port.close();
  };
}
await boot();
