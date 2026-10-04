import { isRejected } from "@hitslop/document";
// Stateless text binding. The DOM keeps the user's text; the owner merges each change
// from the text the binding last confirmed. No draft identity survives a request.
import type { EditText } from "@hitslop/schema/core";
import type { Segment } from "@hitslop/document";

type TextReply = import("@hitslop/schema/page").PageResult<"text">;
interface TextHost {
  /** The store's text at `path` (not a string when the field is gone) and its version. */
  read(path: readonly Segment[]): { text: unknown; version: string; sequence: number };
  send(request: EditText): Promise<TextReply>;
  reached(sequence: number): Promise<void>;
  recover(): Promise<void>;
  readOnly(): boolean;
  /** Registers in-flight work so close and capture barriers wait for it. */
  track(work: Promise<unknown>): void;
  report(error: unknown): void;
  /** The document's undo, which replaces the field's own. */
  undo(redo: boolean): void;
}

/** One prefix/suffix replacement from `before` to `after`, on UTF-16 boundaries. */
function splice(before: string, after: string) {
  let index = 0;
  while (index < before.length && index < after.length && before[index] === after[index]) index++;
  if (index && /[\uD800-\uDBFF]/.test(before[index - 1]!)) index--;
  let a = before.length,
    b = after.length;
  while (a > index && b > index && before[a - 1] === after[b - 1]) {
    a--;
    b--;
  }
  if (a < before.length && a > index && /[\uDC00-\uDFFF]/.test(before[a]!)) {
    a++;
    b++;
  }
  return { index, delete: a - index, insert: after.slice(index, b) };
}
/** Maps a caret across a remote replacement of `before` with `after`. */
function mapCaret(before: string, after: string, position: number) {
  const delta = splice(before, after);
  if (position <= delta.index) return position;
  if (position >= delta.index + delta.delete) return position + delta.insert.length - delta.delete;
  return delta.index + delta.insert.length;
}

