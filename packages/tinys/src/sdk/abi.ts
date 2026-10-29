import type { LiveDocument } from "./schema";
import type { Environment } from "./environment";
/**
 * The interface between a built tiny and the page shell. A package's `assets/ui.js`
 * default-exports a `TinyApp`; the shell opens the document, then calls
 * `mount(ctx, target)`. Apps reach the host only through `ctx`.
 *
 * Every released ui.js keeps running against later shells. `ctx` grows additively;
 * a change an older app cannot run raises `RuntimeABI`, and the shell keeps the older
 * behavior for packages built at the lower level. A handle that carries an author's
 * fields (an object, a row, a record entry, an optional object) starts any member of its
 * own, now or later, with `$`, which no field name can, so none shadows a field: an
 * optional object has `$set` and `$clear`. Handles without fields (scalars, text, lists,
 * records, counters) keep plain names.
 *
 * The ABI is more than `ctx`: an app's bundles carry SDK code that meets the host here.
 * Each of these is released behavior and changes only behind a raised RuntimeABI:
 * - Page boot: the shell imports `/assets/ui.js` (default export `TinyApp`) and links
 *   `/assets/ui.css` when present; it mounts the app only when `TinyApp.descriptor` equals
 *   the file's stored descriptor as JSON, so the host passes that descriptor unnormalized.
 * - Command program (`commands.js`): it sets `globalThis.__tinyCommands` to a set
 *   branded with `Symbol.for("tiny.command-set")` → `{ document, specs }`, and sets
 *   `__tinyDescribe`; specs map names to `{ description, args, run }`. The ABI's evaluator prelude
 *   defines `__tinyRun(json)` and replies `{ ok, intents, result }` or `{ ok: false,
 *   error }`, with intents in the core's vocabulary for that ABI.
 * - Errors: `Symbol.for("tiny.operation-error")` brands document outcomes; apps treat an
 *   unknown `code` or `reason` as an outcome. `refuse()` throws a DocumentError with
 *   code `rejected` and reason `refused`; the evaluator replies
 *   `{ ok: false, error, refused: true }`. The shell offers an unhandled refusal to the
 *   app as a cancelable `tiny:refused` event on `document` whose `detail` is the
 *   message; an app that cancels it has shown the message, otherwise it is reported.
 *   `Symbol.for("tiny.speech-error")` brands a `SpeechError` from `ctx.speech`; its
 *   `code` and `reason` may grow the same way.
 * - Markup the host styles: `[data-tiny-root]` (the app's root, sized to the window),
 *   `[data-tiny-capture-target]` (the wrapper of a static view's markup, child of
 *   `<body>`), `[data-tiny-export="hide"]` (hidden in captures), and the attributes the
 *   host sets on `<html>`: `data-tiny-presentation`, `data-tiny-capture`,
 *   `data-tiny-resizable`, `data-tiny-controls`, `data-tiny-platform`,
 *   `data-tiny-device`, `data-tiny-layout` (the latter three only in editors).
 *   The Svelte root renders
 *   `[data-tiny-notice]`, a status region holding the current notice as a `<p>`.
 * - CSS variables: `--tiny-<token>` for each theme color and `--tiny-window-radius`,
 *   `--tiny-window-width` and `--tiny-window-height`. Live editors additionally expose
 *   `--tiny-host-actions-width` and `--tiny-host-actions-height`: the corner reserved
 *   for native actions in phone editors, zero in other presentations. Phone editors also
 *   set `--tiny-keyboard-inset`: the height of the bottom edge the keyboard and its
 *   toolbar cover; 0 when the keyboard is hidden and everywhere else. The phone editor
 *   runs edge to edge, so it also sets `--tiny-safe-top` and `--tiny-safe-bottom`: the
 *   status bar/notch and home indicator heights the page extends under, for the app to
 *   pad by; 0 everywhere else.
 * - Host-internal, not app ABI (the shell and host ship together and may change them):
 *   `tiny:render-error` (the shell telling itself a remount failed), the
 *   `style[data-tiny-host]` sheet, the `__tiny` host entry point, and `__tinyPreview`
 *   with the `tiny:*` messages of the `tiny dev` preview transport. Apps never read them.
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
/** The runtime ABI this SDK's app-side code needs; `tiny build` stamps it. */
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
export type { HapticImpactStyle, HapticNotification } from "../wire/page.generated";
import type { HapticImpactStyle, HapticNotification } from "../wire/page.generated";
/** Discrete tactile feedback. Unsupported hosts and disabled feedback are silent. */
export interface Haptics {
  selection(): void;
  impact(style?: HapticImpactStyle): void;
  notification(notification: HapticNotification): void;
}
export interface SpeechOptions {
  /** A locale identifier such as `"en-US"`; the person's own locale when absent. */
  locale?: string;
}
export interface SpeechTranscript {
  text: string;
  /** The locale the recognizer used. */
  locale: string;
}

export interface TinyApp {
  /** The document descriptor the app was built for (`svelteApp` declares its schema's); the
   * shell refuses to mount it on a document of another. */
  readonly descriptor: object;
  /** Runs once the document is open, and again after Reload Interface unmounts the
   * previous view. */
  mount(ctx: TinyContext, target: HTMLElement): TinyView | Promise<TinyView>;
}
/** `rendered` resolves after pending framework updates reach the DOM. `unmount` runs
 * after the document has saved and closed: it stops transient work such as timers, and
 * writes made then are refused with `closing`. */
export interface TinyView {
  rendered?(): void | Promise<void>;
  unmount?(): void | Promise<void>;
}

export interface TinyContext {
  /** Optional on older hosts: the SDK falls back to unknown identity and live layout. */
  readonly environment?: Environment;
  /** Optional on older hosts. Feedback is best effort and never blocks an edit. */
  readonly haptics?: Haptics;
  readonly document: TinyDocument<ObjectNode>;
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
     * `tiny.speech-error`) when speech is unavailable, cancelled or failed. */
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

export interface TinyDocument<N extends ObjectNode> extends LiveDocument<N> {
  /** Observes accepted snapshot updates; returns an idempotent disposer. */
  subscribe(listener: () => void): () => void;
  /** Called whenever a scalar handle's `value` is read, so a framework adapter can
   * record the dependency (Svelte reads its own signal here). Replaces the active
   * observer; its idempotent disposer only removes that same observer. */
  observe(read: () => void): () => void;
}
export type { Definition };
