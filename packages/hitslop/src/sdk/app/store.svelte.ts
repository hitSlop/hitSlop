// The Svelte adapter, compiled into each slop. It depends only on ctx.
import { mount, tick, unmount, type Component } from "svelte";
import type { Binding, SlopApp, SlopContext } from "../abi";
import type { TextHandle } from "../handle-types";
import type { Definition, LiveDocument, ObjectNode, Value } from "../schema";
import { activate, deactivate, current } from "./context";
import { bindCommands } from "../commands";
import Root from "./Root.svelte";
export type { SlopApp, SlopContext } from "../abi";
export { attachments, type AttachmentInfo, type AttachmentRef } from "./attachments";
export { capture, type CaptureMode, type CaptureTarget } from "./capture";
export { default as EditableText } from "./EditableText.svelte";

/**
 * A Svelte app's entry: `export default svelteApp(App, { schema })`; the host mounts it.
 * The generated entry passes the default exports of `schema.ts`, `Export.svelte` and
 * `Icon.svelte`. Once mounted, `schema` is the live document.
 */
export function svelteApp(App: Component, options: {
  schema: Definition<ObjectNode>;
  commands?: Record<string, unknown>;
  export?: Component<{ mode: "preview" | "export" }>;
  icon?: Component;
}): SlopApp {
  bindCommands(options.commands ?? {}, options.schema);
  return {
    descriptor: options.schema.descriptor,
    mount(ctx, target) {
      const adapter = createAdapter(ctx.document);
      activate(ctx, options.schema, adapter.document);
      const release = () => {
        adapter.dispose();
        deactivate(ctx);
      };
      try {
        const app = mount(Root, { target, props: { App, Export: options.export, Icon: options.icon } });
        return {
          rendered: tick,
          unmount: async () => {
            try {
              await unmount(app);
            } finally {
              release();
            }
          },
        };
      } catch (error) {
        release();
        throw error;
      }
    },
  };
}

function createAdapter(doc: SlopContext["document"]) {
  let current = $state.raw(doc.current);
  const unsubscribe = doc.subscribe(() => {
    current = doc.current;
  });
  // A handle's `value` derives from the snapshot, so reading it depends on `current`.
  const unobserve = doc.observe(() => void current);
  const dispose = () => {
    unobserve();
    unsubscribe();
  };
  const document: LiveDocument<ObjectNode> = {
    get current() {
      return current as Value<ObjectNode>;
    },
    fields: doc.fields,
    at: doc.at,
    change: (callback) => doc.change(callback),
    flush: () => doc.flush(),
    undo: () => doc.undo(),
    redo: () => doc.redo(),
  };
  return { document, dispose };
}

/** Svelte action for text fields: `use:bindText={doc.fields.title}`. It keeps what the
 * person types, merges edits made elsewhere, and survives IME composition. Scalars use
 * `bind:value={doc.fields.done.value}` instead. For text in a list's rows, use
 * `EditableText`, which mounts a field only while it is edited. */
export function bindText(
  element: HTMLInputElement | HTMLTextAreaElement,
  handle: TextHandle,
): Binding<TextHandle> {
  return current().bind.text(element, handle);
}

/** Request a window content size; ignored where the host has no window. */
export function resizeWindow(size: { width: number; height: number }): Promise<void> {
  return current().window.resize(size);
}
