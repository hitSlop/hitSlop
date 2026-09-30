import type { OwnerIntent, OwnerPath as Path, OwnerState } from "@hitslop/schema/owner";
import type { Handle, At } from "../handle-types";
import { isScalar, schemaKey, unwrap, type Definition, type Node, type ObjectNode, type Value } from "../schema";
import { applyOps, readPath, Store, type Changes, type Segment } from "./store";
import { bindText as bindField } from "./text";
import type { OwnerTransport } from "./transport";
import { newID } from "../identity";
import type { AsyncHandle } from "../async-types";
export type { AsyncHandle } from "../async-types";
export type { OwnerTransport } from "./transport";
export type OwnerScope<N extends ObjectNode> = { readonly fields: Handle<N>; readonly at: At };
type Collector = (intent: OwnerIntent) => void;

/** Page-side document state. It contains no CRDT and never writes persistent JSON. */
export class OwnerDocument<N extends ObjectNode> {
  readonly key: string;
  readonly fields: AsyncHandle<Handle<N>>;
  readonly id: string;
  private readonly store: Store;
  private readonly paths = new WeakMap<object, { node: Node; path: Path }>();
  private readonly handles = new Map<string, any>();
  private readonly handlePaths = new WeakMap<object, { node: Node; path: Path }>();
  private readonly bindings = new Set<ReturnType<typeof bindField>>();
  private readonly listeners = new Set<() => void>();
  private readonly pathListeners = new Map<string, Set<() => void>>();
  /** Work the close and capture barriers wait for: writes, text sends, attachments. */
  private readonly pending = new Set<Promise<unknown>>();
  private tail: Promise<unknown> = Promise.resolve();
  private collecting = false;
  private blocked = false;
  /** Local-only values shown over the store (drags, drawing) until set or flush. */
  private readonly previews = new Map<string, { path: Path; value: unknown }>();
  private presented: unknown;

