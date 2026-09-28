import type { AsyncHandle, AsyncAt } from "./async-types";
import type { OwnerState } from "@hitslop/schema/owner";
/**
 * The interface between a built slop and the page shell. A package's `assets/app.js`
 * default-exports a `SlopApp`; the shell opens the document, then calls
 * `mount(ctx, target)`. Apps reach the host only through `ctx`. Both sides are built
 * from this repository; there is no versioned compatibility promise before release.
 */
export type Issue = OwnerState["issues"][number];
import type { Handle, ScalarHandle, TextHandle } from "./handle-types";
import type { Definition, ObjectNode, Value } from "./schema";
import type { AttachmentInfo, AttachmentRef } from "./contracts";
import type { CaptureMode } from "./contracts";
import type { Scope } from "./contracts";

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
    text(
      element: HTMLInputElement | HTMLTextAreaElement,
      handle: AsyncHandle<TextHandle>,
    ): Binding<AsyncHandle<TextHandle>>;
    /** Checkbox, range, number, text-like input or select bound to a scalar. */
    value<V extends string | number | boolean>(
      element: HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement,
      handle: AsyncHandle<ScalarHandle<V>>,
    ): Binding<AsyncHandle<ScalarHandle<V>>>;
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
    import(
      file: File,
      options: { commit(ref: AttachmentRef): void | Promise<void> },
    ): Promise<AttachmentRef>;
    read(id: string, options?: { type?: string }): Promise<Blob>;
    list(): Promise<AttachmentInfo[]>;
  };
  readonly theme: {
    get(): {
      defaults: Record<string, string>;
      overrides: Record<string, string>;
      effective: Record<string, string>;
    };
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
  /** Immutable snapshot including uncommitted previews. Unchanged rows keep identity. */
  readonly current: Value<N>;
  readonly status: "saved" | "pending" | "save-failed";
  readonly error: string | null;
  /** Merged-state anomalies; stored values are preserved, never repaired. */
  readonly issues: Readonly<OwnerState["issues"]>;
  readonly fields: AsyncHandle<Handle<N>>;
  readonly at: AsyncAt;
  /** Collect synchronously; resolve after acceptance and local publication. */
  change<R>(callback: (tx: Scope<N>) => R, options?: { message?: string }): Promise<R>;
  /** Durability barrier: commits drafts and previews, then waits for storage. */
  flush(): Promise<void>;
  subscribe(listener: () => void): () => void;
}
export type { Definition };
