// The page's immutable projection of owner state. No CRDT: it applies the owner's
// publications in sequence order and resyncs from a fresh snapshot on any gap.
import type { OwnerPatchOp, OwnerPublication, OwnerState, PagePush } from "@hitslop/schema/owner";

export type Segment = string | { id: string };

// Snapshot arrays are immutable, so one `$id → index` map per array serves every reader.
const rowIndexes = new WeakMap<readonly any[], Map<string, number>>();
function indexOf(rows: readonly any[], id: string): number {
  let index = rowIndexes.get(rows);
  if (!index) {
    index = new Map();
    rows.forEach((row, i) => {
      if (typeof row?.$id === "string" && !index!.has(row.$id)) index!.set(row.$id, i);
    });
    rowIndexes.set(rows, index);
  }
  return index.get(id) ?? -1;
}
export function readPath(value: any, path: readonly Segment[]) {
  for (const part of path) {
    if (typeof part === "string") value = value?.[part];
    else if (!Array.isArray(value)) return undefined;
    else value = value[indexOf(value, part.id)];
  }
  return value;
}
function freeze(value: any, fresh: Set<object>) {
  if (!value || typeof value !== "object" || Object.isFrozen(value)) return;
  if (!fresh.has(value)) return;
  for (const child of Object.values(value)) freeze(child, fresh);
  Object.freeze(value);
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
      const key = typeof part === "string" ? part : indexOf(node, part.id);
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
      case "set": {
        if (!op.path.length) {
          root = op.value;
          break;
        }
        const parent = owned(parentPath);
        const key = typeof last === "string" ? last : indexOf(parent, last.id);
        if (typeof key === "number" && key < 0) throw new Error("Missing publication row");
        parent[key] = op.value;
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
  // New values from the owner (inserted rows, set containers) are fresh as well.
  for (const op of ops) if ("value" in op && op.value && typeof op.value === "object") deepFreeze(op.value);
  freeze(root, fresh);
  return root;
}

export type Changes = { paths: Set<string> } | "all";

/** Sequence-ordered state plus save status, fed by `open` and the push stream. */
export class Store {
  state: OwnerState;
  savedSequence: number;
  saveFailure: string | null;
  private opened = false;
  private buffered: PagePush[] = [];
  private resyncing?: Promise<void>;
  private waiters: { sequence: number; resolve(): void; reject(error: unknown): void }[] = [];

  constructor(
    private readonly reopen: () => Promise<{ state: OwnerState; savedSequence: number; saveFailure: string | null }>,
    private readonly changed: (changes: Changes) => void,
  ) {
    this.state = { sequence: 0, version: "", value: undefined, issues: [] };
    this.savedSequence = 0;
    this.saveFailure = null;
  }
  /** Installs an `open` reply, then applies pushes that arrived before it. */
  load(opened: { state: OwnerState; savedSequence: number; saveFailure: string | null }) {
    deepFreeze(opened.state.value);
    deepFreeze(opened.state.issues);
    this.state = opened.state;
    this.savedSequence = Math.max(this.opened ? this.savedSequence : 0, opened.savedSequence);
    this.saveFailure = opened.saveFailure;
    this.opened = true;
    const buffered = this.buffered;
    this.buffered = [];
    this.settle();
    this.changed("all");
    if (buffered.length) this.publish(buffered);
  }
  /** Processes pushes in order; publications at or below the current sequence are old. */
  publish(pushes: readonly PagePush[]) {
    if (!this.opened || this.resyncing) {
      this.buffered.push(...pushes);
      return;
    }
    const paths = new Set<string>();
    let touched = false;
    for (const [i, push] of pushes.entries()) {
      if (push.type === "saved") {
        this.savedSequence = Math.max(this.savedSequence, push.sequence);
        this.saveFailure = null;
        touched = true;
      } else if (push.type === "failed") {
        this.saveFailure = push.error;
        touched = true;
      } else {
        const p: OwnerPublication = push.publication;
        if (p.sequence <= this.state.sequence) continue;
        if (p.previous !== this.state.sequence) {
          // A gap: never guess. Reload the snapshot; later pushes wait for it.
          if (touched) this.changed({ paths });
          this.buffered.push(...pushes.slice(i));
          void this.resync();
          return;
        }
        let value;
        try {
          value = applyOps(this.state.value, p.ops);
        } catch {
          this.buffered.push(...pushes.slice(i));
          void this.resync();
          return;
        }
        deepFreeze(p.issues);
        this.state = { sequence: p.sequence, version: p.version, value, issues: p.issues };
        for (const op of p.ops) {
          const path = op.type === "deleteRow" ? [...op.path, { id: op.id }] : op.path;
          // Inserting or moving a row changes no bound field; rows are found by ID.
          if (op.type !== "insertRow" && op.type !== "moveRow") paths.add(JSON.stringify(path));
        }
        touched = true;
      }
    }
    this.settle();
    if (touched) this.changed({ paths });
  }
  /** Replaces the state with a fresh snapshot after a gap or a failed apply. */
  resync(): Promise<void> {
    return (this.resyncing ??= (async () => {
      try {
        const opened = await this.reopen();
        const buffered = this.buffered;
        this.buffered = [];
        this.resyncing = undefined;
        this.opened = false;
        this.load(opened);
        if (buffered.length) this.publish(buffered);
      } catch (error) {
        this.resyncing = undefined;
        this.rejectAll(error);
      }
    })());
  }
  /** Resolves once the state includes publication `sequence`. */
  reached(sequence: number): Promise<void> {
    if (this.state.sequence >= sequence) return Promise.resolve();
    return new Promise((resolve, reject) => this.waiters.push({ sequence, resolve, reject }));
  }
  rejectAll(error: unknown) {
    const waiters = this.waiters;
    this.waiters = [];
    for (const waiter of waiters) waiter.reject(error);
  }
  private settle() {
    this.waiters = this.waiters.filter((waiter) => {
      if (this.state.sequence < waiter.sequence) return true;
      waiter.resolve();
      return false;
    });
  }
}
