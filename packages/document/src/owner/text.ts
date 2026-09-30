// Stateless text binding. The DOM keeps the user's text; the owner merges each change
// from the text the binding last confirmed. No draft identity survives a request.
import type { EditText } from "@hitslop/schema/owner";
import type { Segment } from "./store";

export type TextReply = {
  sequence: number;
  authored: string;
  selectionStart: number;
  selectionEnd: number;
};
export interface TextHost {
  /** The store's text at `path` (not a string when the field is gone) and its version. */
  read(path: readonly Segment[]): { text: unknown; version: string; sequence: number };
  send(request: EditText): Promise<TextReply>;
  reached(sequence: number): Promise<void>;
  readOnly(): boolean;
  /** Registers in-flight work so close and capture barriers wait for it. */
  track(work: Promise<unknown>): void;
  report(error: unknown): void;
}

/** One prefix/suffix replacement from `before` to `after`, on UTF-16 boundaries. */
export function splice(before: string, after: string) {
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
  let path = initialPath;
  let composing = false;
  let disposed = false;
  let removed = false;
  let inflight: Promise<void> | undefined;
  // The text this binding last knew to be the field's value at `version`.
  let confirmed = { text: "", version: "" };

  const write = (next: string) => {
    if (element.value === next) return;
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
  const dirty = () => !removed && !disposed && element.value !== confirmed.text;
  /** Sends the DOM value as one change from `confirmed`; one request at a time. */
  const send = () => {
    if (inflight || composing || removed || host.readOnly() || element.value === confirmed.text) return;
    const target = path;
    const from = confirmed;
    const sent = element.value;
    const selectionStart = element.selectionStart ?? sent.length;
    const selectionEnd = element.selectionEnd ?? selectionStart;
    const work = (async () => {
      const reply = await host.send({
        base: from.version,
        path: target as EditText["path"],
        from: from.text,
        to: sent,
        selectionStart,
        selectionEnd,
      });
      await host.reached(reply.sequence);
      if (target !== path) return;
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
        // Refused edits are never replayed; show the owner's text for the field again.
        confirmed = { text: "", version: "" };
        const current = host.read(path);
        if (typeof current.text === "string") {
          confirmed = { text: current.text, version: current.version };
          if (!composing) write(current.text);
        }
        host.report(error);
      })
      .finally(() => {
        inflight = undefined;
        if (!disposed) send();
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
  const start = (next: readonly Segment[]) => {
    path = next;
    removed = false;
    const current = host.read(path);
    confirmed =
      typeof current.text === "string"
        ? { text: current.text, version: current.version }
        : { text: "", version: "" };
    if (typeof current.text === "string") write(current.text);
    adopt();
  };
  start(initialPath);
  element.addEventListener("input", onInput);
  element.addEventListener("compositionstart", onStart);
  element.addEventListener("compositionend", onEnd);
  return {
    get path() {
      return path;
    },
    refresh: adopt,
    /** Whether the DOM holds text the owner has not accepted yet. */
    pending: () => !!inflight || composing || dirty(),
    /** Sends unsent text now, ending a composition; the barrier awaits the result. */
    commit() {
      composing = false;
      send();
      return inflight;
    },
    /** Moves to another field. Unsent text for the old one is sent first. */
    retarget(next: readonly Segment[]) {
      if (JSON.stringify(next) === JSON.stringify(path)) return;
      this.commit();
      const previous = inflight;
      const move = () => start(next);
      if (previous) void previous.then(move);
      else move();
    },
    destroy() {
      this.commit();
      disposed = true;
      element.removeEventListener("input", onInput);
      element.removeEventListener("compositionstart", onStart);
      element.removeEventListener("compositionend", onEnd);
    },
  };
}
