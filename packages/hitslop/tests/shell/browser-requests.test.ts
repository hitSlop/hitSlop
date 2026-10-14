// The settlement table behind the browser host's and page frame's requests: a reply, a
// refusal, a deadline or one terminal failure settles each request exactly once, clears
// its timer, and nothing that arrives afterwards resolves anything.
import { afterEach, beforeEach, expect, jest, test } from "bun:test";
import { PendingRequests } from "../../src/browser/requests";

beforeEach(() => jest.useFakeTimers());
afterEach(() => jest.useRealTimers());

/** Follows a promise without awaiting it, so a request that never settles fails an assertion
 * instead of hanging the test. */
function track<T>(promise: Promise<T>) {
  const box = {
    state: "pending" as "pending" | "resolved" | "rejected",
    value: undefined as T | undefined,
    error: undefined as unknown,
  };
  promise.then(
    (value) => Object.assign(box, { state: "resolved", value }),
    (error) => Object.assign(box, { state: "rejected", error }),
  );
  return box;
}
/** Lets every promise reaction queued so far run. */
const flush = async () => {
  for (let turn = 0; turn < 8; turn++) await null;
};
/** A table plus the IDs it handed to `send`, in order. */
function table<Reply = string>() {
  const requests = new PendingRequests<Reply>();
  const sent: string[] = [];
  const start = (timeout?: { after: number; message: string; terminal?: boolean }) =>
    track(requests.start((id) => sent.push(id), { timeout }));
  return { requests, sent, start };
}

test("a reply settles its request once; later replies for it and unknown ones are rejected", async () => {
  const { requests, sent, start } = table();
  const request = start({ after: 1000, message: "late" });
  expect(sent).toHaveLength(1);
  expect(requests.size).toBe(1);
  expect(jest.getTimerCount()).toBe(1);
  expect(requests.settle(sent[0]!, "first")).toBe(true);
  await flush();
  expect(request).toMatchObject({ state: "resolved", value: "first" });
  expect(requests.size).toBe(0);
  expect(jest.getTimerCount()).toBe(0);
  expect(requests.settle(sent[0]!, "second")).toBe(false);
  expect(requests.settle("never-issued", "reply")).toBe(false);
  expect(request.value).toBe("first");
});

test("a refusal rejects only its own request and clears only its own timer", async () => {
  const { requests, sent, start } = table();
  const refused = start({ after: 1000, message: "late" });
  const answered = start({ after: 1000, message: "late" });
  expect(jest.getTimerCount()).toBe(2);
  const reason = new Error("refused");
  expect(requests.settle(sent[0]!, undefined, reason)).toBe(true);
  await flush();
  expect(refused).toMatchObject({ state: "rejected", error: reason });
  expect(answered.state).toBe("pending");
  expect(requests.size).toBe(1);
  expect(jest.getTimerCount()).toBe(1);
  requests.settle(sent[1]!, "ok");
  await flush();
  expect(answered.state).toBe("resolved");
  expect(jest.getTimerCount()).toBe(0);
  // A refusal is not a failure: the table keeps accepting requests.
  expect(requests.failure).toBeUndefined();
});

test("a request without a deadline holds no timer", async () => {
  const { requests, sent, start } = table();
  const request = start();
  expect(jest.getTimerCount()).toBe(0);
  jest.advanceTimersByTime(24 * 60 * 60_000);
  await flush();
  expect(request.state).toBe("pending");
  requests.settle(sent[0]!, "reply");
  await flush();
  expect(request.state).toBe("resolved");
});

test("a deadline rejects its request, ignores its late reply and leaves the table usable", async () => {
  const { requests, sent, start } = table();
  const slow = start({ after: 1000, message: "The copy did not respond" });
  const other = start({ after: 5000, message: "other" });
  jest.advanceTimersByTime(1000);
  await flush();
  expect(slow.state).toBe("rejected");
  expect((slow.error as Error).message).toBe("The copy did not respond");
  expect(other.state).toBe("pending");
  expect(requests.size).toBe(1);
  expect(jest.getTimerCount()).toBe(1);
  // The reply that comes after the deadline resolves nothing, and says so.
  expect(requests.settle(sent[0]!, "too late")).toBe(false);
  await flush();
  expect(slow.state).toBe("rejected");
  expect(requests.failure).toBeUndefined();
  // Not terminal: the other request still settles, and new ones still start.
  requests.settle(sent[1]!, "ok");
  const next = start({ after: 1000, message: "late" });
  expect(sent).toHaveLength(3);
  requests.settle(sent[2]!, "next");
  await flush();
  expect(other.state).toBe("resolved");
  expect(next.value).toBe("next");
  expect(requests.size).toBe(0);
  expect(jest.getTimerCount()).toBe(0);
});

