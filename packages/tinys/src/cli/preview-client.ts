/// <reference types="vite/client" />
// The `tiny dev` app frame's transport: Vite's socket to the disposable Rust owner. Vite
// compiles it for /__preview__/native.js, the client the preview policy admits; it is never
// part of a tiny.
import { PushLimits } from "../schema/constants";
import type { PreviewHost } from "../shell/preview";
import type { PagePush } from "../wire/page";
import { environmentPresets, isEnvironmentPreset } from "./environment-presets";
import type { HostEnvironment } from "../wire/page.generated";

const hot = import.meta.hot!;
const query = new URL(import.meta.url).searchParams;
const token = query.get("token");
// A page reload replaces the socket, not its disposable document. Each tab has its own key.
const sessionKey = `tinys-preview:${token}`;
const session = sessionStorage.getItem(sessionKey) ?? crypto.randomUUID();
sessionStorage.setItem(sessionKey, session);
const pending = new Map<
  number,
  {
    resolve(reply: unknown): void;
    reject(error: Error): void;
    timer: ReturnType<typeof setTimeout>;
  }
>();
const ready = Promise.withResolvers<void>();
const environmentReady = Promise.withResolvers<void>();
let environment: HostEnvironment = environmentPresets.browser;
const environmentListeners = new Set<() => void>();
addEventListener("message", ({ source, origin, data }) => {
  const preset: unknown = data?.preset;
  if (
    source !== parent ||
    origin !== location.origin ||
    data?.type !== "tiny:environment" ||
    !isEnvironmentPreset(preset)
  )
    return;
  const { platform, device } = environmentPresets[preset];
  environment = Object.freeze({ platform, device });
  environmentReady.resolve();
  for (const listener of environmentListeners) listener();
});
if (parent === window) environmentReady.resolve();
else parent.postMessage({ type: "tiny:environment-ready" }, location.origin);
let next = Number(sessionStorage.getItem(sessionKey + ":request") ?? 0);
let resourceToken: string | undefined;
let failed: Error | undefined;
let receiver: ((pushes: PagePush[]) => void) | undefined;
let pushes: PagePush[] = [];

function fatal(error: string) {
  clearTimeout(opening);
  failed = new Error(error);
  ready.reject(failed);
  environmentReady.reject(failed);
  for (const { reject, timer } of pending.values()) {
    clearTimeout(timer);
    reject(failed);
  }
  pending.clear();
  document.documentElement.dataset.previewDisconnected = "true";
  let notice = document.getElementById("tinys-preview-failure");
  if (!notice) {
    notice = document.body.appendChild(document.createElement("div"));
    notice.id = "tinys-preview-failure";
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
hot.on("tiny:ready", (data: { resourceToken: string }) => {
  clearTimeout(opening);
  resourceToken = data.resourceToken;
  ready.resolve();
});
hot.on("tiny:fatal", ({ error }: { error: string }) => fatal(error));
hot.on("vite:ws:disconnect", () => fatal("Preview connection lost; reload before editing"));
hot.on("tiny:reply", ({ id, reply }: { id: number; reply: unknown }) => {
  const request = pending.get(id);
  if (!request) return;
  pending.delete(id);
  clearTimeout(request.timer);
  request.resolve(reply);
});
hot.on("tiny:push", (batch: PagePush[]) => {
  if (receiver) receiver(batch);
  else {
    pushes.push(...batch);
    if (pushes.length > PushLimits.items) pushes = [{ type: "resync" }];
  }
});

const host: PreviewHost = {
  uiURL: "/__app__/assets/views.js",
  environment: {
    get current() {
      return environment;
    },
    subscribe(receiver) {
      environmentListeners.add(receiver);
      return () => {
        environmentListeners.delete(receiver);
      };
    },
  },
  // The page around this frame re-sends its selected view on each of these.
  ready() {
    parent.postMessage(
      { type: "tiny:preview-ready", widget: query.get("widget") === "true" },
      location.origin,
    );
  },
  attachmentURL: (id) => new URL(`/attachments/${resourceToken}/${id}`, location.href).href,
  async request(request) {
    await ready.promise;
    if (failed) throw failed;
    if (pending.size >= 64) throw new Error("Preview request queue full");
    const id = ++next;
    sessionStorage.setItem(sessionKey + ":request", String(next));
    return new Promise((resolve, reject) => {
      const timer = setTimeout(
        () => fatal("Preview request timed out; outcome unknown. Reload before editing."),
        15000,
      );
      pending.set(id, { resolve, reject, timer });
      hot.send("tiny:request", { token, id, request });
    });
  },
  onPush(next) {
    receiver = next;
    if (pushes.length) next(pushes);
    pushes = [];
  },
};
globalThis.__tinyPreview = host;
hot.on("vite:afterUpdate", () => setTimeout(() => host.ready!(), 0));
hot.send("tiny:open", { token, session });
// The shell is served beside the app, not through Vite. A preview that cannot open shows
// its notice instead.
const shell = "/__shell__/boot.js";
await Promise.all([ready.promise, environmentReady.promise]).then(
  () => import(/* @vite-ignore */ shell),
  () => {},
);
