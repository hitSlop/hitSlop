/// <reference types="vite/client" />
// The `slop dev` app frame's transport: Vite's socket to the disposable Rust owner. Vite
// compiles it for /__preview__/native.js, the client the preview policy admits; it is never
// part of a slop.
import { PushLimits } from "../schema/constants";
import type { PreviewHost } from "../shell/preview";
import type { PagePush } from "../wire/page";

const hot = import.meta.hot!;
const query = new URL(import.meta.url).searchParams;
const token = query.get("token");
const pending = new Map<
  number,
  {
    resolve(reply: unknown): void;
    reject(error: Error): void;
    timer: ReturnType<typeof setTimeout>;
  }
>();
const ready = Promise.withResolvers<void>();
let next = 0;
let resourceToken: string | undefined;
let failed: Error | undefined;
let receiver: ((pushes: PagePush[]) => void) | undefined;
let pushes: PagePush[] = [];

function fatal(error: string) {
  clearTimeout(opening);
  failed = new Error(error);
  ready.reject(failed);
  for (const { reject, timer } of pending.values()) {
    clearTimeout(timer);
    reject(failed);
  }
  pending.clear();
  document.documentElement.dataset.previewDisconnected = "true";
  let notice = document.getElementById("hitslop-preview-failure");
  if (!notice) {
    notice = document.body.appendChild(document.createElement("div"));
    notice.id = "hitslop-preview-failure";
    notice.role = "alert";
    notice.style.cssText =
      "position:fixed;inset:0 0 auto;z-index:2147483647;padding:12px;background:#fff1d6;color:#452c10;font:14px system-ui";
  }
  notice.textContent = error;
}
const opening = setTimeout(
  () => fatal("The preview did not start; check the terminal, then reload"),
  20000,
);
hot.on("slop:ready", (data: { resourceToken: string }) => {
  clearTimeout(opening);
  resourceToken = data.resourceToken;
  ready.resolve();
});
hot.on("slop:fatal", ({ error }: { error: string }) => fatal(error));
hot.on("vite:ws:disconnect", () => fatal("Preview connection lost; reload before editing"));
hot.on("slop:reply", ({ id, reply }: { id: number; reply: unknown }) => {
  const request = pending.get(id);
  if (!request) return;
  pending.delete(id);
  clearTimeout(request.timer);
  request.resolve(reply);
});
hot.on("slop:push", (batch: PagePush[]) => {
  if (receiver) receiver(batch);
  else {
    pushes.push(...batch);
    if (pushes.length > PushLimits.items) pushes = [{ type: "resync" }];
  }
});

const host: PreviewHost = {
  uiURL: "/__app__/assets/views.js",
  // The page around this frame re-sends its selected view on each of these.
  ready() {
    parent.postMessage(
      { type: "slop:preview-ready", widget: query.get("widget") === "true" },
      location.origin,
    );
  },
  attachmentURL: (id) => new URL(`/attachments/${resourceToken}/${id}`, location.href).href,
  async request(request) {
    await ready.promise;
    if (failed) throw failed;
    if (pending.size >= 64) throw new Error("Preview request queue full");
    const id = ++next;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(
        () => fatal("Preview request timed out; outcome unknown. Reload before editing."),
        15000,
      );
      pending.set(id, { resolve, reject, timer });
      hot.send("slop:request", { token, id, request });
    });
  },
  onPush(next) {
    receiver = next;
    if (pushes.length) next(pushes);
    pushes = [];
  },
};
globalThis.__slopPreview = host;
hot.on("vite:afterUpdate", () => setTimeout(() => host.ready!(), 0));
hot.send("slop:open", { token });
// The shell is served beside the app, not through Vite. A preview that cannot open shows
// its notice instead.
const shell = "/__shell__/boot.js";
await ready.promise.then(
  () => import(/* @vite-ignore */ shell),
  () => {},
);
