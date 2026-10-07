import type { LiveDocument } from "./schema";
/**
 * The interface between a built slop and the page shell. A package's `assets/ui.js`
 * default-exports a `SlopApp`; the shell opens the document, then calls
 * `mount(ctx, target)`. Apps reach the host only through `ctx`.
 *
 * Every released app.js keeps running against later shells. `ctx` grows additively;
 * a change an older app cannot run raises `RuntimeABI`, and the shell keeps the older
 * behavior for packages built at the lower level. New members of object-like handles
 * start with `$`, which no field name can, so they never shadow an author's fields.
 *
 * The ABI is more than `ctx`: an app's bundles carry SDK code that meets the host here.
 * Each of these is released behavior and changes only behind a raised RuntimeABI:
 * - Page boot: the shell imports `/assets/ui.js` (default export `SlopApp`) and links
 *   `/assets/ui.css` when present; it mounts the app only when `SlopApp.descriptor` equals
 *   the file's stored descriptor as JSON, so the host passes that descriptor unnormalized.
 * - Command program (`commands.js`): it sets `globalThis.__slopCommands` (name → command)
 *   and `__hitslopDescribe`; a command carries `Symbol.for("hitslop.command")` →
 *   `{ definition, spec: { description, args, run }, name }`. The ABI's evaluator prelude
 *   defines `__hitslopRun(json)` and replies `{ ok, intents, result }` or `{ ok: false,
 *   error }`, with intents in the core's vocabulary for that ABI.
 * - Errors: `Symbol.for("hitslop.operation-error")` brands refusals; apps treat an
 *   unknown `code` or `reason` as an outcome.
 * - Markup the host styles: `[data-hitslop-root]` (the app's root, sized to the window),
 *   `[data-slop-capture-target]` and `[data-hitslop-active-target]` (capture targets,
 *   children of `<body>`), `[data-slop-export="hide"]` (hidden in captures), and the
 *   attributes the host sets on `<html>`: `data-slop-presentation`, `data-slop-capture`,
 *   `data-slop-resizable`, `data-slop-renderer`.
 * - CSS variables: `--slop-<token>` for each theme color and `--slop-window-radius`,
 *   `--slop-window-width` and `--slop-window-height`.
 * The conformance app in the frozen corpus (`tests/abi/owner-svelte`) exercises these
 * against every later shell.
 */
/** The runtime ABI this SDK's app-side code needs; `slop build` stamps it. */
export { RuntimeABI } from "../schema/constants";
import type { At, Handle, TextHandle } from "./handle-types";
import type { Definition, ObjectNode } from "./schema";
/** A `change()` transaction: the same handles, collecting synchronously. */
export type Scope<N extends ObjectNode> = {
  readonly fields: Handle<N, "tx">;
  readonly at: At<"tx">;
};
export type AttachmentInfo = import("../schema/values").AttachmentInfo;
export type AttachmentRef = AttachmentInfo & { name: string; mimeType: string };
export type CaptureMode = "preview" | "export" | "icon";

export interface SlopApp {
  /** The document descriptor the app was built for (`svelteApp` declares its schema's); the
   * shell refuses to mount it on a document of another. */
  readonly descriptor: object;
  /** Runs once the document is open, and again after Reload Interface unmounts the
   * previous view. */
  mount(ctx: SlopContext, target: HTMLElement): SlopView | Promise<SlopView>;
}
/** `rendered` resolves after pending framework updates reach the DOM. `unmount` runs
 * after the document has saved and closed: it stops transient work such as timers, and
 * writes made then are refused with `closing`. */
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
    url(id: string): string;
    read(id: string): Promise<Blob>;
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
  /** A named synchronous action; the host applies one guarded undo step. */
  runCommand<R>(name: string, args: unknown): Promise<R>;
  subscribe(listener: () => void): () => void;
  /** Called whenever a scalar handle's `value` is read, so a framework adapter can
   * record the dependency (Svelte reads its own signal here). One observer at a time. */
  observe(read: () => void): () => void;
}
export type { Definition };
