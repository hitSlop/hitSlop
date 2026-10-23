// What lets a hosted tab mount: a join settles on the worker's phases, so a pause the person
// must act on can never leave the page unmounted with nothing said.
import { expect, test } from "bun:test";
import { JoinGate } from "../../src/browser/join";
import type { SyncPhase } from "../../src/wire/sharing.generated";

const sync = (phase: SyncPhase, status = "words shown") =>
  ({ type: "sync", status, phase }) as const;
/** Follows a promise without awaiting it, so one that never settles fails an assertion. */
function settled(promise: Promise<void>) {
  const box = { done: false };
  void promise.then(() => (box.done = true));
  return async () => {
    for (let turn = 0; turn < 4; turn++) await null;
    return box.done;
  };
}
function watch(gate: JoinGate) {
  return { resources: settled(gate.resources), catchup: settled(gate.catchup) };
}

test("a pause settles both waits and says why the join stopped", async () => {
  const gate = new JoinGate();
  const waiting = watch(gate);
  gate.handle(sync("offline", "Offline; reconnecting"));
  gate.handle(sync("syncing", "Receiving files"));
  expect(await waiting.resources()).toBe(false);
  expect(await waiting.catchup()).toBe(false);
  gate.handle(sync("paused", "Update hitSlop to keep sharing"));
  expect(await waiting.resources()).toBe(true);
  expect(await waiting.catchup()).toBe(true);
  expect(gate.stopped).toBe("Update hitSlop to keep sharing");
  expect(gate.ended).toBe(false);
});

test("the first pause is the one reported", () => {
  const gate = new JoinGate();
  gate.handle(sync("paused", "Refused"));
  gate.handle(sync("paused", "Later"));
  expect(gate.stopped).toBe("Refused");
});

test("a room that ended opens the local copy rather than stopping the join", async () => {
  const gate = new JoinGate();
  const waiting = watch(gate);
  gate.handle(sync("ended", "Sharing ended"));
  expect(await waiting.resources()).toBe(true);
  expect(await waiting.catchup()).toBe(true);
  expect(gate.ended).toBe(true);
  expect(gate.stopped).toBeUndefined();
});

test("catching up settles the join without resources, and resources alone do not", async () => {
  const gate = new JoinGate();
  const waiting = watch(gate);
  gate.handle({ type: "resourcesReady" });
  expect(await waiting.resources()).toBe(true);
  expect(await waiting.catchup()).toBe(false);
  gate.handle(sync("synced", "Synced"));
  expect(await waiting.catchup()).toBe(true);
  expect(gate.stopped).toBeUndefined();
});

test("the words shown never decide: an old worker's event without a phase waits", async () => {
  const gate = new JoinGate();
  const waiting = watch(gate);
  gate.handle({ type: "sync", status: "Synced" });
  expect(await waiting.catchup()).toBe(false);
});
