import type { PageFailure, PageMethod, PageRequest, PageResult } from "../wire/page";
import { OutcomeCodes } from "../schema/constants";
import { DocumentError } from "../sdk/internal";
import { preview } from "./preview";

let browserBridge: ((request: unknown) => Promise<string>) | undefined;
/** Installed only by the host frame bootstrap, before authored code loads. */
export function installBrowserBridge(bridge: (request: unknown) => Promise<string>) { browserBridge = bridge; }
export const hasBrowserBridge = () => Boolean(browserBridge);

/** One native request path. WebKit correlates each reply with its returned promise. */
export async function call<M extends PageMethod>(request: PageRequest<M>): Promise<PageResult<M>> {
  let reply: unknown;
  try {
    if (browserBridge) reply = await browserBridge(request);
    else if (preview.host) reply = await preview.host.request(request);
    else {
      const handler = globalThis.webkit?.messageHandlers?.hitslop;
      if (!handler) throw new Error("Native document bridge is unavailable");
      reply = await handler.postMessage(JSON.stringify(request));
    }
  } catch (error) {
    throw new DocumentError("unknown_outcome", String(error));
  }
  try {
    if (typeof reply !== "string") throw new Error("Expected JSON reply");
    reply = JSON.parse(reply);
  } catch {
    throw new DocumentError("unknown_outcome", "Invalid host reply; inspect current state");
  }
  // The host and shell share one build. Validate the outcome envelope, not every
  // payload; request validation remains in the core.
  if (typeof reply !== "object" || reply === null || Array.isArray(reply))
    throw new DocumentError("unknown_outcome", "Invalid host reply; inspect current state");
  const envelope = reply as { ok?: unknown; code?: unknown; error?: unknown; method?: unknown };
  if (
    envelope.ok === false &&
    typeof envelope.error === "string" &&
    (OutcomeCodes as readonly unknown[]).includes(envelope.code)
  ) {
    const failure = reply as PageFailure;
    throw new DocumentError(failure.code, failure.error, failure.reason, failure.opIndex);
  }
  if (envelope.ok !== true || envelope.method !== request.method)
    throw new DocumentError("unknown_outcome", "Invalid host reply; inspect current state");
  const { ok, method, ...result } = reply as { ok: true; method: M } & PageResult<M>;
  return result as PageResult<M>;
}

// Safari 18.2+ has native base64 on Uint8Array; the fallbacks avoid per-byte callbacks.
const encode = (bytes: Uint8Array) => {
  if (bytes.toBase64) return bytes.toBase64();
  const chunks: string[] = [];
  for (let offset = 0; offset < bytes.length; offset += 16_384)
    chunks.push(String.fromCharCode(...bytes.subarray(offset, offset + 16_384)));
  return btoa(chunks.join(""));
};
const decode = (text: string) => {
  if (Uint8Array.fromBase64) return Uint8Array.fromBase64(text);
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
};
export const base64 = { encode, decode };
