// Freeze this context's behavior with the first released runtime ABI.
import type { SlopContext } from "../../sdk/abi";
import type { ObjectNode } from "../../sdk/schema";
import type { OwnerDocument as Document } from "../owner/document";
import type { ownerAttachments } from "../attachments";
import type { createCaptureController } from "../capture";

/** The app-facing interface over this page's document and host services. The core refused
 * an app needing a newer runtime ABI before this page opened. */
export function createContext(
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
    change: ((callback: (tx: any) => unknown) => doc.change(callback)) as SlopContext["document"]["change"],
    runCommand: <R>(name: string, args: unknown) => doc.runCommand<R>(name, args),
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