test("a terminal deadline fails every pending request and every later one", async () => {
  const { requests, sent, start } = table();
  const write = start({ after: 1000, message: "The document did not respond", terminal: true });
  const sibling = start({ after: 60_000, message: "sibling" });
  const unbounded = start();
  jest.advanceTimersByTime(1000);
  await flush();
  for (const request of [write, sibling, unbounded]) expect(request.state).toBe("rejected");
  // One error describes the failure, whichever request it reaches.
  expect((write.error as Error).message).toBe("The document did not respond");
  expect(sibling.error).toBe(write.error);
  expect(unbounded.error).toBe(write.error);
  expect(requests.failure).toBe(write.error as Error);
  expect(requests.size).toBe(0);
  expect(jest.getTimerCount()).toBe(0);
  // Nothing is replayed: a later request is refused without being sent.
  const later = start({ after: 1000, message: "later" });
  await flush();
  expect(later).toMatchObject({ state: "rejected", error: write.error });
  expect(sent).toHaveLength(3);
  expect(requests.size).toBe(0);
  expect(jest.getTimerCount()).toBe(0);
  for (const id of sent) expect(requests.settle(id, "late")).toBe(false);
});

test("fail rejects everything pending with its error and refuses later requests immediately", async () => {
  const { requests, sent, start } = table();
  const first = start({ after: 60_000, message: "first" });
  const second = start({ after: 30_000, message: "second" });
  const third = start();
  expect(jest.getTimerCount()).toBe(2);
  const stopped = new Error("The document worker stopped");
  requests.fail(stopped);
  expect(requests.size).toBe(0);
  expect(jest.getTimerCount()).toBe(0);
  await flush();
  for (const request of [first, second, third])
    expect(request).toMatchObject({ state: "rejected", error: stopped });
  expect(requests.failure).toBe(stopped);
  // Later requests fail at once, with the same error, and never reach `send`.
  const sentBefore = sent.length;
  const later = start({ after: 60_000, message: "later" });
  await flush();
  expect(later).toMatchObject({ state: "rejected", error: stopped });
  expect(sent).toHaveLength(sentBefore);
  expect(requests.size).toBe(0);
  expect(jest.getTimerCount()).toBe(0);
});

test("replies that arrive after a terminal failure resolve nothing", async () => {
  const { requests, sent, start } = table();
  const request = start({ after: 60_000, message: "late" });
  const stopped = new Error("stopped");
  requests.fail(stopped);
  expect(requests.settle(sent[0]!, "late reply")).toBe(false);
  expect(requests.settle(sent[0]!, undefined, new Error("late refusal"))).toBe(false);
  await flush();
  expect(request).toMatchObject({ state: "rejected", error: stopped });
  jest.advanceTimersByTime(120_000);
  await flush();
  expect(request.error).toBe(stopped);
});

test("the first failure wins and a repeated one changes nothing", async () => {
  const { requests, start } = table();
  const first = new Error("first");
  const second = new Error("second");
  requests.fail(first);
  requests.fail(second);
  const later = start();
  await flush();
  expect(requests.failure).toBe(first);
  expect(later.error).toBe(first);
});

test("a send that throws rejects its own request and leaves no timer behind", async () => {
  const requests = new PendingRequests<string>();
  const thrown = new Error("DataCloneError");
  const refused = track(
    requests.start(
      () => {
        throw thrown;
      },
      { timeout: { after: 1000, message: "late" } },
    ),
  );
  await flush();
  expect(refused).toMatchObject({ state: "rejected", error: thrown });
  expect(requests.size).toBe(0);
  expect(jest.getTimerCount()).toBe(0);
  const text = track(
    requests.start(() => {
      throw "plain text";
    }),
  );
  await flush();
  expect(text.state).toBe("rejected");
  expect(text.error).toBeInstanceOf(Error);
  expect((text.error as Error).message).toBe("plain text");
  // The request was never sent, so its outcome is known: the table stays usable.
  expect(requests.failure).toBeUndefined();
});

test("a reply that arrives while the request is still being sent settles it", async () => {
  const requests = new PendingRequests<string>();
  const request = track(
    requests.start((id) => expect(requests.settle(id, "synchronous")).toBe(true), {
      timeout: { after: 1000, message: "late" },
    }),
  );
  await flush();
  expect(request).toMatchObject({ state: "resolved", value: "synchronous" });
  expect(jest.getTimerCount()).toBe(0);
});

test("each request gets its own ID, and settling one leaves the others pending", async () => {
  const { requests, sent, start } = table<number>();
  const requestsInFlight = Array.from({ length: 50 }, () =>
    start({ after: 1000, message: "late" }),
  );
  expect(new Set(sent).size).toBe(50);
  expect(jest.getTimerCount()).toBe(50);
  requests.settle(sent[10]!, 10);
  await flush();
  expect(requestsInFlight.filter(({ state }) => state === "pending")).toHaveLength(49);
  requests.fail(new Error("stopped"));
  await flush();
  expect(requestsInFlight.filter(({ state }) => state === "rejected")).toHaveLength(49);
  expect(requestsInFlight[10]).toMatchObject({ state: "resolved", value: 10 });
  expect(jest.getTimerCount()).toBe(0);
});

test("replies keep the table's type", () => {
  const requests = new PendingRequests<{ json: string }>();
  // @ts-expect-error a reply must have the table's type
  requests.settle("id", "not a reply");
  // @ts-expect-error a success carries a reply; only a refusal omits it
  requests.settle("id", undefined);
  expect(requests.settle("id", { json: "{}" })).toBe(false);
});
