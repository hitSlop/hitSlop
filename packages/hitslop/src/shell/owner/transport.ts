import { CoreErrorCodes } from "../../schema/constants";
import type { Batch, CoreErrorCode, OwnerState } from "../../schema/core";
import type { PagePush, PageResult } from "../../wire/page";
import { DocumentError } from "../../sdk/internal";
import { call } from "../bridge";

/** Replies acknowledge a sequence; the ordered push stream updates the snapshot. */
export interface OwnerTransport {
  readonly readOnly: boolean;
  open(): Promise<OwnerState>;
  apply(batch: Batch): Promise<PageResult<"apply">>;
  flush(): Promise<void>;
  runCommand(name: string, args: unknown): Promise<PageResult<"commands.run">>;
  undo(): Promise<PageResult<"undo">>;
  redo(): Promise<PageResult<"redo">>;
  onPush(receiver: (pushes: PagePush[]) => void): void;
}
const coreCode = (code: unknown): code is CoreErrorCode =>
  (CoreErrorCodes as readonly unknown[]).includes(code);

export function nativeTransport(
  readOnly: boolean,
): OwnerTransport & { publish(pushes: unknown): void } {
  let receiver: (pushes: PagePush[]) => void = () => {};
  return {
    readOnly,
    open: async () => JSON.parse((await call({ method: "open" })).state),
    runCommand: (name, args) => call({ method: "commands.run", name, args }),
    apply: (batch) => call({ method: "apply", batch: JSON.stringify(batch) }),
    flush: async () => {
      await call({ method: "flush" });
    },
    undo: () => call({ method: "undo" }),
    redo: () => call({ method: "redo" }),
    onPush(next) {
      receiver = next;
      (globalThis as any).__hitslopPreview?.onPush(next);
    },
    publish(pushes: unknown) {
      if (
        !Array.isArray(pushes) ||
        !pushes.every((push) => push && ["publication", "resync"].includes(push.type))
      )
        throw new DocumentError(
          "unknown_outcome",
          "Invalid owner publication; reload current state",
        );
      receiver(pushes);
    },
  };
}

/**
 * Browser development and tests: an in-page WASM core with the native reply and push
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
