import { CoreErrorCodes, PageErrorCodes } from "@hitslop/schema/constants";
import type {
  Batch,
  CoreErrorCode,
  EditText,
  OwnerState,
  PagePush,
  PageRequest,
} from "@hitslop/schema/owner";
import { OperationRejectedError, OwnerError } from "../errors";
import { postToHost } from "../bridge-client";
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
const coreCode = (code: unknown): code is CoreErrorCode => (CoreErrorCodes as readonly unknown[]).includes(code);
const pushTypes = ["publication", "saved", "failed", "resync"];
const count = (value: unknown) => Number.isSafeInteger(value) && (value as number) >= 0;
/** The fields each successful reply must carry. */
const replies: Record<PageRequest["method"], (reply: any) => boolean> = {
  open: (r) => typeof r.state === "string" && count(r.savedSequence),
  apply: (r) => count(r.sequence) && Array.isArray(r.ids),
  text: (r) => count(r.sequence) && typeof r.authored === "string" && count(r.selectionStart) && count(r.selectionEnd),
  flush: () => true,
};

/**
 * The native owner, reached through the `owner` message handler. Document payloads cross
 * as JSON text: the core parses them, and the page parses the state it receives. The
 * host is part of this build, so replies get a shape check, not a schema walk; a
 * malformed publication fails to apply and recovers through a fresh snapshot.
 */
export function nativeTransport(id: string, view: string, readOnly = false): OwnerTransport {
  const call = async (request: Distribute<PageRequest>): Promise<any> => {
    const message = { ...request, id: crypto.randomUUID(), view } as PageRequest;
    const reply = await postToHost(message);
    if (!reply || typeof reply !== "object" || reply.id !== message.id || typeof reply.ok !== "boolean")
      throw new OwnerError("unknown_outcome", "Invalid owner reply; inspect current state");
    if (!reply.ok) {
      if (reply.code === "rejected") throw new OperationRejectedError(reply.error, coreCode(reply.reason) ? reply.reason : undefined, reply.opIndex);
      throw new OwnerError((PageErrorCodes as readonly unknown[]).includes(reply.code) ? reply.code : "unknown_outcome", String(reply.error));
    }
    if (!replies[request.method](reply)) throw new OwnerError("unknown_outcome", "Invalid owner reply; inspect current state");
    return reply;
  };
  return {
    id,
    readOnly,
    open: async () => {
      const reply = await call({ method: "open" });
      return { state: JSON.parse(reply.state), savedSequence: reply.savedSequence, saveFailure: reply.saveFailure ?? null };
    },
    apply: async (batch) => {
      const reply = await call({ method: "apply", batch: JSON.stringify(batch) });
      return { sequence: reply.sequence, ids: reply.ids };
    },
    text: async (request) => call({ method: "text", request: JSON.stringify(request) }),
    flush: async () => {
      await call({ method: "flush" });
    },
    onPush(receiver) {
      // Swift delivers pushes in order through one awaited call per batch.
      (globalThis as any).__hitslop = { publish: (pushes: unknown) => {
        if (!Array.isArray(pushes) || !pushes.every(push => push && typeof push.view === "string" && pushTypes.includes(push.type)))
          throw new OwnerError("unknown_outcome", "Invalid owner publication; reload current state");
        receiver(pushes.filter(push => push.view === view));
      } };
    },
  };
}

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
  const publish = (publication: string | undefined) => {
    if (publication) push({ view: id, type: "publication", publication: JSON.parse(publication) });
  };
  const run = <T>(action: () => T): T => {
    try {
      return action();
    } catch (error) {
      const failure = error as { code?: unknown; message?: unknown; opIndex?: number } | null;
      if (failure && coreCode(failure.code) && typeof failure.message === "string")
        throw new OperationRejectedError(`${failure.code}: ${failure.message}`, failure.code, failure.opIndex);
      throw new OwnerError("unknown_outcome", String(error));
    }
  };
  return {
    id,
    open: async () => ({ state: JSON.parse(core.state()), savedSequence: core.sequence(), saveFailure: null }),
    apply: async (batch) => {
      const applied = run(() => core.applyBatch(JSON.stringify(batch)));
      publish(applied.publication);
      return { sequence: applied.sequence, ids: [...applied.ids] };
    },
    text: async (request) => {
      const edit = run(() => core.editText(JSON.stringify(request)));
      publish(edit.publication);
      return {
        sequence: edit.sequence,
        authored: edit.authored,
        selectionStart: edit.selectionStart,
        selectionEnd: edit.selectionEnd,
      };
    },
    flush: async () => {
      push({ view: id, type: "saved", sequence: core.sequence() });
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
