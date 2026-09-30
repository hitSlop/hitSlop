// Compile-only boundary checks: unsafe envelopes and replies must not become `any`.
import type { SocketRequest } from "@hitslop/schema/socket";
import type { BridgeRequest } from "@hitslop/schema/bridge";
import type { SlopRuntimeHandle } from "../src/runtime-handle";
import { hostCall } from "../src/bridge";

function contracts(handle: SlopRuntimeHandle) {
  // @ts-expect-error An edit without lifetime identity is unsafe.
  const missingEpoch: SocketRequest = { id: "edit", documentPath: "/doc", method: "apply", op: {} };
  const error: BridgeRequest<"runtimeError"> = { method: "runtimeError", kind: "application", error: "x" };
  // @ts-expect-error A runtime error names its kind.
  const missingKind: BridgeRequest<"runtimeError"> = { method: "runtimeError", error: "x" };
  // @ts-expect-error Attachment reads need an attachment ID, not bytes.
  hostCall({ method: "attachments.read", bytes: "AA==" });
  hostCall({ method: "attachments.list" }).then(reply => {
    const count: number = reply.files.length;
    // @ts-expect-error A listing does not return config fields.
    const epoch: string = reply.epoch;
  });
  hostCall({ method: "config" }).then(reply => {
    const epoch: string = reply.epoch;
    const view: string = reply.view;
    // @ts-expect-error Method inference must not widen the reply to any.
    const bytes: string = reply.bytes;
  });
}
