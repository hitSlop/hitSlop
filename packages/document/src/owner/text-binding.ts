// Local DOM draft only; all CRDT ancestry and merge decisions live in Rust.
export function splice(before: string, after: string) {
  let index = 0;
  while (index < before.length && index < after.length && before[index] === after[index]) index++;
  // Back up if the common prefix ends between a surrogate pair.
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
// Snapshot arrays are immutable. Share one index across all field bindings so a
// publication does not scan an N-row list once for each of its N controls.
// Weak keys let replaced snapshots (and their indexes) be collected together.
const rowIndexes = new WeakMap<readonly any[], Map<string, any>>();
function readRow(rows: any, id: string) {
  if (!Array.isArray(rows)) return undefined;
  if (!Object.isFrozen(rows)) return rows.find((row) => row?.$id === id);
  let index = rowIndexes.get(rows);
  if (!index) {
    index = new Map();
    for (const row of rows)
      if (typeof row?.$id === "string" && !index.has(row.$id)) index.set(row.$id, row);
    rowIndexes.set(rows, index);
  }
  return index.get(id);
}
export function readPath(value: any, path: any[]) {
  for (const part of path)
    value = typeof part === "string" ? value?.[part] : readRow(value, part.id);
  return value;
}
export function bindText(
  element: HTMLTextAreaElement | HTMLInputElement,
  path: any[],
  host: {
    frame(): any;
    editable?(): boolean;
    text(request: any): Promise<any>;
    release(id: string): Promise<void>;
  },
  report: (error: unknown) => void = console.error,
) {
  let composing = false,
    running: Promise<void> | undefined,
    failure: unknown;
  let draft:
    { id: string; base: string; session: string; sequence: number; authored: string } | undefined;
  let disposed = false;
  element.value = readPath(host.frame().value, path);
  function refresh() {
    const disabled = disposed || host.editable?.() === false;
    if (element.disabled !== disabled) element.disabled = disabled;
    const next = readPath(host.frame().value, path);
    if (typeof next !== "string") {
      disposed = true;
      composing = false;
      failure = undefined;
      element.disabled = true;
      element.blur();
      const id = draft?.id;
      draft = undefined;
      if (id) void host.release(id).catch(report);
      return;
    }
    if (!draft && !composing && !failure) {
      if (element.value !== next) {
        const old = element.value,
          start = element.selectionStart ?? 0,
          end = element.selectionEnd ?? start;
        const delta = splice(old, next);
        const map = (p: number) =>
          p <= delta.index
            ? p
            : p >= delta.index + delta.delete
              ? p + delta.insert.length - delta.delete
              : delta.index + delta.insert.length;
        element.value = next;
        element.setSelectionRange(map(start), map(end));
      }
    }
  }
  function ensureDraft() {
    if (!draft) {
      const f = host.frame();
      draft = {
        id: crypto.randomUUID(),
        base: f.version,
        session: f.session,
        sequence: 0,
        authored: readPath(f.value, path),
      };
    }
  }
  function pump() {
    if (composing || running || failure || disposed) return;
    ensureDraft();
    running = (async () => {
      while (!composing && !disposed && draft && element.value !== draft.authored) {
        const active = draft;
        const sent = element.value,
          selectionStart = element.selectionStart ?? sent.length,
          selectionEnd = element.selectionEnd ?? selectionStart;
        const sequence = draft.sequence + 1;
        const response = await host.text({
          session: draft.session,
          draft: draft.id,
          sequence,
          base: draft.base,
          path,
          ...(draft.sequence ? { parent: draft.sequence } : {}),
          ...splice(draft.authored, sent),
          selectionStart,
          selectionEnd,
        });
        if (disposed || draft !== active) return;
        if (host.frame().session !== active.session)
          throw new Error("owner_changed: reconcile before editing");
        draft.authored = sent;
        draft.sequence = sequence;
        if (typeof readPath(host.frame().value, path) !== "string") {
          disposed = true;
          element.blur();
          break;
        }
        if (!composing && element.value === sent) {
          // Only apply returned caret anchors if the user hasn't moved selection meanwhile.
          const untouched =
            element.selectionStart === selectionStart && element.selectionEnd === selectionEnd;
          element.value = readPath(host.frame().value, path);
          if (untouched)
            element.setSelectionRange(response.text.selectionStart, response.text.selectionEnd);
          const id = draft.id;
          draft = undefined;
          await host.release(id);
          break;
        }
      }
      if (!composing && draft && element.value === draft.authored) {
        const id = draft.id;
        draft = undefined;
        await host.release(id);
      }
    })()
      .catch((e) => {
        failure = e;
        report(e);
      })
      .finally(() => {
        running = undefined;
        // Input/composition may arrive while an acknowledged draft is releasing.
        if (draft && !composing && !failure && !disposed) pump();
      });
  }
  function input() {
    if (disposed || host.editable?.() === false) {
      refresh();
      return;
    }
    ensureDraft();
    pump();
  }
  function start() {
    if (disposed || host.editable?.() === false) return;
    ensureDraft();
    composing = true;
  }
  function end() {
    composing = false;
    pump();
  }
  refresh();
  element.addEventListener("input", input);
  element.addEventListener("compositionstart", start);
  element.addEventListener("compositionend", end);
  return {
    refresh,
    pending: () => !!running || !!draft || composing,
    async drain() {
      if (composing) throw new Error("composition_pending");
      while (draft || running) {
        if (failure) throw failure;
        if (composing) throw new Error("composition_pending");
        pump();
        await running;
      }
      if (failure) throw failure;
    },
    destroy() {
      disposed = true;
      element.removeEventListener("input", input);
      element.removeEventListener("compositionstart", start);
      element.removeEventListener("compositionend", end);
      if (draft) {
        const id = draft.id;
        draft = undefined;
        void host.release(id).catch(report);
      }
    },
  };
}
