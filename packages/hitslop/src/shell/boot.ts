// Host-owned page lifecycle. The page shell, SDK, app and host are built from one tree;
// the ctx handed to SlopApp.mount is the only interface apps use.
import { createContext as abi1 } from "./abi/1";
import { ownerAttachments } from "./attachments";
import { call } from "./bridge";
import { createCaptureController } from "./capture";
import { OwnerDocument as Document } from "./owner/document";
import { nativeTransport } from "./owner/transport";
import { installPresentationStage, presentationStage } from "./presentation";
import type { ObjectNode } from "../sdk/schema";
import { fromDescriptor } from "../sdk/internal";
import { applyTheme } from "./theme-runtime";
import { mountViewLifecycle } from "./view-lifecycle";
import { checkedApp } from "./app-module";
import { isDocumentError } from "../sdk/errors";
import type { PageResult } from "../wire/page";
import type {} from "./page-handle";
import { hostDispatcher } from "./host-dispatch";
import { ErrorTextLimit, RuntimeABI } from "../schema/constants";

const isNative = () => Boolean((globalThis as any).webkit?.messageHandlers?.hitslop);
/** Reports a page error to the host, or to the console in the browser preview. */
const report = (native: boolean, kind: "application" | "operation", error: unknown) => {
  if (native) void call({ method: "pageError", kind, error: describe(error) }).catch(() => {});
  else console.error(error);
};
/** Open the document with host or disposable memory storage. The native owner holds the
 * saved state and sends the descriptor with the config; only the preview reads the
 * initial values. */
async function openDocument(native: boolean) {
  const config = await call({ method: "config" });
  const descriptor = config.descriptor as ObjectNode;
  const host = nativeTransport(config.readOnly);
  const transport = host;
  // Boot owns the host entry point. Register it before opening the document; open
  // installs the receiver synchronously before requesting its initial snapshot.
  const page = { publish: host?.publish ?? (() => {}) };
  globalThis.__slop = Object.assign(page, { dispatch: hostDispatcher(page) });
  const doc = await Document.open(
    fromDescriptor(descriptor),
    transport,
    (error, kind = "operation") => report(native, kind, error),
  );
  let palette = doc.theme;
  applyTheme(palette);
  doc.subscribe(() => {
    if (doc.theme === palette) return;
    palette = doc.theme;
    applyTheme(palette);
  });
  const attachments = ownerAttachments(doc);
  return { config, doc, attachments };
}

// Every app ABI the core admits gets this one context. Raising RuntimeABI fails here: keep
// this context for the released ABI and dispatch on the app's (docs/engineering-contract.md).
const contexts = { 1: abi1 } satisfies Record<typeof RuntimeABI, typeof abi1>;
function contextFor(abi: number) {
  if (abi === 1) return contexts[abi];
  throw new Error(`Unsupported runtime ABI ${abi}`);
}

/** The message first: WebKit's `stack` lists only frames, V8's repeats the message. */
const describe = (error: unknown) => {
  if (!(error instanceof Error)) return String(error).slice(0, ErrorTextLimit);
  const head = `${error.name}: ${error.message}`;
  const stack = error.stack ?? "";
  return (stack.startsWith(head) ? stack : stack ? `${head}\n${stack}` : head).slice(
    0,
    ErrorTextLimit,
  );
};

/** Visible sessions: open the document, then mount the package's app module. */
export async function boot() {
  const native = isNative();
  for (const type of ["error", "unhandledrejection"] as const)
    addEventListener(type, (event) => {
      const error =
        "reason" in event
          ? event.reason
          : ((event as ErrorEvent).error ?? (event as ErrorEvent).message);
      report(native, isDocumentError(error) ? "operation" : "application", error);
    });
  // Failures inside the package's own module are authored failures, reported as such.
  const authored = (error: unknown) => {
    report(native, "application", error);
    throw error;
  };
  const app = import(new URL((globalThis as any).__hitslopPreview?.uiURL ?? "/assets/ui.js", location.href).href).then(
    (module) => module.default as unknown,
    authored,
  );
  app.catch(() => {});
  const opened = openDocument(native);
  const { config, doc, attachments } = await opened;
  if (native && config.style) {
    await new Promise<void>((resolve, reject) => {
      const style = document.createElement("link");
      style.rel = "stylesheet";
      style.href = "/assets/ui.css";
      style.onload = () => resolve();
      style.onerror = () => reject(new Error("Could not load app stylesheet"));
      document.head.append(style);
    });
  }
  installPresentationStage(presentationStage(config.window));
  const view = checkedApp(await app, config.descriptor);
  const capture = createCaptureController();
  const reportError = (error: unknown) => {
    globalThis.document.dispatchEvent(new CustomEvent("hitslop:render-error", { detail: error }));
    report(native, "application", error);
  };
  const ctx = contextFor(config.runtimeABI)(doc as Document<ObjectNode>, {
    attachments,
    capture,
    resize: async (size) => {
      if (native) await call({ method: "window.resize", ...size });
    },
    reportError,
  });
  const target = globalThis.document.body;
  const lifecycle = await mountViewLifecycle({
    mount: async () => {
      const mounted = await Promise.resolve()
        .then(() => view.mount(ctx, target))
        .catch(authored);
      return { rendered: () => mounted?.rendered?.(), unmount: () => mounted?.unmount?.() };
    },
    document: doc,
    target,
    recovered: native ? () => call({ method: "pageRecovered" }) : undefined,
  });
  Object.assign(globalThis.__slop!, lifecycle, { capture });
  if (native) await call({ method: "ready" });
}
