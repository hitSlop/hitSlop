// Compile-only boundary checks: unsafe envelopes and replies must not become `any`.
import type { SocketRequest } from "@hitslop/schema/socket";
import type { PageRequest } from "@hitslop/schema/page";
import type { SlopPageHandle } from "../src/page-handle";
import { call } from "../src/bridge";

function contracts(handle: SlopPageHandle) {
  // @ts-expect-error An edit without lifetime identity is unsafe.
  const missingEpoch: SocketRequest = { documentPath: "/doc", method: "batch", ops: "[]" };
  const error: PageRequest<"pageError"> = { method: "pageError", kind: "application", error: "x" };
  // @ts-expect-error A runtime error names its kind.
  const missingKind: PageRequest<"pageError"> = { method: "pageError", error: "x" };
  // @ts-expect-error Attachment reads need an attachment ID, not bytes.
  call({ method: "attachments.read", bytes: "AA==" });
  call({ method: "attachments.read", attachmentID: "a".repeat(64) }).then(reply => {
    const bytes: string = reply.bytes;
    // @ts-expect-error An attachment read does not return config fields.
    const epoch: string = reply.epoch;
  });
  call({ method: "config" }).then(reply => {
    const descriptor: object = reply.descriptor;
    // @ts-expect-error Method inference must not widen the reply to any.
    const bytes: string = reply.bytes;
  });
}
