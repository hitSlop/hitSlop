import type {
  BridgeMethod as Method,
  BridgeRequest as Message,
  BridgeReply as Result,
} from "@hitslop/schema/bridge";
import { OperationRejectedError } from "./errors";
// Safari 18.2+ has native base64 on Uint8Array; the fallbacks avoid per-byte callbacks.
const native = Uint8Array as unknown as {
  fromBase64?: (text: string) => Uint8Array;
  prototype: { toBase64?: () => string };
};
const encode = (bytes: Uint8Array) => {
  if (native.prototype.toBase64) return (bytes as any).toBase64() as string;
  const chunks: string[] = [];
  for (let offset = 0; offset < bytes.length; offset += 16_384)
    chunks.push(String.fromCharCode(...bytes.subarray(offset, offset + 16_384)));
  return btoa(chunks.join(""));
};
const decode = (text: string) => {
  if (native.fromBase64) return native.fromBase64(text);
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
};
export const base64 = { encode, decode };
/** Native replies `{ rejected }` for refusals that retrying cannot fix; thrown errors stay retryable. */
export async function hostCall<M extends Method>(args: Message<M>): Promise<Result<M>> {
  const reply = await (globalThis as any).webkit.messageHandlers.storage.postMessage(args);
  if (reply && typeof reply.rejected === "string") throw new OperationRejectedError(reply.rejected);
  return reply;
}
