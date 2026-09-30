import { Check } from "typebox/value";
import { PageReplySchema } from "@hitslop/schema/owner";
import type {
  Batch,
  EditText,
  OwnerState,
  PageErrorCode,
  PagePush,
  PageRequest,
} from "@hitslop/schema/owner";
import { OwnerError } from "../errors";
import type { TextReply } from "./text";

export type Opened = { state: OwnerState; savedSequence: number; saveFailure: string | null };
/** The page's only path to the owner. Replies never carry document state; pushes do. */
export interface OwnerTransport {
  readonly id?: string;
  readonly readOnly?: boolean;
  open(): Promise<Opened>;
  apply(batch: Batch): Promise<{ sequence: number; ids: string[] }>;
  text(request: EditText): Promise<TextReply>;
  flush(): Promise<void>;
  /** Receives ordered host pushes. Set once, before `open`. */
  onPush(receiver: (pushes: PagePush[]) => void): void;
}

type Distribute<T> = T extends unknown ? Omit<T, "id" | "view"> : never;

/** The native owner, reached through the `owner` message handler. */
export function nativeTransport(id: string, view: string, readOnly = false): OwnerTransport {
  const call = async (request: Distribute<PageRequest>): Promise<any> => {
    const message = { ...request, id: crypto.randomUUID(), view } as PageRequest;
    const reply = await (globalThis as any).webkit.messageHandlers.owner.postMessage(message);
    if (!Check(PageReplySchema, reply) || reply.id !== message.id)
      throw new OwnerError("unknown_outcome", "Invalid owner reply; inspect current state");
    if (!reply.ok) throw new OwnerError(reply.code, reply.error);
    return reply;
  };
  return {
    id,
    readOnly,
    open: async () => {
      const reply = await call({ method: "open" });
      return { state: reply.state, savedSequence: reply.savedSequence, saveFailure: reply.saveFailure ?? null };
    },
    apply: async (batch) => {
      const reply = await call({ method: "apply", batch });
      return { sequence: reply.sequence, ids: reply.ids };
    },
    text: async (request) => call({ method: "text", request }),
    flush: async () => {
      await call({ method: "flush" });
    },
    onPush(receiver) {
      // Swift delivers pushes in order through one awaited call per batch.
      (globalThis as any).__hitslop = { publish: (pushes: PagePush[]) => receiver(pushes) };
    },
  };
}

const codeOf = (error: unknown): PageErrorCode => {
  const code = String(error).split(":")[0]?.trim();
  return code === "owner_invalidated" ? code : "rejected";
};

/**
 * Browser development and tests: an in-page WASM core with the native reply and push
 * ordering. Replies resolve before their publication is pushed, as they can natively.
 */
export function wasmTransport(core: any, id: string = crypto.randomUUID()): OwnerTransport {
  let receiver: (pushes: PagePush[]) => void = () => {};
  let queue: PagePush[] = [];
  let scheduled = false;
  const push = (item: PagePush) => {
    queue.push(item);
    if (scheduled) return;
    scheduled = true;
    setTimeout(() => {
      scheduled = false;
      const batch = queue;
      queue = [];
      receiver(batch);
    }, 0);
  };
  const run = <T>(action: () => T): T => {
    try {
      return action();
    } catch (error) {
      throw new OwnerError(codeOf(error), String(error));
    }
  };
  return {
    id,
    open: async () => ({ state: JSON.parse(core.snapshot()), savedSequence: core.sequence(), saveFailure: null }),
    apply: async (batch) => {
      const applied = run(() => core.applyBatch(JSON.stringify(batch)));
      push({ type: "publication", publication: JSON.parse(applied.publication) });
      return { sequence: applied.sequence, ids: [...applied.ids] };
    },
    text: async (request) => {
      const edit = run(() => core.editText(JSON.stringify(request)));
      if (edit.publication) push({ type: "publication", publication: JSON.parse(edit.publication) });
      return {
        sequence: edit.sequence,
        authored: edit.authored,
        selectionStart: edit.selectionStart,
        selectionEnd: edit.selectionEnd,
      };
    },
    flush: async () => {
      push({ type: "saved", sequence: core.sequence() });
    },
    onPush(next) {
      receiver = next;
    },
  };
}

/** Only disposable browser development creates a WASM owner. */
export async function browserTransport(descriptor: unknown, initial: unknown): Promise<OwnerTransport> {
  const module = await import(new URL("./core/hitslop_core_wasm.js", import.meta.url).href);
  await module.default({
    module_or_path: new URL("./core/hitslop_core_wasm_bg.wasm", import.meta.url).href,
  });
  return wasmTransport(module.WasmDocument.create(JSON.stringify(descriptor), JSON.stringify(initial)));
}