  private constructor(
    private readonly definition: Definition<N>,
    private readonly transport: OwnerTransport,
    private readonly reportError: (error: unknown, kind?: "application" | "operation") => void,
  ) {
    this.key = schemaKey(definition.descriptor);
    this.id = transport.id ?? crypto.randomUUID();
    this.store = new Store(
      () => transport.open(),
      (changes) => this.changed(changes),
    );
    this.fields = this.handle(definition.descriptor.root, [], undefined);
  }
  static async open<N extends ObjectNode>(
    definition: Definition<N>,
    transport: OwnerTransport,
    reportError: (error: unknown, kind?: "application" | "operation") => void = console.error,
  ) {
    const doc = new OwnerDocument(definition, transport, reportError);
    // The receiver exists before `open`, so no push after the snapshot is missed.
    transport.onPush((pushes) => doc.store.publish(pushes));
    doc.store.load(await transport.open());
    return doc;
  }
  get current(): Value<N> {
    return (this.previews.size ? this.presented : this.store.state.value) as Value<N>;
  }
  get issues(): OwnerState["issues"] {
    return this.store.state.issues;
  }
  get status(): "pending" | "saved" | "save-failed" {
    if (this.store.saveFailure) return "save-failed";
    return this.store.state.sequence > this.store.savedSequence || this.pending.size || this.previews.size
      ? "pending"
      : "saved";
  }
  get error() {
    return this.store.saveFailure;
  }
  subscribe(listener: () => void) {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }
  private report(error: unknown, kind: "application" | "operation" = "operation") {
    try {
      this.reportError(error, kind);
    } catch {
      /* Error observers cannot reject an accepted operation. */
    }
  }
  private notify() {
    for (const listener of this.listeners) {
      try {
        listener();
      } catch (error) {
        this.report(error, "application");
      }
    }
  }
  /** Bindings refresh only when a publication touches their path (or on a full refresh). */
  private subscribePath(path: Path, listener: () => void) {
    const key = JSON.stringify(path);
    let set = this.pathListeners.get(key);
    if (!set) this.pathListeners.set(key, (set = new Set()));
    set.add(listener);
    return () => {
      set!.delete(listener);
      if (!set!.size) this.pathListeners.delete(key);
    };
  }
  private notifyPaths(changes: Changes) {
    for (const [key, set] of this.pathListeners) {
      if (changes !== "all") {
        // A touched path refreshes its own bindings and every binding beneath it.
        let hit = false;
        for (const touched of changes.paths) {
          if (key === touched || key.startsWith(touched.slice(0, -1) + ",")) {
            hit = true;
            break;
          }
        }
        if (!hit) continue;
      }
      for (const listener of set) {
        try {
          listener();
        } catch (error) {
          this.report(error, "application");
        }
      }
    }
  }
  private changed(changes: Changes) {
    this.present();
    this.notify();
    this.notifyPaths(changes);
  }
  /** Applies previews over the store's snapshot. A preview whose row is gone is dropped. */
  private present() {
    let value = this.store.state.value;
    for (const [key, preview] of this.previews) {
      try {
        value = applyOps(value, [{ type: "set", path: preview.path, value: preview.value } as any]);
      } catch {
        this.previews.delete(key);
      }
    }
    this.presented = value;
    this.register(value, this.definition.descriptor.root, []);
  }
  private setPreview(path: Path, value: unknown) {
    if (this.transport.readOnly) throw new Error("Read-only document");
    if (this.blocked) throw new Error("Document barrier is active");
    if (this.collecting) throw new Error("Previews are not transaction writes");
    const key = JSON.stringify(path);
    this.previews.set(key, { path, value: structuredClone(value) });
    this.present();
    this.notify();
    this.notifyPaths({ paths: new Set([key]) });
  }
  /** Drops the preview for `path` once a write covering it is accepted, unless a newer
   * preview replaced it meanwhile. */
  private settlePreview(key: string, preview: { path: Path; value: unknown } | undefined) {
    if (!preview || this.previews.get(key) !== preview) return;
    this.previews.delete(key);
    this.present();
    this.notify();
    this.notifyPaths({ paths: new Set([key]) });
  }
  /** Maps snapshot objects to their paths for `at`. Stops at objects already seen, so
   * the cost follows the objects a publication created. */
  private register(value: any, node: Node, path: Path) {
    if (value === null || typeof value !== "object" || this.paths.has(value)) return;
    this.paths.set(value, { node, path });
    const inner = unwrap(node);
    if (inner.kind === "object" && !Array.isArray(value)) {
      for (const [key, child] of Object.entries(inner.properties))
        this.register(value[key], child, [...path, key]);
    } else if (inner.kind === "list" && Array.isArray(value)) {
      for (const row of value)
        if (row && typeof row === "object" && typeof row.$id === "string")
          this.register(row, inner.item, [...path, { id: row.$id }]);
    }
  }
  /** Tracks work for barriers and status; `notify` reports the pending transition. */
  private track<T>(work: Promise<T>): Promise<T> {
    this.pending.add(work);
    if (this.pending.size === 1) this.notify();
    const done = () => {
      this.pending.delete(work);
      if (!this.pending.size) this.notify();
    };
    work.then(done, done);
    return work;
  }
  /** One FIFO queue for handle writes, so `insert` then `move` cannot reorder. */
  private submit<R>(intents: OwnerIntent[], result: R, admitted = false): Promise<R> {
    if (this.transport.readOnly) return Promise.reject(new Error("Read-only document"));
    if (this.blocked && !admitted) return Promise.reject(new Error("Document barrier is active"));
    if (this.collecting) throw new Error("Use tx handles inside change()");
    const batch = structuredClone({ intents });
    const run = this.tail.then(async () => {
      const reply = await this.transport.apply(batch);
      await this.store.reached(reply.sequence);
      return result;
    });
    this.tail = run.catch(() => {});
    return this.track(run).catch((error) => {
      this.report(error);
      throw error;
    });
  }
  at = (<T extends Node>(value: object) => {
    const location = this.paths.get(value);
    if (!location) throw new Error("Value is not a snapshot from this document");
    return this.handle(location.node, location.path, undefined);
  }) as <T extends Node>(value: import("../schema").Snapshot<T>) => AsyncHandle<Handle<T>>;

