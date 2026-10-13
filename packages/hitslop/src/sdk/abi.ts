import type { LiveDocument } from "./schema";
/**
 * The interface between a built slop and the page shell. A package's `assets/ui.js`
 * default-exports a `SlopApp`; the shell opens the document, then calls
 * `mount(ctx, target)`. Apps reach the host only through `ctx`.
 *
 * Every released ui.js keeps running against later shells. `ctx` grows additively;
 * a change an older app cannot run raises `RuntimeABI`, and the shell keeps the older
 * behavior for packages built at the lower level. New members of object-like handles
 * start with `$`, which no field name can, so they never shadow an author's fields.
 *
 * The ABI is more than `ctx`: an app's bundles carry SDK code that meets the host here.
 * Each of these is released behavior and changes only behind a raised RuntimeABI:
 * - Page boot: the shell imports `/assets/ui.js` (default export `SlopApp`) and links
 *   `/assets/ui.css` when present; it mounts the app only when `SlopApp.descriptor` equals
 *   the file's stored descriptor as JSON, so the host passes that descriptor unnormalized.
 * - Command program (`commands.js`): it sets `globalThis.__slopCommands` to a set
 *   branded with `Symbol.for("slop.command-set")` → `{ document, specs }`, and sets
 *   `__slopDescribe`; specs map names to `{ description, args, run }`. The ABI's evaluator prelude
 *   defines `__slopRun(json)` and replies `{ ok, intents, result }` or `{ ok: false,
 *   error }`, with intents in the core's vocabulary for that ABI.
 * - Errors: `Symbol.for("slop.operation-error")` brands document outcomes; apps treat an
 *   unknown `code` or `reason` as an outcome. `refuse()` throws a DocumentError with
 *   code `rejected` and reason `refused`; the evaluator replies
 *   `{ ok: false, error, refused: true }`. The shell offers an unhandled refusal to the
 *   app as a cancelable `slop:refused` event on `document` whose `detail` is the
 *   message; an app that cancels it has shown the message, otherwise it is reported.
 *   `Symbol.for("slop.speech-error")` brands a `SpeechError` from `ctx.speech`; its
 *   `code` and `reason` may grow the same way.
 * - Markup the host styles: `[data-slop-root]` (the app's root, sized to the window),
 *   `[data-slop-capture-target]` (the wrapper of a static view's markup, child of
 *   `<body>`), `[data-slop-export="hide"]` (hidden in captures), and the attributes the
 *   host sets on `<html>`: `data-slop-presentation`, `data-slop-capture`,
 *   `data-slop-resizable`, `data-slop-controls`. The Svelte root renders
 *   `[data-slop-notice]`, a status region holding the current notice as a `<p>`.
 * - CSS variables: `--slop-<token>` for each theme color and `--slop-window-radius`,
 *   `--slop-window-width` and `--slop-window-height`.
 * - Host-internal, not app ABI (the shell and host ship together and may change them):
 *   `slop:render-error` (the shell telling itself a remount failed), the
 *   `style[data-slop-host]` sheet, the `__slop` host entry point, and `__slopPreview`
 *   with the `slop:*` messages of the `slop dev` preview transport. Apps never read them.
 *
 * Lifecycle decisions: mount/rendered/unmount remain framework-neutral. Awaited writes
 * expose the accepted immutable snapshot, not a promise of DOM paint. subscribe allows
 * multiple listeners; observe allows one adapter, with identity-safe cleanup so an old
 * adapter cannot detach its replacement. Text merge/IME/caret behavior stays in the host.
 * Draft, motion and notice presentation stay in the SDK. They do not add host APIs.
 * Static views (Preview, Export, Icon, Widget) render from their props and never meet
 * `ctx`. Rust owns the capture kinds and shared limits. The conformance app in the frozen
 * corpus (`tests/abi/owner-svelte`) exercises these against every later shell.
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
export type { WidgetViewport as WidgetProps } from "../wire/page.generated";
/** A recording stored with `attachments.import` (its reference), or its attachment ID. */
export type SpeechClip = string | { readonly id: string };
export interface SpeechOptions {
  /** A locale identifier such as `"en-US"`; the person's own locale when absent. */
  locale?: string;
}
export interface SpeechTranscript {
  text: string;
  /** The locale the recognizer used. */
  locale: string;
}

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
  readonly attachments: {
    /** Stores the file, then runs `reference` synchronously (like `change`) to write
     * its reference; both are accepted together or the promise rejects. */
    import(
      file: File,
      reference: (tx: Scope<ObjectNode>, ref: AttachmentRef) => void | undefined,
    ): Promise<AttachmentRef>;
    url(id: string): string;
    read(id: string): Promise<Blob>;
  };
  readonly speech: {
    /** Transcribes a stored recording on this device. The host reads it by attachment ID,
     * so it never crosses the page bridge again. Rejects with a `SpeechError` (brand
     * `slop.speech-error`) when speech is unavailable, cancelled or failed. */
    transcribe(clip: SpeechClip, options?: SpeechOptions): Promise<SpeechTranscript>;
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

export interface SlopDocument<N extends ObjectNode> extends LiveDocument<N> {
  /** Observes accepted snapshot updates; returns an idempotent disposer. */
  subscribe(listener: () => void): () => void;
  /** Called whenever a scalar handle's `value` is read, so a framework adapter can
   * record the dependency (Svelte reads its own signal here). Replaces the active
   * observer; its idempotent disposer only removes that same observer. */
  observe(read: () => void): () => void;
}
export type { Definition };
