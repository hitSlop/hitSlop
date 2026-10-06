import doc from "./schema";
import { createCountdown, type Session } from "./src/countdown.svelte";

// One clock for the editor and its export. A failed write is reported by the host.
function saveSession(session: Session) {
  const history = doc.current.history;
  const expired = history.slice(11).map((entry) => entry.$id);
  void doc.change((tx) => {
    tx.fields.history.insert(session, history.length ? { before: history[0].$id } : undefined);
    for (const id of expired) tx.fields.history.remove(id);
  });
}

let shared: ReturnType<typeof createCountdown> | undefined;

/** Created on first use, once the document is mounted. */
export function useClock() {
  if (!shared) {
    $effect.root(() => {
      shared = createCountdown(
        () => ({ focusMinutes: doc.current.focusMinutes, restMinutes: doc.current.restMinutes }),
        saveSession,
      );
    });
  }
  return shared!;
}
