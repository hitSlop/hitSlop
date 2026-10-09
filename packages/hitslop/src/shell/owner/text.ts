import { DocumentError, isRejected } from "../../sdk/errors";
// Stateless text binding. The DOM keeps the user's text; the owner merges each change
// from the text the binding last confirmed. No draft identity survives a request.
import type { Batch, OwnerPath } from "../../schema/core";
import type { Segment } from "../../sdk/schema";

type ApplyReply = import("../../wire/page").PageResult<"apply">;
interface TextHost {
  /** The store's text at `path` (not a string when the field is gone) and its version. */
  read(path: readonly Segment[]): { text: unknown; version: string; sequence: number };
  /** Applies a batch; one whose set carries `selection` is answered with `authored` and
   * the merged selection. */
  send(batch: Batch): Promise<ApplyReply>;
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
  let expiredBase: DocumentError | undefined;
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
    if (expiredBase || inflight || composing || element.value !== confirmed.text) return;
    confirmed = { text: current.text, version: current.version };
    write(current.text);
  };
  const value = () => detached?.text ?? element.value;
  /** Sends the DOM value as one change from `confirmed`; one request at a time. */
  const send = () => {
    if (expiredBase || outcomeFailure || inflight || composing || removed || host.readOnly() || value() === confirmed.text) return;
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
        intents: [{
          type: "set",
          path: target as OwnerPath,
          value: sent,
          from: from.text,
          selection: { start: selectionStart, end: selectionEnd },
        }],
      });
      dispatched = true;
      const { sequence, authored, selectionStart: mergedStart, selectionEnd: mergedEnd } = await request;
      if (authored === undefined || mergedStart === undefined || mergedEnd === undefined)
        throw new Error("The owner did not answer the text edit");
      await host.reached(sequence);
      if (detached) {
        confirmed = { text: sent, version: authored };
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
        const exact = current.sequence === sequence;
        if (element.value !== current.text) {
          element.value = current.text;
          if (untouched && exact) element.setSelectionRange(mergedStart, mergedEnd);
          else {
            const start = mapCaret(sent, current.text, selectionStart);
            const end = mapCaret(sent, current.text, selectionEnd);
            element.setSelectionRange(start, end);
          }
          element.dispatchEvent(new Event("input", { bubbles: true }));
        } else if (untouched && exact) {
          element.setSelectionRange(mergedStart, mergedEnd);
        }
      } else {
        // The user kept typing: the next change starts from what this one authored.
        confirmed = { text: sent, version: authored };
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
        if (error.reason === "stale_base") {
          // Retention can expire a long-lived draft's merge base. Keep it in the DOM;
          // neither overwriting it nor rebasing/replaying it is an accepted recovery.
          expiredBase = new DocumentError("rejected",
            "This edit is too old to merge. Your draft is still in this field. Copy it before leaving, or press Escape to discard it.",
            "stale_base");
          if (!quiet) host.report(expiredBase);
          return;
        }
        // Other definite refusals show the owner's text again.
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
  const onKeyDown = (event: Event) => {
    const key = event as KeyboardEvent;
    if (!expiredBase || key.key !== "Escape" || composing || key.isComposing || key.keyCode === 229) return;
    const current = host.read(path);
    if (typeof current.text !== "string") return;
    expiredBase = undefined;
    confirmed = { text: current.text, version: current.version };
    write(current.text);
    event.preventDefault();
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
  // Run before an author's Escape handler ends/unmounts the editor.
  element.addEventListener("keydown", onKeyDown, true);
  return {
    get detached() { return detached !== undefined; },
    refresh: adopt,
    /** Sends unsent text now, ending a composition; the barrier awaits the result. */
    async commit() {
      composing = false;
      send();
      while (inflight) await inflight;
      if (expiredBase) throw expiredBase;
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
        if (expiredBase) throw expiredBase;
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
      element.removeEventListener("keydown", onKeyDown, true);
      drain = this.commit();
      host.track(drain);
      return drain;
    },
  };
}
