import type { BrowserEvent, BrowserRequest } from "../wire/browser.generated";
import type { HostRequest } from "../wire/page";

/**
 * How long the worker has to answer before it is judged unresponsive. The host's operations
 * (flush, export, resource reads) and the page's requests all wait on the same serial worker,
 * so they share one bound: a stuck worker fails every one of them within it. Legitimate work
 * is far shorter (Rust applies a write synchronously). The slowest is an export, which the
 * host also gives up on at this deadline, and a request queued behind it is judged by the
 * same clock.
 */
export const copyDeadline = 60_000;

/** The time one request has to be answered. */
export type Deadline = {
  after: number;
  /** What the request fails with when time runs out. */
  message: string;
  /**
   * The request may already have changed the document, so its outcome is unknown and
   * retrying could apply it twice. Running out of time then fails the whole table.
   */
  terminal?: boolean;
};

type Waiting<Reply> = {
  resolve(reply: Reply): void;
  reject(error: Error): void;
  timer: ReturnType<typeof setTimeout> | undefined;
};

/**
 * Requests waiting for a reply from another context. Each request settles exactly once, by
 * its reply, a refusal, its deadline or a terminal failure, and clears its timer on every one
 * of those paths. Whatever arrives afterwards is rejected: `settle` returns false and nothing
 * resolves.
 *
 * `fail` is for a peer that cannot answer again. It rejects every waiting request and every
 * later one, and sends nothing: a request whose outcome is unknown is never replayed.
 */
export class PendingRequests<Reply> {
  private readonly waiting = new Map<string, Waiting<Reply>>();
  private failed: Error | undefined;

  /** The terminal failure, once there is one. */
  get failure(): Error | undefined {
    return this.failed;
  }

  /** How many requests are waiting. */
  get size(): number {
    return this.waiting.size;
  }

  /** Sends one request, under the next ID, and waits for `settle` to answer it. */
  start(send: (id: string) => void, { timeout }: { timeout?: Deadline } = {}): Promise<Reply> {
    if (this.failed) return Promise.reject(this.failed);
    const id = crypto.randomUUID();
    return new Promise<Reply>((resolve, reject) => {
      const timer =
        timeout &&
        setTimeout(() => {
          const error = new Error(timeout.message);
          if (timeout.terminal) this.fail(error);
          else this.settle(id, undefined, error);
        }, timeout.after);
      this.waiting.set(id, { resolve, reject, timer });
      try {
        send(id);
      } catch (error) {
        // Never sent, so the outcome is known and the table stays usable.
        this.settle(id, undefined, error instanceof Error ? error : new Error(String(error)));
      }
    });
  }

  /** Settles the request `id` with its reply, or with `error`. False when none is waiting. */
  settle(id: string, reply: Reply): boolean;
  settle(id: string, reply: undefined, error: Error): boolean;
  settle(id: string, reply: Reply | undefined, error?: Error): boolean {
    const waiting = this.waiting.get(id);
    if (!waiting) return false;
    this.waiting.delete(id);
    clearTimeout(waiting.timer);
    if (error) waiting.reject(error);
    else waiting.resolve(reply as Reply);
    return true;
  }

  /** The peer is gone. The first failure is the one every request, now or later, gets. */
  fail(error: Error): void {
    this.failed ??= error;
    const waiting = [...this.waiting.values()];
    this.waiting.clear();
    for (const request of waiting) {
      clearTimeout(request.timer);
      request.reject(this.failed);
    }
  }
}

/** What the worker posts back: a wire event, and the bytes a `resource` read transfers. */
export type WorkerEvent = BrowserEvent & { bytes?: Uint8Array<ArrayBuffer> };

/** The requests the worker answers by ID. `open` is answered by `ready`, or by an `error`
 * without an ID. */
type Call = Extract<BrowserRequest, { id: string }>;
export type CallType = Call["type"];
/** The event that answers each request: the wire pairs them by ID, not by name. */
const answeredBy = {
  page: "reply",
  flush: "reply",
  export: "exported",
  resourcePlan: "resourcePlan",
  resource: "resource",
} as const satisfies Record<CallType, BrowserEvent["type"]>;
/** A request as the caller writes it: the ID is the table's. */
export type CallBody<T extends CallType> = Omit<Extract<Call, { type: T }>, "id">;
/** What resolves a `T` request. A failed one rejects instead, with an `error` event's message. */
export type CallReply<T extends CallType> = Extract<WorkerEvent, { type: (typeof answeredBy)[T] }>;

/** The page frame's answer to a `host` message. */
export type HostReply = { type: "hostReply"; id: string; error?: string };

/** What the host posts to the page frame, over the page port. A `fatal` message is the
 * copy's last: the worker has failed, so the frame fails whatever waits on it. */
export type FrameMessage =
  | Extract<BrowserEvent, { type: "reply" | "publication" }>
  | { type: "error"; id: string; error: string }
  | { type: "host"; id: string; request: HostRequest }
  | { type: "fatal"; error: string };
