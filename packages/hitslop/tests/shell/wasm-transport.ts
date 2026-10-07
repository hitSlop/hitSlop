import type { PagePush } from "../../src/wire/page";
import { DocumentError } from "../../src/sdk/internal";
import type { OwnerTransport } from "../../src/shell/owner/transport";
import { CoreErrorCodes } from "../../src/schema/constants";
import type { CoreErrorCode } from "../../src/schema/core";

const coreCode = (code: unknown): code is CoreErrorCode =>
  (CoreErrorCodes as readonly unknown[]).includes(code);
/**
 * The WASM core in Bun, with the native reply and push
 * ordering. Replies resolve before their publication is pushed, as they can natively.
 */
export function wasmTransport(core: any): OwnerTransport {
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
    if (publication) push({ type: "publication", publication: JSON.parse(publication) });
  };
  const execute = <T>(action: () => T): T => {
    try {
      const result = action();
      publish((result as { publication?: string }).publication);
      return result;
    } catch (error) {
      const failure = error as { code?: unknown; message?: unknown; opIndex?: number } | null;
      if (failure && coreCode(failure.code) && typeof failure.message === "string")
        throw new DocumentError(
          "rejected",
          `${failure.code}: ${failure.message}`,
          failure.code,
          failure.opIndex,
        );
      throw new DocumentError("unknown_outcome", String(error));
    }
  };
  return {
    readOnly: false,
    runCommand: async () => { throw new DocumentError("rejected", "Commands require the native owner"); },
    open: async () => JSON.parse(core.state()),
    apply: async (batch) => {
      const { sequence, ids, authored, selectionStart, selectionEnd } = execute(() =>
        core.applyBatch(JSON.stringify(batch)),
      );
      // A text edit's reply also carries what the page continues from.
      return authored === undefined
        ? { sequence, ids: [...ids] }
        : { sequence, ids: [...ids], authored, selectionStart, selectionEnd };
    },
    flush: async () => {},
    undo: async () => {
      const applied = execute(() => core.undo());
      return { sequence: applied.sequence };
    },
    redo: async () => {
      const applied = execute(() => core.redo());
      return { sequence: applied.sequence };
    },
    onPush(next) {
      receiver = next;
    },
  };
}
