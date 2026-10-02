/**
 * The interface between a built slop and the page shell. A package's `assets/app.js`
 * default-exports a `SlopApp`; the shell opens the document, then calls
 * `mount(ctx, target)`. Apps reach the host only through `ctx`. Both sides are built
 * from this repository; there is no versioned compatibility promise before release.
 */
import type { At, Handle, TextHandle } from "./handle-types";
import type { Definition, ObjectNode, Value } from "./schema";
/** A `change()` transaction: the same handles, collecting synchronously. */
export type Scope<N extends ObjectNode> = { readonly fields: Handle<N, "tx">; readonly at: At<"tx"> };
export type Issue = import("@hitslop/schema/owner").OwnerState["issues"][number];
export type AttachmentInfo = { id: string; byteLength: number };
export type AttachmentRef = AttachmentInfo & { name: string; mimeType: string };
export type CaptureMode = "preview" | "export" | "icon";

export interface SlopApp {
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

export interface SlopDocument<N extends ObjectNode> {
  /** Schema key; an app refuses a document of another schema. */
  readonly key: string;
  /** Logical document identity. */
  readonly id: string;
  /** Immutable snapshot. Unchanged rows keep identity. */
  readonly current: Value<N>;
  /** Merged-state anomalies; stored values are preserved, never repaired. */
  readonly issues: readonly Issue[];
  readonly fields: Handle<N>;
  readonly at: At;
  /** Collect synchronously; resolve after acceptance and local publication. */
  change<R>(callback: (tx: Scope<N>) => R): Promise<R>;
  /** Durability barrier: sends unsent text, waits for pending writes, then for storage.
   * Rejects when the save fails; the host shows save failures and offers retry. */
  flush(): Promise<void>;
  /** Edit ▸ Undo: the last step, the person's or an agent's. Sends unsent text first;
   * resolves once `current` shows the result. */
  undo(): Promise<void>;
  redo(): Promise<void>;
  subscribe(listener: () => void): () => void;
  /** Called whenever a scalar handle's `value` is read, so a framework adapter can
   * record the dependency (Svelte reads its own signal here). One observer at a time. */
  observe(read: () => void): () => void;
}
export type { Definition };
