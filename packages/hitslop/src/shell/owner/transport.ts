import type { Batch, OwnerState } from "../../schema/core";
import type { PagePush, PageResult } from "../../wire/page";
import { DocumentError } from "../../sdk/internal";
import { call } from "../bridge";
import { preview } from "../preview";

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

export function nativeTransport(
  readOnly: boolean,
): OwnerTransport & { publish(pushes: unknown): void } {
  let receiver: (pushes: PagePush[]) => void = () => {};
  return {
    readOnly,
    open: async () => JSON.parse((await call({ method: "open" })).state) as OwnerState,
    runCommand: (name, args) => call({ method: "commands.run", name, args }),
    apply: (batch) => call({ method: "apply", batch }),
    flush: async () => {
      await call({ method: "flush" });
    },
    undo: () => call({ method: "undo" }),
    redo: () => call({ method: "redo" }),
    onPush(next) {
      receiver = next;
      preview.host?.onPush(next);
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
