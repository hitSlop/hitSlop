/**
 * The frozen boundary between a built slop and the hitSlop runtime.
 *
 * A package's `assets/app.js` default-exports a `SlopApp`. The runtime opens the
 * document, then calls `mount(ctx, target)`. Apps import nothing from the runtime;
 * everything they may rely on at run time is reachable from `ctx`.
 *
 * Compatibility rule: this interface only grows. Existing members keep their
 * arguments, results, timing and errors. New capabilities are optional and
 * advertised in `capabilities`. Behavior is pinned by the sealed consumer
 * bundles in tests/abi, not by this declaration alone.
 */
import type { DocumentEvent, Issue } from "./document";
import type { At, Handle, ScalarHandle, TextHandle } from "./handles";
import type { Definition, ObjectNode, Value } from "./schema";
import type { AttachmentInfo, AttachmentRef } from "./attachments";
import type { CaptureMode } from "./capture";
import type { Scope } from "./document";

export const ABI = 1;

export interface SlopApp {
  mount(ctx: SlopContext, target: HTMLElement): SlopView | Promise<SlopView>;
}
/** `rendered` resolves after pending framework updates reach the DOM. */
export interface SlopView {
  rendered?(): void | Promise<void>;
  unmount?(): void | Promise<void>;
}

export interface SlopContext {
  readonly abi: typeof ABI;
  /** Optional features beyond ABI 1, such as future "presence" or "sync". */
  readonly capabilities: readonly string[];
  readonly document: SlopDocument<ObjectNode>;
  readonly bind: {
    /** Two-way text binding with IME composition and remote-edit transforms. */
    text(element: HTMLInputElement | HTMLTextAreaElement, handle: TextHandle): Binding<TextHandle>;
    /** Checkbox, range, number, text-like input or select bound to a scalar. */
    value<V extends string | number | boolean>(
      element: HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement,
      handle: ScalarHandle<V>,
    ): Binding<ScalarHandle<V>>;
  };
  readonly capture: {
    /** True inside the host's export renderer. */
    isRenderer(): boolean;
    onPrepare(handler: (mode: CaptureMode, signal: AbortSignal) => void | Promise<void>): () => void;
    registerTarget(kind: "icon" | "export", target: CaptureTarget): () => void;
  };
  readonly attachments: {
    import(file: File, options: { commit(ref: AttachmentRef): void }): Promise<AttachmentRef>;
    read(id: string, options?: { type?: string }): Promise<Blob>;
    list(): Promise<AttachmentInfo[]>;
  };
  readonly theme: {
    get(): { defaults: Record<string, string>; overrides: Record<string, string>; effective: Record<string, string> };
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
  readonly status: "saved" | "saving" | "save-failed";
  readonly error: string | null;
  /** Merged-state anomalies; stored values are preserved, never repaired. */
  readonly issues: readonly Issue[];
  /** The last save exceeded capacity. Edits remain live; retry saving or discard through the host. */
  readonly full: boolean;
  readonly fields: Handle<N>;
  readonly at: At;
  /** One synchronous, all-or-nothing commit. */
  change<R>(callback: (tx: Scope<N>) => R, options?: { message?: string }): R;
  /** Durability barrier: commits drafts and previews, then waits for storage. */
  flush(): Promise<void>;
  subscribe(listener: (event: DocumentEvent) => void): () => void;
}
export type { DocumentEvent, Issue, Definition };
