// Host-owned page lifecycle. The page shell, SDK, app and host are built from one tree;
// the ctx handed to SlopApp.mount is the only interface apps use.
import type { SlopApp, SlopContext } from "./abi";
import { ownerAttachments } from "./owner/attachments";
import { hostCall } from "./bridge";
import { captureController } from "./capture";
import { OwnerDocument as Document } from "./owner/document";
import { nativeTransport, browserTransport } from "./owner/transport";
import {
  installPresentationStage,
  presentationStage,
} from "./presentation";
import type { ObjectNode } from "./schema";
import { fromDescriptor } from "./descriptor";
import { openTheme } from "./theme-runtime";
import { mountViewLifecycle } from "./view-lifecycle";
import { ErrorTextLimit } from "@hitslop/schema/constants";

const isNative = () => Boolean((globalThis as any).webkit?.messageHandlers?.hitslop);
/** Reports a page error to the host, or to the console in the browser preview. */
const report = (native: boolean, kind: "application" | "operation", error: unknown) => {
  if (native) void hostCall({ method: "pageError", kind, error: describe(error) }).catch(() => {});
  else console.error(error);
};
const fetchJSON = async (path: string, missing: string) => {
  const response = await fetch(path);
  if (!response.ok) throw new Error(missing);
  return response.json();
};

type PageConfig = {
  view: string;
  documentID: string;
  readOnly: boolean;
  presentation?: import("@hitslop/schema").SlopPresentation;
};
/** The browser preview's config: a disposable document, with the manifest's stage. */
async function previewConfig(): Promise<PageConfig> {
  const manifest = await fetch("/manifest.json").then((r) => (r.ok ? r.json() : undefined));
  return {
    view: crypto.randomUUID(),
    documentID: crypto.randomUUID(),
    readOnly: false,
    presentation: manifest?.presentation,
  };
}

/** Open the package's document with host or disposable memory storage. The native owner
 * holds the saved state, so only the preview reads the initial values. */
async function openDocument(native: boolean) {
  const [config, descriptor, initial, theme] = await Promise.all([
    native ? (hostCall({ method: "config" }) as Promise<PageConfig>) : previewConfig(),
    fetchJSON("/state.schema.json", "Missing document descriptor"),
    native ? undefined : fetchJSON("/initial.json", "Missing initial values"),
    openTheme(native),
  ]);
  const doc = await Document.open(
    fromDescriptor(descriptor),
    native
      ? nativeTransport(config.documentID, config.view, config.readOnly)
      : await browserTransport(descriptor, initial),
    (error, kind = "operation") => report(native, kind, error),
  );
  const attachments = ownerAttachments(doc, native);
  return { config, doc, theme, attachments };
}

/** The app-facing interface over this page's document and host services. */
function createContext(
  doc: Document<ObjectNode>,
  options: {
    attachments: ReturnType<typeof ownerAttachments>;
    capture: ReturnType<typeof captureController>;
    resize(size: { width: number; height: number }): Promise<void>;
    reportError(error: unknown): void;
  },
): SlopContext {
  const { attachments, capture } = options;
  const document = Object.freeze({
    get key() {
      return doc.key;
    },
    get id() {
      return doc.id;
    },
    get current() {
      return doc.current;
    },
    get issues() {
      return doc.issues;
    },
    fields: doc.fields,
    at: ((value: any) => doc.at(value)) as SlopContext["document"]["at"],
    change: <R>(callback: (tx: any) => R) => doc.change(callback),
    flush: () => doc.flush(),
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
    attachments: Object.freeze({
      import: ((file, reference) =>
        attachments.import(file, reference as any)) as SlopContext["attachments"]["import"],
      read: (id: string, options?: { type?: string }) => attachments.read(id, options),
    }),
    window: Object.freeze({ resize: options.resize }),
    reportError: options.reportError,
  });
}

/** The message first: WebKit's `stack` lists only frames, V8's repeats the message. */
const describe = (error: unknown) => {
  if (!(error instanceof Error)) return String(error).slice(0, ErrorTextLimit);
  const head = `${error.name}: ${error.message}`;
  const stack = error.stack ?? "";
  return (stack.startsWith(head) ? stack : stack ? `${head}\n${stack}` : head).slice(0, ErrorTextLimit);
};

/** Visible sessions: open the document, then mount the package's app module. */
export async function boot() {
  const native = isNative();
  // Failures inside the package's own module are authored failures, reported as such.
  const authored = (error: unknown) => {
    report(native, "application", error);
    throw error;
  };
  const app = import(new URL("/assets/app.js", location.href).href).then(
    (module) => module.default as SlopApp,
    authored,
  );
  app.catch(() => {});
  const opened = openDocument(native);
  const { config, doc, theme, attachments } = await opened;
  if (config.presentation) installPresentationStage(presentationStage(config.presentation));
  const view = await app;
  if (!view || typeof view.mount !== "function")
    throw new Error("assets/app.js must export default { mount(ctx, target) }");
  const capture = captureController();
  const reportError = (error: unknown) => {
    globalThis.document.dispatchEvent(new CustomEvent("hitslop:render-error", { detail: error }));
    report(native, "application", error);
  };
  const ctx = createContext(doc as Document<ObjectNode>, {
    attachments,
    capture,
    resize: async (size) => {
      if (native) await hostCall({ method: "window.resize", ...size });
    },
    reportError,
  });
  const target = globalThis.document.body;
  globalThis.__slop = await mountViewLifecycle({
    mount: async () => {
      const mounted = await Promise.resolve()
        .then(() => view.mount(ctx, target))
        .catch(authored);
      return { rendered: () => mounted?.rendered?.(), unmount: () => mounted?.unmount?.() };
    },
    document: doc,
    theme,
    target,
    recovered: native ? () => hostCall({ method: "pageRecovered" }) : undefined,
  });
  if (native) await hostCall({ method: "ready" });
}
