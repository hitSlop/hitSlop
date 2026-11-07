// Freeze this context's behavior with the first released runtime ABI.
import type { PopContext } from "../../sdk/abi";
import type { ObjectNode } from "../../sdk/schema";
import type { OwnerDocument as Document } from "../owner/document";
import type { ownerAttachments } from "../attachments";
import type { ownerSpeech } from "../speech";

/** The app-facing interface over this page's document and host services. The core refused
 * an app needing a newer runtime ABI before this page opened. */
export function createContext(
  doc: Document<ObjectNode>,
  options: {
    attachments: ReturnType<typeof ownerAttachments>;
    speech: ReturnType<typeof ownerSpeech>;
    haptics: PopContext["haptics"];
    resize(size: { width: number; height: number }): Promise<void>;
    reportError(error: unknown): void;
    environment: PopContext["environment"];
  },
): PopContext {
  const { attachments } = options;
  const document = Object.freeze({
    get current() {
      return doc.current;
    },
    fields: doc.fields,
    at: ((value: any) => doc.at(value)) as PopContext["document"]["at"],
    change: ((callback: (tx: any) => unknown) =>
      doc.change(callback)) as PopContext["document"]["change"],
    flush: () => doc.flush(),
    undo: () => doc.undo(),
    redo: () => doc.redo(),
    subscribe: (listener: Parameters<typeof doc.subscribe>[0]) => doc.subscribe(listener),
    observe: (read: () => void) => doc.observe(read),
  });
  return Object.freeze({
    document,
    environment: options.environment,
    bind: Object.freeze({ text: doc.bindText.bind(doc) }),
    attachments: Object.freeze(attachments),
    speech: Object.freeze(options.speech),
    haptics: Object.freeze(options.haptics),
    window: Object.freeze({ resize: options.resize }),
    reportError: options.reportError,
  });
}
