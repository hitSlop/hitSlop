// The Svelte adapter, compiled into each slop. It depends only on ctx.
import { mount, tick, unmount, type Component } from "svelte";
import type { SlopApp, SlopContext } from "../abi";
import type { Definition, LiveDocument, ObjectNode, Value } from "../schema";
import { activate, deactivate, current } from "./context";
import { documentContext } from "./document";
import { environmentContext } from "./environment.svelte";
import Root from "./Root.svelte";
export type { WidgetProps } from "../abi";
export { getDocument } from "./document";
export { getEnvironment, type EnvironmentSnapshot } from "./environment.svelte";
export type { CaptureMode, CaptureProps } from "./capture-props";
export { attachments, type AttachmentInfo, type AttachmentRef } from "./attachments";
export {
  speech,
  isSpeechError,
  type SpeechError,
  type SpeechClip,
  type SpeechOptions,
  type SpeechTranscript,
  type SpeechUnavailableReason,
} from "./speech";
export { default as EditableText } from "./EditableText.svelte";
export { notify } from "./notice.svelte";
export { draft } from "./draft.svelte";
export { motion } from "./motion.svelte";
export { bindText } from "./bind-text";

/**
 * The generated entry adapts App.svelte to the host. Each mount gives App's components
 * their own reactive document, which they acquire with `getDocument(schema)`.
 */
export function svelteApp(App: Component, options: { schema: Definition<ObjectNode> }): SlopApp {
  return {
    descriptor: options.schema.descriptor,
    mount(ctx, target) {
      const adapter = createAdapter(ctx.document);
      const environment = environmentContext(ctx.environment);
      activate(ctx);
      const release = () => {
        adapter.dispose();
        environment.dispose();
        deactivate(ctx);
      };
      try {
        const app = mount(Root, {
          target,
          props: { App },
          context: new Map<symbol, unknown>([
            ...documentContext(options.schema, adapter.document),
            ...environment.context,
          ]),
        });
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

/** Request a window content size; ignored where the host has no window. */
export function resizeWindow(size: { width: number; height: number }): Promise<void> {
  return current().window.resize(size);
}
