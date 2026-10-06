import type { PageFailure, PageMethod, PageRequest, PageResult } from "../schema/page";
import { OutcomeCodes } from "../schema/constants";
import { DocumentError } from "../sdk/internal";

/** One native request path. WebKit correlates each reply with its returned promise. */
export async function call<M extends PageMethod>(request: PageRequest<M>): Promise<PageResult<M>> {
  let reply: unknown;
  try {
    reply = await (globalThis as any).webkit.messageHandlers.hitslop.postMessage(request);
  } catch (error) {
    throw new DocumentError("unknown_outcome", String(error));
  }
  // The host and shell share one build. Validate the outcome envelope, not every
  // payload; request validation remains in the core.
  if (typeof reply !== "object" || reply === null || Array.isArray(reply))
    throw new DocumentError("unknown_outcome", "Invalid host reply; inspect current state");
  const envelope = reply as { ok?: unknown; code?: unknown; error?: unknown };
  if (
    envelope.ok === false &&
    typeof envelope.error === "string" &&
    (OutcomeCodes as readonly unknown[]).includes(envelope.code)
  ) {
    const failure = reply as PageFailure;
    throw new DocumentError(failure.code, failure.error, failure.reason, failure.opIndex);
  }
  if (envelope.ok !== true)
    throw new DocumentError("unknown_outcome", "Invalid host reply; inspect current state");
  const { ok, ...result } = reply as { ok: true } & PageResult<M>;
  return result as PageResult<M>;
}

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
