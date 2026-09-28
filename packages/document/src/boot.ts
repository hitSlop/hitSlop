// Host-owned page lifecycle. Everything here ships with the app's runtime and may
// change in any release; only the ctx handed to SlopApp.mount is a frozen contract.
import { ABI, type SlopApp, type SlopContext } from "./abi";
import { AttachmentController, configureAttachments } from "./attachments";
import { bindText } from "./bind-text";
import { bindValue } from "./bind-value";
import { HostStore, hostCall } from "./bridge";
import { captureController } from "./capture";
import { Document } from "./document";
import { MemoryStore } from "./memory";
import { installPresentationStage, presentationStage, type PresentationStage } from "./presentation";
import { fromDescriptor, type ObjectNode } from "./schema";
import { Session } from "./session";
import { openTheme, type ThemeController } from "./theme-runtime";
import { mountViewLifecycle } from "./view-lifecycle";

let initializing: Promise<void> | undefined;
/** Fetch and compile the engine once per page. */
export function initialize() {
  return (initializing ??= (async () => {
    const url = new URL("./loro/index.js", import.meta.url).href;
    const { default: init } = await import(url);
    await init({ module_or_path: new URL("./loro/loro_wasm_bg.wasm", import.meta.url).href });
  })());
}

const isNative = () => Boolean((globalThis as any).webkit?.messageHandlers?.storage);
const fetchJSON = async (path: string, missing: string) => {
  const response = await fetch(path);
  if (!response.ok) throw new Error(missing);
  return response.json();
};

/** Open the package's document with host or disposable memory storage. */
async function openDocument(native: boolean) {
  const [, config, descriptor, initial, theme] = await Promise.all([
    initialize(),
    native
      ? hostCall({ method: "config" })
      : ({ epoch: crypto.randomUUID() } as { epoch: string; presentation?: PresentationStage }),
    fetchJSON("/state.schema.json", "Missing document descriptor"),
    fetchJSON("/initial.json", "Missing initial values"),
    openTheme(native),
  ]);
  const doc = await Document.open(
    fromDescriptor(descriptor),
    native ? new HostStore() : new MemoryStore(),
    initial,
  );
  const attachments = configureAttachments(doc, native);
  const session = new Session(doc, config.epoch, theme, attachments);
  return { config, doc, theme, attachments, session };
}

/** Build the frozen app-facing interface over this release's implementation. */
export function createContext(
  doc: Document<ObjectNode>,
  options: {
    attachments: AttachmentController;
    theme: Pick<ThemeController, "get">;
    capture: ReturnType<typeof captureController>;
    resize(size: { width: number; height: number }): Promise<void>;
    reportError(error: unknown): void;
  },
): SlopContext {
  const { attachments, theme, capture } = options;
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
    get status() {
      return doc.status;
    },
    get error() {
      return doc.error;
    },
    get issues() {
      return doc.issues;
    },
    get full() {
      return doc.full;
    },
    fields: doc.fields,
    at: ((value: any) => doc.at(value)) as SlopContext["document"]["at"],
    change: <R>(callback: (tx: any) => R, options?: { message?: string }) =>
      doc.change(callback, { message: options?.message }),
    flush: () => doc.flush(),
    subscribe: (listener: Parameters<typeof doc.subscribe>[0]) => doc.subscribe(listener),
  });
  return Object.freeze({
    abi: ABI,
    capabilities: Object.freeze([]),
    document,
    bind: Object.freeze({ text: bindText, value: bindValue }),
    capture: Object.freeze({
      isRenderer: () => globalThis.document?.documentElement.dataset.slopRenderer === "true",
      onPrepare: capture.onPrepare,
      registerTarget: capture.registerTarget,
    }),
    attachments: Object.freeze({
      import: (file: File, options: Parameters<AttachmentController["import"]>[1]) => attachments.import(file, options),
      read: (id: string, options?: { type?: string }) => attachments.read(id, options),
      list: () => attachments.list(),
    }),
    theme: Object.freeze({ get: () => theme.get() }),
    window: Object.freeze({ resize: options.resize }),
    reportError: options.reportError,
  });
}

const describe = (error: unknown) =>
  (error instanceof Error ? (error.stack ?? error.message) : String(error)).slice(0, 4096);

/** Visible sessions: open the document, then mount the package's app module. */
export async function boot() {
  const native = isNative();
  try {
    // Failures inside the package's own module are authored failures, reported as such.
    const authored = (error: unknown) => {
      if (native) void hostCall({ method: "runtimeError", kind: "application", error: describe(error) }).catch(() => {});
      throw error;
    };
    const app = import(new URL("/assets/app.js", location.href).href).then(
      (module) => module.default as SlopApp,
      authored,
    );
    app.catch(() => {});
    const opened = openDocument(native);
    const { config, doc, theme, attachments, session } = await opened;
    // The disposable preview derives the stage from the manifest, as the native host does.
    if (!native) {
      const manifest = await fetch("/manifest.json").then((r) => (r.ok ? r.json() : undefined));
      if (manifest?.presentation) config.presentation = presentationStage(manifest.presentation);
    }
    if (config.presentation) installPresentationStage(config.presentation);
    const view = await app;
    if (!view || typeof view.mount !== "function")
      throw new Error("assets/app.js must export default { mount(ctx, target) }");
    const capture = captureController();
    const reportError = (error: unknown) => {
      globalThis.document.dispatchEvent(new CustomEvent("hitslop:render-error", { detail: error }));
      if (native) void hostCall({ method: "runtimeError", kind: "application", error: describe(error) }).catch(() => {});
    };
    const ctx = createContext(doc as Document<ObjectNode>, {
      attachments,
      theme,
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
      target,
      session,
      capture,
      recovered: native ? () => hostCall({ method: "runtimeRecovered" }) : undefined,
    });
    if (native) {
      doc.subscribe(() => {
        void hostCall({ method: "status", status: doc.status, error: doc.error }).catch(() => {});
      });
      await hostCall({ method: "ready" });
    }
  } catch (error) {
    globalThis.document.body.textContent = `Could not open this document: ${String(error)}`;
    if (native) await hostCall({ method: "failed", error: String(error) }).catch(() => {});
    throw error;
  }
}

/** Closed-document editing: the engine only. Never loads assets/app.js. */
export async function bootHeadless() {
  try {
    const { session } = await openDocument(true);
    globalThis.__slop = {
      request: (request) => session.handle(request),
      prepareClose: () => session.prepareClose(),
      cancelClose: () => session.cancelClose(),
      flush: () => session.flush(),
      close: () => session.close(),
      retrySave: async () => {
        await session.flush();
        return true;
      },
      discardPending: () => session.discardPending(),
    };
    await hostCall({ method: "ready" });
  } catch (error) {
    await hostCall({ method: "failed", error: String(error) });
  }
}
