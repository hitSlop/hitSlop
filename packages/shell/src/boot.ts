// Host-owned page lifecycle. The page shell, SDK, app and host are built from one tree;
// the ctx handed to SlopApp.mount is the only interface apps use.
import type { SlopContext } from "@hitslop/document/abi";
import { ownerAttachments } from "./attachments";
import { call } from "./bridge";
import { createCaptureController } from "./capture";
import { OwnerDocument as Document } from "./owner/document";
import { nativeTransport, browserTransport } from "./owner/transport";
import { installPresentationStage, presentationStage } from "./presentation";
import type { ObjectNode } from "@hitslop/document";
import { fromDescriptor } from "@hitslop/document/internal";
import { applyTheme } from "./theme-runtime";
import { mountViewLifecycle } from "./view-lifecycle";
import { checkedApp } from "./app-module";
import { isDocumentError } from "@hitslop/document";
import type { PageResult } from "@hitslop/schema/page";
import type { AppRow } from "@hitslop/schema";
import type {} from "./page-handle";
import { hostDispatcher } from "./host-dispatch";
import { ErrorTextLimit, RuntimeABI } from "@hitslop/schema/constants";

const isNative = () => Boolean((globalThis as any).webkit?.messageHandlers?.hitslop);
/** Reports a page error to the host, or to the console in the browser preview. */
const report = (native: boolean, kind: "application" | "operation", error: unknown) => {
  if (native) void call({ method: "pageError", kind, error: describe(error) }).catch(() => {});
  else console.error(error);
};
const fetchJSON = async (path: string, missing: string) => {
  const response = await fetch(path);
  if (!response.ok) throw new Error(missing);
  return response.json();
};

/** The browser preview's document: the build's `app.json`, the row a `.slop` stores, in
 * disposable memory storage. */
async function previewApp(): Promise<{ config: PageResult<"config">; initial: unknown; template: string; theme: Record<string, string> }> {
  const app: AppRow = await fetchJSON("/app.json", "Missing app.json");
  const { theme, manifest, descriptor, initial } = app;
  return {
    config: { readOnly: false, presentation: manifest.presentation, descriptor: descriptor as object },
    initial,
    template: manifest.slug,
    theme,
  };
}

/** Open the document with host or disposable memory storage. The native owner holds the
 * saved state and sends the descriptor with the config; only the preview reads the
 * initial values. */
async function openDocument(native: boolean) {
  const { config, initial, template, theme } = native
    ? { config: await call({ method: "config" }), initial: undefined, template: "", theme: {} }
    : await previewApp();
  // The core checked the descriptor when the file opened, or the build evaluated it.
  const descriptor = config.descriptor as ObjectNode;
  const host = native ? nativeTransport(config.readOnly) : undefined;
  const transport = host ?? (await browserTransport(descriptor, initial, template, theme));
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
  const attachments = ownerAttachments(doc, native);
  return { config, doc, attachments };
}

// Every app ABI the core admits gets this one context. Raising RuntimeABI fails here: keep
// this context for the released ABI and dispatch on the app's (docs/engineering-contract.md).
RuntimeABI satisfies 1;

/** The app-facing interface over this page's document and host services. The core refused
 * an app needing a newer runtime ABI before this page opened. */
function createContext(
  doc: Document<ObjectNode>,
  options: {
    attachments: ReturnType<typeof ownerAttachments>;
    capture: ReturnType<typeof createCaptureController>;
    resize(size: { width: number; height: number }): Promise<void>;
    reportError(error: unknown): void;
  },
): SlopContext {
  const { attachments, capture } = options;
  const document = Object.freeze({
    get current() {
      return doc.current;
    },
    fields: doc.fields,
    at: ((value: any) => doc.at(value)) as SlopContext["document"]["at"],
    change: <R>(callback: (tx: any) => R) => doc.change(callback),
    flush: () => doc.flush(),
    undo: () => doc.undo(),
    redo: () => doc.redo(),
    subscribe: (listener: Parameters<typeof doc.subscribe>[0]) => doc.subscribe(listener),
    observe: (read: () => void) => doc.observe(read),
  });
  return Object.freeze({
    document,
    bind: Object.freeze({ text: doc.bindText.bind(doc) }),
    capture: Object.freeze({
      isRenderer: () => globalThis.document?.documentElement.dataset.slopRenderer === "true",
      onPrepare: capture.onPrepare,
      registerTarget: capture.registerTarget,
    }),
    attachments: Object.freeze(attachments),
    window: Object.freeze({ resize: options.resize }),
    reportError: options.reportError,
  });
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
  const app = import(new URL("/assets/app.js", location.href).href).then(
    (module) => module.default as unknown,
    authored,
  );
  app.catch(() => {});
  const opened = openDocument(native);
  const { config, doc, attachments } = await opened;
  installPresentationStage(presentationStage(config.presentation));
  const view = checkedApp(await app, config.descriptor);
  const capture = createCaptureController();
  const reportError = (error: unknown) => {
    globalThis.document.dispatchEvent(new CustomEvent("hitslop:render-error", { detail: error }));
    report(native, "application", error);
  };
  const ctx = createContext(doc as Document<ObjectNode>, {
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
