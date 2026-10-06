import type { LiveDocument } from "./schema";
/**
 * The interface between a built slop and the page shell. A package's `assets/app.js`
 * default-exports a `SlopApp`; the shell opens the document, then calls
 * `mount(ctx, target)`. Apps reach the host only through `ctx`.
 *
 * Every released app.js keeps running against later shells. `ctx` grows additively;
 * a change an older app cannot run raises `RuntimeABI`, and the shell keeps the older
 * behavior for packages built at the lower level. New members of object-like handles
 * start with `$`, which no field name can, so they never shadow an author's fields.
 */
/** The runtime ABI this SDK's app-side code needs; `slop build` stamps it. */
export { RuntimeABI } from "@hitslop/schema/constants";
import type { At, Handle, TextHandle } from "./handle-types";
import type { Definition, ObjectNode } from "./schema";
/** A `change()` transaction: the same handles, collecting synchronously. */
export type Scope<N extends ObjectNode> = {
  readonly fields: Handle<N, "tx">;
  readonly at: At<"tx">;
};
export type Issue = import("@hitslop/schema/core").OwnerState["issues"][number];
export type AttachmentInfo = import("@hitslop/schema/values").AttachmentInfo;
export type AttachmentRef = AttachmentInfo & { name: string; mimeType: string };
export type CaptureMode = "preview" | "export" | "icon";

export interface SlopApp {
  /** The document descriptor the app was built for (`svelteApp` declares its schema's); the
   * shell refuses to mount it on a document of another. */
  readonly descriptor?: object;
  mount(ctx: SlopContext, target: HTMLElement): SlopView | Promise<SlopView>;
}
/** `rendered` resolves after pending framework updates reach the DOM. */
export interface SlopView {
  rendered?(): void | Promise<void>;
  unmount?(): void | Promise<void>;
}

export interface SlopContext {
  readonly document: SlopDocument<ObjectNode>;
  readonly bind: {
    /** Two-way text binding with IME composition and remote-edit transforms. */
    text(element: HTMLInputElement | HTMLTextAreaElement, handle: TextHandle): Binding<TextHandle>;
  };
  readonly capture: {
    /** True inside the host's export renderer. */
    isRenderer(): boolean;
    onPrepare(
      handler: (mode: CaptureMode, signal: AbortSignal) => void | Promise<void>,
    ): () => void;
    registerTarget(kind: "icon" | "export", target: CaptureTarget): () => void;
  };
  readonly attachments: {
    /** Stores the file, then runs `reference` synchronously (like `change`) to write
     * its reference; both are accepted together or the promise rejects. */
    import(
      file: File,
      reference: (tx: Scope<ObjectNode>, ref: AttachmentRef) => void,
    ): Promise<AttachmentRef>;
    read(id: string, options?: { type?: string }): Promise<Blob>;
  };
  readonly window: {
    /** Request a window content size; ignored where the host has no window. */
    resize(size: { width: number; height: number }): Promise<void>;
  };
  /** Report an application render or logic failure to the host. */
  reportError(error: unknown): void;
}

export interface Binding<H> {
  update(next: H): void;
  destroy(): void;
}
export interface CaptureTarget {
  element: HTMLElement;
  prepare(): void | Promise<void>;
  restore(): void | Promise<void>;
}

export interface SlopDocument<N extends ObjectNode> extends LiveDocument<N> {
  subscribe(listener: () => void): () => void;
  /** Called whenever a scalar handle's `value` is read, so a framework adapter can
   * record the dependency (Svelte reads its own signal here). One observer at a time. */
  observe(read: () => void): () => void;
}
export type { Definition };