  /** Collects intents synchronously, then submits them as one batch. */
  private collect<R>(
    callback: (tx: OwnerScope<N>) => R,
  ): { intents: OwnerIntent[]; result: R } {
    if (this.collecting) throw new Error("Nested change() is not supported");
    const intents: OwnerIntent[] = [];
    let active = true;
    const collect: Collector = (intent) => {
      if (!active) throw new Error("Transaction handle escaped change()");
      intents.push(structuredClone(intent));
    };
    this.collecting = true;
    try {
      const tx = {
        fields: this.handle(this.definition.descriptor.root, [], collect),
        at: (value: object) => {
          if (!active) throw new Error("Transaction handle escaped change()");
          const location = this.paths.get(value);
          if (!location) throw new Error("Value is not a snapshot from this document");
          return this.handle(location.node, location.path, collect);
        },
      } as OwnerScope<N>;
      const result = callback(tx);
      if (result && typeof (result as any).then === "function") {
        // Observe an async callback's eventual failure without submitting any work.
        Promise.resolve(result).catch((error) => this.report(error));
        throw new Error("change() callback must be synchronous");
      }
      return { intents, result };
    } finally {
      active = false;
      this.collecting = false;
    }
  }
  change<R>(callback: (tx: OwnerScope<N>) => R, _options?: { message?: string }): Promise<R> {
    if (this.transport.readOnly) return Promise.reject(new Error("Read-only document"));
    if (this.blocked) return Promise.reject(new Error("Document barrier is active"));
    if (this.collecting) throw new Error("Nested change() is not supported");
    let collected;
    try {
      collected = this.collect(callback);
    } catch (error) {
      this.report(error);
      return Promise.reject(error);
    }
    return this.submit(collected.intents, collected.result);
  }
  /**
   * Stores a blob, then submits the collector's edits that reference it. It joins the
   * barrier's pending set before its first await, and its edits pass an active barrier,
   * so close or capture can never save the blob without its reference.
   */
  admit<R, T>(
    store: () => Promise<T>,
    callback: (tx: OwnerScope<N>, stored: T) => R,
  ): Promise<T> {
    if (this.transport.readOnly) return Promise.reject(new Error("Read-only document"));
    if (this.blocked) return Promise.reject(new Error("Document barrier is active"));
    return this.track(
      (async () => {
        const stored = await store();
        const { intents } = this.collect((tx) => callback(tx, stored));
        await this.submit(intents, undefined, true);
        return stored;
      })(),
    );
  }

