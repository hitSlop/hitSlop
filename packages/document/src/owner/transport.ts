import { OwnerError } from "../errors";
import { Check } from "typebox/value";
import { OwnerReplySchema } from "@hitslop/schema/owner";
import type {
  OwnerRequest,
  OwnerPublication,
  OwnerState,
  OwnerTextRequest,
} from "@hitslop/schema/owner";
import type { OwnerTransport } from "./document";

/** Native pages carry intents and publications, never CRDT bytes. */
export function nativeTransport(id: string, readOnly = false): OwnerTransport {
  let session = "";
  const call = async (request: OwnerRequest): Promise<any> => {
    const reply = await (globalThis as any).webkit.messageHandlers.owner.postMessage(request);
    if (!Check(OwnerReplySchema, reply) || reply.id !== request.id)
      throw new OwnerError("unknown_outcome", "Invalid owner reply; inspect current state");
    if (!reply.ok) throw new OwnerError(reply.code, reply.error);
    return reply;
  };
  return {
    id,
    readOnly,
    state: async () => {
      const { state } = await call({ method: "state", id: crypto.randomUUID() });
      session = state.session;
      return state as OwnerState;
    },
    apply: async (request) =>
      (await call({ method: "apply", ...request })).publication as OwnerPublication,
    text: async (request) =>
      (await call({ method: "text", id: crypto.randomUUID(), session: request.session, request }))
        .publication as OwnerPublication,
    releaseDraft: async (draft) => {
      await call({ method: "releaseDraft", id: crypto.randomUUID(), session, draft });
    },
    flush: async () => {
      await call({ method: "flush", id: crypto.randomUUID() });
    },
  };
}

/** Only disposable browser development creates a WASM owner. */
export async function browserTransport(
  descriptor: unknown,
  initial: unknown,
): Promise<OwnerTransport> {
  const module = await import(new URL("./core/hitslop_core_wasm.js", import.meta.url).href);
  await module.default({
    module_or_path: new URL("./core/hitslop_core_wasm_bg.wasm", import.meta.url).href,
  });
  const core = module.WasmDocument.create(JSON.stringify(descriptor), JSON.stringify(initial));
  return {
    id: crypto.randomUUID(),
    state: async () => JSON.parse(core.snapshot()),
    apply: async ({ batch }) => JSON.parse(core.apply(JSON.stringify(batch))),
    text: async (request: OwnerTextRequest) => JSON.parse(core.text(JSON.stringify(request))),
    releaseDraft: async (id) => core.release_draft(id),
    flush: async () => {},
  };
}
