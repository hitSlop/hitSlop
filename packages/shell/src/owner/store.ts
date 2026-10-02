import { rowIndexes, indexOf, keyIn, pathKey } from "./path";
import { PushLimits } from "@hitslop/schema/constants";
import { isRejected, isDocumentError } from "@hitslop/document";
import { DocumentError } from "@hitslop/document/internal";
// The page's immutable projection of owner state. No CRDT: it applies the owner's
// publications in sequence order and resyncs from a fresh snapshot on any gap.
import type { OwnerPatchOp, OwnerPublication, OwnerState } from "@hitslop/schema/core";
import type { PagePush } from "@hitslop/schema/page";

import type { Segment } from "@hitslop/document";

/** Applies a text change counted in Unicode code points of `text`, as the core counts. */
function applyText(text: string, delta: readonly import("@hitslop/schema/core").TextHunk[]) {
  let out = "",
    at = 0;
  const advance = (count: number) => {
    const start = at;
    for (let i = 0; i < count; i++) {
      if (at >= text.length) throw new Error("Text publication exceeds its field");
      const unit = text.charCodeAt(at);
      at += unit >= 0xd800 && unit <= 0xdbff ? 2 : 1;
    }
    return text.slice(start, at);
  };
  for (const hunk of delta) {
    if ("retain" in hunk) out += advance(hunk.retain);
    else if ("delete" in hunk) advance(hunk.delete);
    else out += hunk.insert;
  }
  return out + text.slice(at);
}
function deepFreeze(value: any) {
  if (!value || typeof value !== "object" || Object.isFrozen(value)) return;
  for (const child of Object.values(value)) deepFreeze(child);
  Object.freeze(value);
}

/**
 * Applies one publication's operations. Objects copied during the batch are mutated in
 * place for the rest of it, so a 1,000-operation batch copies each list once, and only
 * those new objects are frozen: unchanged rows keep their identity.
 */
export function applyOps(root: any, ops: readonly OwnerPatchOp[]): any {
  const fresh = new Set<object>();
  const own = (value: any) => {
    if (fresh.has(value)) return value;
    const copy = Array.isArray(value) ? value.slice() : { ...value };
    // Setting a row keeps positions, so the copy shares the index map.
    if (Array.isArray(value) && rowIndexes.has(value)) rowIndexes.set(copy, rowIndexes.get(value)!);
    fresh.add(copy);
    return copy;
  };
  // Returns the owned container at `path`, copying its ancestors on the way.
  const owned = (path: readonly Segment[]) => {
    root = own(root);
    let node = root;
    for (const part of path) {
      const key = keyIn(node, part);
      if (typeof key === "number" && key < 0) throw new Error("Missing publication row");
      node[key] = own(node[key]);
      node = node[key];
    }
    return node;
  };
  const listChanged = (list: any[]) => rowIndexes.delete(list);
  for (const op of ops) {
    const parentPath = op.path.slice(0, -1) as Segment[];
    const last = op.path.at(-1) as Segment;
    switch (op.type) {
      case "set":
      case "text": {
        const parent = owned(parentPath);
        const key = keyIn(parent, last);
        if (typeof key === "number" && (key < 0 || key >= parent.length)) throw new Error("Missing publication row");
        if (op.type === "set") parent[key] = op.value;
        else if (typeof parent[key] === "string") parent[key] = applyText(parent[key], op.delta);
        else throw new Error("Text publication for a value that is not text");
        break;
      }
      case "remove": {
        const parent = owned(parentPath);
        delete parent[last as string];
        break;
      }
      case "insertRow": {
        const list = owned(op.path as Segment[]);
        list.splice(op.index, 0, op.value);
        listChanged(list);
        break;
      }
      case "deleteRow":
      case "moveRow": {
        const list = owned(op.path as Segment[]);
        const i = indexOf(list, op.id);
        if (i < 0) throw new Error("Missing publication row");
        const [row] = list.splice(i, 1);
        if (op.type === "moveRow") list.splice(op.index, 0, row);
        listChanged(list);
        break;
      }
    }
  }
  deepFreeze(root);
  return root;
}

export type Changes = { paths: Set<string>; removed?: Segment[][] } | "all";

/** Timing and buffer bounds are injectable for deterministic boundary tests. Buffer bytes
 * are UTF-8, as the host counts them. */
export const recoveryPolicy = { stallMS: 2000, deadlineMS: 15000, retryMS: 250, maxRetryMS: 4000,
  maxItems: PushLimits.items as number, maxBytes: PushLimits.bytes as number };
const utf8 = new TextEncoder();

/** Sequence-ordered state, fed by `open` and the push stream. */
export class Store {
  state: OwnerState = { sequence: 0, version: "", value: undefined, issues: [] };
  failure: Error | undefined;
  private opened = false;
  private buffered: PagePush[] = [];
  private bufferedBytes = 0;
  private bufferGeneration = 0;
  private resyncing?: Promise<void>;
  private watchdog?: ReturnType<typeof setTimeout>;
  private waiters: { sequence: number; deadline: number; resolve(): void; reject(error: unknown): void }[] = [];

  constructor(
    private readonly reopen: () => Promise<OwnerState>,
    private readonly changed: (changes: Changes) => void,
    private readonly policy = recoveryPolicy,
  ) {}