export function bindText(
  element: HTMLInputElement | HTMLTextAreaElement,
  initialPath: readonly Segment[],
  host: TextHost,
) {
  const path = initialPath;
  let composing = false;
  let detached: { text: string; selectionStart: number; selectionEnd: number } | undefined;
  let drain: Promise<void> | undefined;
  let removed = false;
  let outcomeFailure: unknown;
  /** A dispatched request whose outcome is unknown: the text before it and the text sent. */
  let uncertainText: { from: string; sent: string } | undefined;
  let inflight: Promise<void> | undefined;
  let barrierResend = false;
  // The text this binding last knew to be the field's value at `version`.
  let confirmed = { text: "", version: "" };

  const write = (next: string) => {
    if (detached || element.value === next) return;
    const old = element.value;
    const start = element.selectionStart ?? old.length;
    const end = element.selectionEnd ?? start;
    element.value = next;
    element.setSelectionRange(mapCaret(old, next, start), mapCaret(old, next, end));
    // Programmatic writes keep auto-grow mirrors and framework listeners in step.
    element.dispatchEvent(new Event("input", { bubbles: true }));
  };
  const setDisabled = () => {
    const disabled = removed || host.readOnly();
    if ((element as any).disabled !== disabled) (element as any).disabled = disabled;
  };
  /** Adopts the store's text when the DOM has nothing unsent for this field. */
  const adopt = () => {
    if (detached) return;
    const current = host.read(path);
    if (typeof current.text !== "string") {
      if (!removed) {
        removed = true;
        composing = false;
        element.blur();
      }
      setDisabled();
      return;
    }
    removed = false;
    setDisabled();
    if (inflight || composing || element.value !== confirmed.text) return;
    confirmed = { text: current.text, version: current.version };
    write(current.text);
  };
  const value = () => detached?.text ?? element.value;
  /** Sends the DOM value as one change from `confirmed`; one request at a time. */
  const send = () => {
    if (outcomeFailure || inflight || composing || removed || host.readOnly() || value() === confirmed.text) return;
    const target = path;
    const from = confirmed;
    const sent = value();
    const selectionStart = detached?.selectionStart ?? element.selectionStart ?? sent.length;
    const selectionEnd = detached?.selectionEnd ?? element.selectionEnd ?? selectionStart;
    // A refusal thrown before the request leaves the page (for example while recovery
    // has failed) is definitely not applied: keep the draft and resend after recovery.
    let dispatched = false;
    // A barrier's own resend fails the barrier; reporting it too would duplicate it.
    const quiet = barrierResend;
    const work = (async () => {
      const request = host.send({
        base: from.version,
        path: target as EditText["path"],
        from: from.text,
        to: sent,
        selectionStart,
        selectionEnd,
      });
      dispatched = true;
      const reply = await request;
      await host.reached(reply.sequence);
      if (detached) {
        confirmed = { text: sent, version: reply.authored };
        return;
      }
      const current = host.read(path);
      if (typeof current.text !== "string") return adopt();
      if (!composing && element.value === sent) {
        // Nothing typed meanwhile: show the merged text.
        const untouched =
          element.selectionStart === selectionStart && element.selectionEnd === selectionEnd;
        confirmed = { text: current.text, version: current.version };
        // The reply's caret is exact only if nothing else was published since.
        const exact = current.sequence === reply.sequence;
        if (element.value !== current.text) {
          element.value = current.text;
          if (untouched && exact) element.setSelectionRange(reply.selectionStart, reply.selectionEnd);
          else {
            const start = mapCaret(sent, current.text, selectionStart);
            const end = mapCaret(sent, current.text, selectionEnd);
            element.setSelectionRange(start, end);
          }
          element.dispatchEvent(new Event("input", { bubbles: true }));
        } else if (untouched && exact) {
          element.setSelectionRange(reply.selectionStart, reply.selectionEnd);
        }
      } else {
        // The user kept typing: the next change starts from what this one authored.
        confirmed = { text: sent, version: reply.authored };
      }
    })();
    inflight = work
      .catch((error) => {
        // An uncertain outcome must not erase or automatically replay the draft.
        if (!isRejected(error)) {
          outcomeFailure = error;
          uncertainText = dispatched ? { from: from.text, sent } : undefined;
          if (!quiet) host.report(error);
          return;
        }
        // Definite refusals show the owner's text again.
        confirmed = { text: "", version: "" };
        const current = host.read(path);
        if (typeof current.text === "string") {
          confirmed = { text: current.text, version: current.version };
          if (!composing) write(current.text);
        }
        if (detached) detached.text = confirmed.text;
        if (!quiet) host.report(error);
      })
      .finally(() => {
        inflight = undefined;
        send();
      });
    host.track(inflight);
  };
  const onInput = () => {
    if (!composing) send();
  };
  const onStart = () => {
    composing = true;
  };
  const onEnd = () => {
    composing = false;
    send();
  };
  // Undo belongs to the document: the field's own history knows nothing of edits made
  // elsewhere, and replaying it would author them again as new typing.
  const onBeforeInput = (event: Event) => {
    const { inputType } = event as InputEvent;
    if (inputType !== "historyUndo" && inputType !== "historyRedo") return;
    event.preventDefault();
    host.undo(inputType === "historyRedo");
  };
  const start = () => {
    removed = false;
    const current = host.read(path);
    confirmed =
      typeof current.text === "string"
        ? { text: current.text, version: current.version }
        : { text: "", version: "" };
    if (typeof current.text === "string") write(current.text);
    adopt();
  };
  start();
  element.addEventListener("input", onInput);
  element.addEventListener("compositionstart", onStart);
  element.addEventListener("compositionend", onEnd);
  element.addEventListener("beforeinput", onBeforeInput);
  return {
    get detached() { return detached !== undefined; },
    refresh: adopt,
    /** Sends unsent text now, ending a composition; the barrier awaits the result. */
    async commit() {
      composing = false;
      send();
      while (inflight) await inflight;
      if (outcomeFailure) {
        await host.recover();
        if (uncertainText) {
          const current = host.read(path);
          // Recovery reads a snapshot admitted after the request, so the field shows the
          // sent text if it applied, or its earlier text if it did not. Anything else is
          // ambiguous: retain the draft, because replaying could duplicate the edit.
          if (current.text === uncertainText.sent) confirmed = { text: current.text, version: current.version };
          else if (current.text !== uncertainText.from) throw outcomeFailure;
        }
        outcomeFailure = undefined;
        uncertainText = undefined;
        barrierResend = true;
        try {
          send();
          while (inflight) await inflight;
        } finally { barrierResend = false; }
        if (outcomeFailure) throw outcomeFailure;
      }
    },
    /** Detach immediately, but retain the final draft and target until it drains. */
    destroy() {
      if (drain) return drain;
      detached = {
        text: element.value,
        selectionStart: element.selectionStart ?? element.value.length,
        selectionEnd: element.selectionEnd ?? element.value.length,
      };
      element.removeEventListener("input", onInput);
      element.removeEventListener("compositionstart", onStart);
      element.removeEventListener("compositionend", onEnd);
      element.removeEventListener("beforeinput", onBeforeInput);
      drain = this.commit();
      host.track(drain);
      return drain;
    },
  };
}