  private handle(node: Node, path: Path, collect: Collector | undefined): any {
    if (collect) return this.makeHandle(node, path, collect);
    const key = JSON.stringify(path);
    let handle = this.handles.get(key);
    if (!handle) {
      handle = this.makeHandle(node, path, undefined);
      this.handles.set(key, handle);
      this.handlePaths.set(handle, { node, path });
    }
    return handle;
  }
  private makeHandle(node: Node, path: Path, collect: Collector | undefined): any {
    const send = <R>(intent: OwnerIntent, result: R) => {
      if (collect) {
        collect(intent);
        return result;
      }
      return this.submit([intent], result);
    };
    const key = JSON.stringify(path);
    const set = (value: unknown) => {
      if (collect) return send({ type: "set", path, value }, undefined);
      const preview = this.previews.get(key);
      return this.submit([{ type: "set", path, value }], undefined).then(() => this.settlePreview(key, preview));
    };
    const scalar = () => ({ set, preview: (value: unknown) => this.setPreview(path, value) });
    if (node.kind === "optional") {
      const clear = () => {
        if (collect) return send({ type: "clear", path }, undefined);
        const preview = this.previews.get(key);
        return this.submit([{ type: "clear", path }], undefined).then(() => this.settlePreview(key, preview));
      };
      if (isScalar(node.inner)) return Object.freeze({ ...scalar(), clear });
      // An optional object: its fields, plus `set` to create or replace it and `clear`.
      const fields = this.makeHandle(node.inner, path, collect);
      const handle = Object.create(null);
      for (const name of Object.keys(fields))
        Object.defineProperty(handle, name, { enumerable: true, get: () => fields[name] });
      Object.defineProperty(handle, "set", { value: (value: unknown) => send({ type: "set", path, value }, undefined) });
      Object.defineProperty(handle, "clear", { value: clear });
      return Object.freeze(handle);
    }
    if (isScalar(node)) return Object.freeze(scalar());
    switch (node.kind) {
      case "object": {
        // Children are built on first access and cached with their path.
        const children: Record<string, any> = {};
        for (const key of Object.keys(node.properties))
          Object.defineProperty(children, key, {
            enumerable: true,
            get: () => this.handle(node.properties[key]!, [...path, key], collect),
          });
        return Object.freeze(children);
      }
      case "counter": {
        const increment = (by = 1) => send({ type: "increment", path, by }, undefined);
        return Object.freeze({ increment, decrement: (by = 1) => increment(-by) });
      }
      case "list": {
        return Object.freeze({
          item: (id: string) => this.handle(node.item, [...path, { id }], collect),
          insert: (value: unknown, at?: { before: string } | { after: string }) => {
            const id = newID();
            return send({ type: "insert", path, value, id, ...(at ? { at } : {}) }, Object.freeze({ id }));
          },
          remove: (id: string) => send({ type: "remove", path, id }, undefined),
          move: (id: string, at?: { before: string } | { after: string }) =>
            send({ type: "move", path, id, ...(at ? { at } : {}) }, undefined),
        });
      }
      case "text":
        // Whole-field replacement of the text as it is when the owner runs it.
        return Object.freeze({ set: (value: string) => send({ type: "set", path, value }, undefined) });
      default:
        throw new Error(`Unsupported descriptor: ${(node as Node).kind}`);
    }
  }
  /** Drains bindings, queued writes and attachment work, then saves once. A barrier
   * never joins a flush that already passed its drain point. */
  private async drainAndSave(): Promise<void> {
    for (const binding of this.bindings) binding.commit();
    // Previews commit as one batch; each stays shown until its write is accepted.
    const previews = [...this.previews.entries()];
    if (previews.length)
      void this.submit(
        previews.map(([, p]) => ({ type: "set" as const, path: p.path, value: p.value }) as OwnerIntent),
        undefined,
        true,
      ).then(
        () => previews.forEach(([key, preview]) => this.settlePreview(key, preview)),
        () => {},
      );
    while (this.pending.size) await Promise.allSettled([...this.pending]);
    const target = this.store.state.sequence;
    try {
      await this.transport.flush();
    } catch (error) {
      this.store.saveFailure = String(error instanceof Error ? error.message : error);
      this.notify();
      throw error;
    }
    this.store.savedSequence = Math.max(this.store.savedSequence, target);
    this.store.saveFailure = null;
    this.notify();
  }
  flush(): Promise<void> {
    if (this.collecting) throw new Error("Cannot flush inside change()");
    return this.drainAndSave();
  }
  /** Close and capture: refuse new work, drain what is pending, then save. Inputs stay
   * enabled so focus and composition survive a barrier that is cancelled. */
  async prepareClose() {
    this.blocked = true;
    this.notify();
    try {
      await this.drainAndSave();
    } catch (error) {
      this.blocked = false;
      this.notify();
      throw error;
    }
  }
  cancelClose() {
    this.blocked = false;
    this.notify();
  }
  async close() {
    await this.prepareClose();
  }
  /**
   * Binds a form control to a scalar. Checkboxes and selects commit on `change`; ranges
   * preview while dragging and commit on `change`; number inputs commit on `change`;
   * text, date and time inputs commit on `input`. Writes are coalesced: one in flight,
   * the latest value wins. An empty value clears an optional field; an invalid one
   * reverts to the document's value.
   */
  bindValue(element: HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement, initial: object) {
    let location!: { node: Node; path: Path };
    let latest: { clear: true } | { value: unknown } | undefined;
    let running: Promise<void> | undefined;
    let destroyed = false;
    const input = element as HTMLInputElement;
    // `<select>` reports "select-one"; a textarea reports "textarea" and commits on input.
    const raw = String((element as { type?: string }).type ?? "text");
    const type = raw.startsWith("select") ? "select" : raw;
    const resolve = (next: object) => {
      const found = this.handlePaths.get(next);
      if (!found || !isScalar(unwrap(found.node))) throw new Error("Expected this document's scalar handle");
      location = found;
    };
    const kind = () => unwrap(location.node) as import("../schema").Scalar;
    const optional = () => location.node.kind === "optional";
    const sync = () => {
      if (running) return;
      const value = readPath(this.current, location.path as Segment[]);
      const expected = { boolean: "boolean", string: "string", enum: "string", number: "number", integer: "number" }[kind().kind];
      const valid = value === undefined ? optional() : typeof value === expected;
      const disabled = !!this.transport.readOnly || !valid;
      if (element.disabled !== disabled) element.disabled = disabled;
      if (type === "checkbox") input.checked = value === true;
      else {
        const text = value === undefined ? "" : String(value);
        if (element.value !== text) element.value = text;
      }
    };
    /** The control's value as a write, or undefined when it cannot be written. */
    const parse = (): { clear: true } | { value: unknown } | undefined => {
      const k = kind();
      if (type === "checkbox") return k.kind === "boolean" ? { value: input.checked } : undefined;
      const raw = element.value;
      if (raw === "" && optional()) return { clear: true };
      switch (k.kind) {
        case "boolean":
          return raw === "true" || raw === "false" ? { value: raw === "true" } : undefined;
        case "number":
        case "integer": {
          const n = raw.trim() === "" ? NaN : Number(raw);
          if (!Number.isFinite(n) || (k.kind === "integer" && !Number.isSafeInteger(n))) return undefined;
          return { value: n };
        }
        default:
          return { value: raw };
      }
    };
    const intent = (write: { clear: true } | { value: unknown }): OwnerIntent =>
      "clear" in write ? { type: "clear", path: location.path } : { type: "set", path: location.path, value: write.value };
    const commit = () => {
      if (destroyed) return;
      const write = parse();
      if (!write || this.blocked || this.transport.readOnly) return void (running ? undefined : sync());
      latest = write;
      if (running) return;
      running = (async () => {
        while (latest) {
          const next = latest;
          latest = undefined;
          const key = JSON.stringify(location.path);
          const preview = this.previews.get(key);
          try {
            await this.submit([intent(next)], undefined, true);
            this.settlePreview(key, preview);
          } catch {
            /* submit already reports the rejection centrally; sync shows the document value. */
            this.settlePreview(key, preview);
          }
        }
      })().finally(() => {
        running = undefined;
        if (!destroyed) sync();
      });
      void this.track(running);
    };
    const preview = () => {
      const write = parse();
      if (write && "value" in write && !this.blocked) this.setPreview(location.path, write.value);
    };
    const events: [string, () => void][] =
      type === "range"
        ? [["input", preview], ["change", commit]]
        : type === "checkbox" || type === "select" || type === "number"
          ? [["change", commit]]
          : [["input", commit]];
    resolve(initial);
    let stop = this.subscribePath(location.path, sync);
    for (const [name, listener] of events) element.addEventListener(name, listener);
    sync();
    return {
      update: (next: object) => {
        resolve(next);
        stop();
        stop = this.subscribePath(location.path, sync);
        sync();
      },
      destroy: () => {
        destroyed = true;
        stop();
        for (const [name, listener] of events) element.removeEventListener(name, listener);
      },
    };
  }
  bindText(element: HTMLInputElement | HTMLTextAreaElement, handle: object) {
    const locate = (next: object) => {
      const location = this.handlePaths.get(next);
      if (!location || location.node.kind !== "text") throw new Error("Expected this document's text handle");
      return location.path;
    };
    let path = locate(handle);
    const binding = bindField(element, path as Segment[], {
      read: (at) => ({
        text: readPath(this.store.state.value, at),
        version: this.store.state.version,
        sequence: this.store.state.sequence,
      }),
      send: (request) => this.transport.text(request),
      reached: (sequence) => this.store.reached(sequence),
      readOnly: () => !!this.transport.readOnly,
      track: (work) => void this.track(work),
      report: (error) => this.report(error),
    });
    this.bindings.add(binding);
    let stop = this.subscribePath(path, () => binding.refresh());
    return {
      update: (next: object) => {
        const nextPath = locate(next);
        if (JSON.stringify(nextPath) === JSON.stringify(path)) return;
        stop();
        path = nextPath;
        binding.retarget(path as Segment[]);
        stop = this.subscribePath(path, () => binding.refresh());
      },
      destroy: () => {
        stop();
        binding.destroy();
        this.bindings.delete(binding);
      },
    };
  }
}
