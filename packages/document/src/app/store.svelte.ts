import type { AsyncHandle } from "../async-types";
// The Svelte adapter, compiled into each slop. It depends only on ctx.
import { getContext, mount, tick, unmount, type Component } from "svelte";
import type { Binding, SlopApp, SlopContext } from "../abi";
import type { Scope } from "../contracts";
import type {
  Handle as SyncHandle,
  ScalarHandle as SyncScalarHandle,
  TextHandle as SyncTextHandle,
} from "../handle-types";
import type { Definition, ObjectNode, Value } from "../schema";
import { schemaKey } from "../schema";
import { activate, deactivate, current, slopContext } from "./context";
const adapterContext = Symbol("hitslop.document");
/** Handle for any object from `current`: the root, a row, a nested object, a record entry or a tree node. */
export type { AsyncAt as At } from "../async-types";
export type { Issue } from "../abi";
export type { SlopContext, SlopApp } from "../abi";
export type DocumentScope<N extends ObjectNode> = Scope<N>;

/** The package entry: `export default defineSlop(App)`; the host mounts it. */
export function defineSlop(App: Component): SlopApp {
  return {
    mount(ctx, target) {
      activate(ctx);
      const adapter = createAdapter(ctx.document);
      try {
        const app = mount(App, {
          target,
          context: new Map<symbol, unknown>([
            [slopContext, ctx],
            [adapterContext, adapter.document],
          ]),
        });
        return {
          rendered: tick,
          unmount: async () => {
            try {
              await unmount(app);
            } finally {
              adapter.dispose();
              deactivate(ctx);
            }
          },
        };
      } catch (error) {
        adapter.dispose();
        deactivate(ctx);
        throw error;
      }
    },
  };
}

export type SlopDocument<N extends ObjectNode> = Pick<
  import("../abi").SlopDocument<N>,
  "current" | "status" | "error" | "issues" | "fields" | "at" | "change" | "flush"
>;
export function useDocument<N extends ObjectNode>(definition: Definition<N>): SlopDocument<N> {
  const ctx = getContext<SlopContext | undefined>(slopContext);
  const doc = ctx?.document;
  if (!doc || doc.key !== schemaKey(definition.descriptor))
    throw new Error("Host document/schema mismatch");
  const adapter = getContext<SlopDocument<N> | undefined>(adapterContext);
  if (!adapter) throw new Error("Missing mounted document adapter");
  return adapter;
}
function createAdapter<N extends ObjectNode>(doc: SlopContext["document"]) {
  let current = $state.raw<Value<N>>(doc.current as Value<N>);
  let status = $state(doc.status);
  let error = $state(doc.error);
  let issues = $state.raw(doc.issues);
  const dispose = doc.subscribe(() => {
    current = doc.current as Value<N>;
    status = doc.status;
    error = doc.error;
    issues = doc.issues;
  });
  const document: SlopDocument<N> = {
    get current() {
      return current;
    },
    get status() {
      return status;
    },
    get error() {
      return error;
    },
    get issues() {
      return issues;
    },
    fields: doc.fields as unknown as AsyncHandle<SyncHandle<N>>,
    at: doc.at,
    change: (callback, options) => doc.change(callback as any, { message: options?.message }),
    flush: () => doc.flush(),
  };
  return { document, dispose };
}

/** Svelte action: `use:bindText={doc.fields.title}`. */
export function bindText(
  element: HTMLInputElement | HTMLTextAreaElement,
  handle: AsyncHandle<SyncTextHandle>,
): Binding<AsyncHandle<SyncTextHandle>> {
  return current().bind.text(element, handle);
}
/** Svelte action: `use:bindValue={doc.fields.done}`. */
export function bindValue<V extends string | number | boolean>(
  element: HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement,
  handle: AsyncHandle<SyncScalarHandle<V>>,
): Binding<AsyncHandle<SyncScalarHandle<V>>> {
  return current().bind.value(element, handle);
}

export { default as Slop } from "./Slop.svelte";
export type { InsertResult } from "../handle-types";
export type Handle<N extends import("../schema").Node> = AsyncHandle<
  import("../handle-types").Handle<N>
>;
export type TextHandle = AsyncHandle<import("../handle-types").TextHandle>;
export type ScalarHandle<V> = AsyncHandle<import("../handle-types").ScalarHandle<V>>;