  private install(state: OwnerState) {
    deepFreeze(state.value);
    deepFreeze(state.issues);
    this.state = state;
    this.opened = true;
  }
  /** Install the snapshot and all buffered publications before resolving writers. */
  load(state: OwnerState) {
    this.install(state);
    const buffered = this.takeBuffer();
    if (!this.consume(buffered)) { this.recover(); return; }
    this.changed("all");
    this.settle();
  }
  private buffer(pushes: readonly PagePush[]) {
    for (const push of pushes) {
      const bytes = utf8.encode(JSON.stringify(push)).length;
      if (push.type === "resync" || this.buffered.length >= this.policy.maxItems ||
          this.bufferedBytes + bytes > this.policy.maxBytes) {
        this.buffered = [];
        this.bufferedBytes = 0;
        this.bufferGeneration++;
        // A marker forces another snapshot if overflow races an in-flight open.
        this.buffered.push({ type: "resync" });
        continue;
      }
      this.buffered.push(push);
      this.bufferedBytes += bytes;
    }
  }
  private takeBuffer() {
    const result = this.buffered;
    this.buffered = [];
    this.bufferedBytes = 0;
    return result;
  }
  publish(pushes: readonly PagePush[]) {
    if (!this.opened || this.resyncing || this.failure) { this.buffer(pushes); return; }
    if (!this.consume(pushes)) { this.recover(); return; }
    this.settle();
  }
  /** Returns false when continuity requires a fresh snapshot. */
  private consume(pushes: readonly PagePush[]): boolean {
    const paths = new Set<string>();
    const removed: Segment[][] = [];
    let touched = false;
    // Every exit announces what was applied; `rest` waits for the fresh snapshot.
    const finish = (complete: boolean, rest: readonly PagePush[] = []) => {
      this.buffer(rest);
      if (touched) this.changed({ paths, removed });
      return complete;
    };
    for (const [i, push] of pushes.entries()) {
      if (push.type === "resync") return finish(false, pushes.slice(i + 1));
      const p: OwnerPublication = push.publication;
      if (p.sequence <= this.state.sequence) continue;
      if (p.previous !== this.state.sequence) return finish(false, pushes.slice(i));
      let value;
      try { value = applyOps(this.state.value, p.ops); }
      catch { return finish(false, pushes.slice(i)); }
      // Issues arrive only when they change.
      const issues = p.issues ?? this.state.issues;
      deepFreeze(issues);
      this.state = { sequence: p.sequence, version: p.version, value, issues };
      for (const op of p.ops) {
        const path = op.type === "deleteRow" ? [...op.path, { id: op.id }] : op.path;
        if (op.type === "deleteRow" || op.type === "remove") removed.push(path);
        if (op.type !== "insertRow" && op.type !== "moveRow") paths.add(pathKey(path));
      }
      touched = true;
    }
    return finish(true);
  }
  private recover() { void this.resync().catch(() => {}); }
  private timeout() {
    return new DocumentError("unknown_outcome", "Document publication recovery timed out; retry loading current state before editing");
  }
  /** Recovery only reads owner state. Accepted mutations are never replayed. */
  resync(): Promise<void> {
    if (this.resyncing) return this.resyncing;
    clearTimeout(this.watchdog);
    const deadline = Math.min(Date.now() + this.policy.deadlineMS, ...this.waiters.map(w => w.deadline));
    const work = (async () => {
      let delay = this.policy.retryMS;
      try {
        while (true) {
          if (Date.now() >= deadline) throw this.timeout();
          const generation = this.bufferGeneration;
          // Older buffered pushes are included by the snapshot requested now.
          this.takeBuffer();
          let timer: ReturnType<typeof setTimeout> | undefined;
          try {
            const opened = await Promise.race([
              this.reopen(),
              new Promise<never>((_, reject) => {
                timer = setTimeout(() => reject(this.timeout()), Math.max(0, deadline - Date.now()));
              }),
            ]);
            if (generation !== this.bufferGeneration) continue;
            this.install(opened);
            if (!this.consume(this.takeBuffer())) continue;
            this.failure = undefined;
            this.changed("all");
            this.settle();
            return;
          } catch (error) {
            if (isRejected(error) || isDocumentError(error) && ["owner_invalidated", "owner_replaced", "closing"].includes(error.code)) throw error;
            if (Date.now() >= deadline) throw this.timeout();
          } finally { clearTimeout(timer); }
          await new Promise(resolve => setTimeout(resolve, Math.min(delay, Math.max(0, deadline - Date.now()))));
          delay = Math.min(delay * 2, this.policy.maxRetryMS);
        }
      } catch (error) {
        this.failure = error instanceof Error ? error : new Error(String(error));
        this.rejectAll(this.failure);
        this.changed("all");
        throw this.failure;
      }
    })();
    this.resyncing = work.finally(() => { this.resyncing = undefined; this.armWatchdog(); });
    return this.resyncing;
  }
  assertWritable() { if (this.failure) throw this.failure; }
  reached(sequence: number): Promise<void> {
    if (this.failure) return Promise.reject(this.failure);
    if (this.state.sequence >= sequence && !this.resyncing) return Promise.resolve();
    return new Promise((resolve, reject) => {
      this.waiters.push({ sequence, deadline: Date.now() + this.policy.deadlineMS, resolve, reject });
      this.armWatchdog();
    });
  }
  private rejectAll(error: unknown) {
    clearTimeout(this.watchdog);
    const waiters = this.waiters;
    this.waiters = [];
    for (const waiter of waiters) waiter.reject(error);
  }
  private armWatchdog() {
    clearTimeout(this.watchdog);
    if (this.resyncing || this.failure || !this.waiters.length) return;
    this.watchdog = setTimeout(() => this.recover(), Math.max(0, Math.min(this.policy.stallMS,
      ...this.waiters.map(w => w.deadline - Date.now()))));
  }
  private settle() {
    this.waiters = this.waiters.filter(waiter => {
      if (this.state.sequence < waiter.sequence) return true;
      waiter.resolve();
      return false;
    });
    this.armWatchdog();
  }
}
