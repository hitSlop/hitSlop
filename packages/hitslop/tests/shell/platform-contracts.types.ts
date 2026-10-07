// Compile-only boundary checks: unsafe envelopes and replies must not become `any`.
import type { SocketRequest } from "../../src/wire/socket";
import type { PageRequest } from "../../src/wire/page";
import type { SlopPageHandle } from "../../src/shell/page-handle";
import { call } from "../../src/shell/bridge";

function contracts(handle: SlopPageHandle) {
  // @ts-expect-error An edit without lifetime identity is unsafe.
  const missingEpoch: SocketRequest = { documentPath: "/doc", method: "batch", batch: { intents: [] } };
  const error: PageRequest<"pageError"> = { method: "pageError", kind: "application", error: "x" };
  // @ts-expect-error A runtime error names its kind.
  const missingKind: PageRequest<"pageError"> = { method: "pageError", error: "x" };
  // @ts-expect-error Attachment reads need an attachment ID, not bytes.
  call({ method: "attachments.read", bytes: "AA==" });
  call({ method: "config" }).then(reply => {
    const descriptor: object = reply.descriptor;
    // @ts-expect-error Method inference must not widen the reply to any.
    const bytes: string = reply.bytes;
  });
}
