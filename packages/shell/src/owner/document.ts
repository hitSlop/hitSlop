import { isRejected } from "@hitslop/document";
import { DocumentError } from "@hitslop/document/internal";
import type { OwnerIntent, OwnerPath as Path, OwnerState } from "@hitslop/schema/core";
import type { Handle } from "@hitslop/document/internal";
import type { Definition, Node, ObjectNode, Value } from "@hitslop/document";
import { unwrap } from "@hitslop/document/internal";
import type { Segment } from "@hitslop/document";
import { Store, type Changes } from "./store";
import { readPath, pathKey, atOrBeneath } from "./path";
import { bindText as bindField } from "./text";
import type { OwnerTransport } from "./transport";
import { handleFactory, type Collector } from "./handles";
import { commitIntent, Previews, type Preview } from "./previews";
export type { OwnerTransport } from "./transport";
import type { Scope as OwnerScope } from "@hitslop/document/abi";

const readOnly = () => new DocumentError("rejected", "Read-only document");
const barrier = () => new DocumentError("closing", "Document barrier is active");
/** How long an assigned `value` waits for the next assignment before it commits, like
 * the host's autosave idle. */
const settleMS = 150;

/** Page-side document state. It contains no CRDT and never writes persistent JSON. */
export class OwnerDocument<N extends ObjectNode> {
  readonly fields: Handle<N>;
  private readonly store: Store;
  private readonly paths = new WeakMap<object, { node: Node; path: Path }>();
  private readonly handles = new Map<string, any>();
  private readonly handlePaths = new WeakMap<object, { node: Node; path: Path }>();
  /** Each binding's drain: sends its unsent edit before a barrier saves. */
  private readonly drains = new Set<() => Promise<void>>();
  private readonly listeners = new Set<() => void>();
  private readonly pathListeners = new Map<string, Set<() => void>>();
  /** Work the close and capture barriers wait for: writes, text sends, attachments. */
  private readonly pending = new Set<Promise<unknown>>();
  /** One FIFO queue for handle writes, so `insert` then `move` cannot reorder. */
  private tail: Promise<unknown> = Promise.resolve();
  private collecting = false;
  private blocked = false;
  /** Local-only values shown over the store: drags and drawing, live scalar writes
   * awaiting acceptance, and assigned values awaiting commit. */
  private readonly previews = new Previews();
  /** Commit timers for assigned values, by path key. */
  private readonly settling = new Map<string, ReturnType<typeof setTimeout>>();
  private presented: unknown;
  /** Lets a framework record a dependency when a handle's `value` is read. */
  private observer: () => void = () => {};

  private readonly makeHandle = handleFactory({
    handle: (node, path, collect) => this.handle(node, path, collect),
    submit: (intents, result) => this.submit(intents, result),
    write: (intent) => this.writeShown(intent),
    preview: (path, value) => this.setPreview(path, value),
    read: (path) => {
      this.observer();
      return readPath(this.current, path);
    },
    assign: (path, value, optional) => this.assign(path, value, optional),
  });

  private constructor(
    private readonly definition: Definition<N>,
    private readonly transport: OwnerTransport,
    private readonly reportError: (error: unknown, kind?: "application" | "operation") => void,
  ) {
    this.store = new Store(
      () => transport.open(),
      (changes) => this.changed(changes),
    );
    this.fields = this.handle(definition.descriptor, [], undefined);
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
  /** Effective palette from the same ordered owner frame as the document. */
  get theme(): OwnerState["theme"] {
    return this.store.state.theme;
  }
  subscribe(listener: () => void) {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }
  observe(read: () => void) {
    this.observer = read;
    return () => {
      if (this.observer === read) this.observer = () => {};
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
    const key = pathKey(path);
    let set = this.pathListeners.get(key);
    if (!set) this.pathListeners.set(key, (set = new Set()));
    set.add(listener);
    return () => {
      set!.delete(listener);
      if (!set!.size) this.pathListeners.delete(key);
    };
  }
  private notifyPaths(changes: Changes) {
    // A touched path refreshes its own bindings and every binding beneath it.
    const touched = changes === "all" ? undefined : [...changes.paths];
    for (const [key, set] of this.pathListeners) {
      if (touched && !touched.some(path => atOrBeneath(key, path))) continue;
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
    // Handles held by callers remain valid path references, but deleted rows/entries
    // must not stay strongly retained by the document's cache for its whole lifetime.
    if (changes === "all") this.handles.clear();
    else for (const path of changes.removed ?? []) {
      const key = pathKey(path);
      for (const cached of this.handles.keys())
        if (atOrBeneath(cached, key)) this.handles.delete(cached);
    }
    this.present();
    this.notify();
    this.notifyPaths(changes);
  }
  private present() {
    this.presented = this.previews.overlay(this.store.state.value);
    this.register(this.presented, this.definition.descriptor, []);
  }
  private refusal(admitted = false) {
    return this.transport.readOnly ? readOnly() : this.blocked && !admitted ? barrier() : undefined;
  }
  private assertNotCollecting(message = "Use tx handles inside change()") {
    if (this.collecting) throw new Error(message);
  }
  private locate(value: object) {
    const location = this.paths.get(value);
    if (!location) throw new Error("Value is not a snapshot from this document");
    return location;
  }
  private setPreview(path: Path, value: unknown) {
    this.assertNotCollecting("Previews are not transaction writes");
    const refusal = this.refusal();
    if (refusal) throw refusal;
    this.show(path, value);
  }
  private show(path: Path, value: unknown) {
    const key = this.previews.set(path, value);
    this.present();
    this.notify();
    this.notifyPaths({ paths: new Set([key]) });
    return key;
  }
  /** A live scalar write (`set`, `clear`, a list element's `set`): shown at once,
   * settled when accepted, reverted when refused. */
  private writeShown(intent: OwnerIntent) {
    this.assertNotCollecting();
    const refusal = this.refusal();
    if (refusal) return Promise.reject(refusal);
    const key = this.show(intent.path, intent.type === "set" ? intent.value : undefined);
    return this.writePreview(key, this.previews.get(key), intent);
  }
  /** `handle.value = v`: shown at once and committed once assignments pause, or at the
   * next barrier. No caller awaits it, so a refusal is reported and the value reverts.
   * `null` (an emptied number input) clears an optional and is ignored otherwise. */
  private assign(path: Path, value: unknown, optional: boolean) {
    this.assertNotCollecting();
    if (value === null) value = undefined;
    if ((value === undefined && !optional) || this.refusal()) return;
    const key = this.show(path, value);
    clearTimeout(this.settling.get(key));
    this.settling.set(key, setTimeout(() => {
      this.settling.delete(key);
      const preview = this.previews.get(key);
      // An active barrier commits the preview itself.
      if (preview && !this.blocked)
        this.writePreview(key, preview, commitIntent(preview)).catch((error) => this.report(error));
    }, settleMS));
  }
  private settlePreviews(covered: ReadonlyMap<string, Preview>) {
    const settled = this.previews.settle(covered);
    if (!settled.size) return;
    this.present();
    this.notify();
    this.notifyPaths({ paths: settled });
  }
  /** Commits one preview's value. A refused value can never be saved, so the saved value
   * shows again; an unknown outcome keeps the preview for the barrier to report. */
  private writePreview(
    key: string,
    preview: Preview | undefined,
    intent: OwnerIntent,
    admitted = false,
  ) {
    return this.submit([intent], undefined, { admitted }).then(
      () => {},
      error => {
        if (isRejected(error) && preview) this.settlePreviews(new Map([[key, preview]]));
        throw error;
      },
    );
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
      // Rows a publication kept are already mapped: skip them before building a path.
      for (const row of value)
        if (row && typeof row === "object" && typeof row.$id === "string" && !this.paths.has(row))
          this.register(row, inner.item, [...path, { id: row.$id }]);
    } else if (inner.kind === "record" && !Array.isArray(value)) {
      // Object entries resolve with `doc.at(entry)`, addressed by their key.
      for (const [key, entry] of Object.entries(value))
        if (!this.paths.has(entry as object)) this.register(entry, inner.value, [...path, key]);
    }
  }
  /** Tracks work the close and capture barriers wait for. */
  private track<T>(work: Promise<T>): Promise<T> {
    this.pending.add(work);
    const done = () => this.pending.delete(work);
    work.then(done, done);
    return work;
  }
  /** `collected` intents were already copied when their collector ran. */
  private submit<R>(intents: OwnerIntent[], result: R, { admitted = false, collected = false }: { admitted?: boolean; collected?: boolean } = {}): Promise<R> {
    this.assertNotCollecting();
    const refusal = this.refusal(admitted);
    if (refusal) return Promise.reject(refusal);
    const batch = { intents: collected ? intents : structuredClone(intents) };
    const covered = this.previews.coveredBy(intents);
    const run = this.tail.then(async () => {
      this.store.assertWritable();
      const reply = await this.transport.apply(batch);
      await this.store.reached(reply.sequence);
      this.settlePreviews(covered);
      return result;
    });
    this.tail = run.catch(() => {});
    // Return a distinct promise: internal queue/barrier observers must not swallow
    // an unhandled rejection on the author's promise.
    return this.track(run).then(value => value);
  }
  at = ((value: object) => {
    const location = this.locate(value);
    return this.handle(location.node, location.path, undefined);
  }) as <T extends Node>(value: import("@hitslop/document").Snapshot<T>) => Handle<T>;

  /** Collects intents synchronously, then submits them as one batch. */
  private collect<R>(
    callback: (tx: OwnerScope<N>) => R,
  ): { intents: OwnerIntent[]; result: R } {
    this.assertNotCollecting("Nested change() is not supported");
    const intents: OwnerIntent[] = [];
    let active = true;
    const collect: Collector = (intent) => {
      if (!active) throw new Error("Transaction handle escaped change()");
      intents.push(structuredClone(intent));
    };
    this.collecting = true;
    try {
      const tx = {
        fields: this.handle(this.definition.descriptor, [], collect),
        at: (value: object) => {
          if (!active) throw new Error("Transaction handle escaped change()");
          const location = this.locate(value);
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
  change<R>(callback: (tx: OwnerScope<N>) => R): Promise<R> {
    this.assertNotCollecting("Nested change() is not supported");
    const refusal = this.refusal();
    if (refusal) return Promise.reject(refusal);
    let collected;
    try {
      collected = this.collect(callback);
    } catch (error) {
      return Promise.reject(error);
    }
    return this.submit(collected.intents, collected.result, { collected: true });
  }
  /**
   * Stores a blob, then submits the collector's edits that reference it. It joins the
   * barrier's pending set before its first await, and its edits pass an active barrier,
   * so a close or capture never falls between the two. A collector that throws, or edits
   * the core refuses, leave the stored blob unreferenced.
   */
  admit<R, T>(
    store: () => Promise<T>,
    callback: (tx: OwnerScope<N>, stored: T) => R,
  ): Promise<T> {
    this.assertNotCollecting();
    const refusal = this.refusal();
    if (refusal) return Promise.reject(refusal);
    return this.track(
      (async () => {
        const stored = await store();
        const { intents } = this.collect((tx) => callback(tx, stored));
        await this.submit(intents, undefined, { admitted: true, collected: true });
        return stored;
      })(),
    );
  }

  private handle(node: Node, path: Path, collect: Collector | undefined): any {
    if (collect) return this.makeHandle(node, path, collect);
    const key = pathKey(path);
    let handle = this.handles.get(key);
    if (!handle) {
      handle = this.makeHandle(node, path, undefined);
      this.handles.set(key, handle);
      this.handlePaths.set(handle, { node, path });
    }
    return handle;
  }
  /** Drains bindings, queued writes and attachment work, then saves once. A barrier
   * never joins a flush that already passed its drain point. */
  private async drainAndSave(): Promise<void> {
    await this.drain();
    // The owner saves; the window shows save failures, and a failed save rejects here.
    await this.transport.flush();
  }
  /** Sends unsent text, waits for queued writes and attachment work, and commits what
   * is only shown locally: everything the person sees becomes a change. */
  private async drain(): Promise<void> {
    // Assigned values commit below, with the other previews.
    for (const timer of this.settling.values()) clearTimeout(timer);
    this.settling.clear();
    if (this.store.failure) await this.store.resync();
    await Promise.all([...this.drains].map((drain) => drain()));
    // Writes in flight settle first: an accepted live write clears its own preview, so
    // only uncommitted previews remain to write.
    while (this.pending.size) await Promise.allSettled([...this.pending]);
    // One invalid draft must not reject an unrelated, valid draft's atomic batch.
    for (const [key, preview] of this.previews.all()) {
      try {
        await this.writePreview(key, preview, commitIntent(preview), true);
      } catch (error) {
        if (!isRejected(error)) throw error;
        this.report(error);
      }
    }
    while (this.pending.size) await Promise.allSettled([...this.pending]);
  }
  flush(): Promise<void> {
    this.assertNotCollecting("Cannot flush inside change()");
    return this.drainAndSave();
  }
  /** Edit ▸ Undo: the last step, the person's or an agent's, after what the person sees
   * is sent. Resolves once `current` shows the result. */
  undo(): Promise<void> {
    return this.history("undo");
  }
  redo(): Promise<void> {
    return this.history("redo");
  }
  private async history(direction: "undo" | "redo"): Promise<void> {
    this.assertNotCollecting(`Cannot ${direction} inside change()`);
    const refusal = this.refusal();
    if (refusal) throw refusal;
    await this.drain();
    // A barrier waits for the request, so close saves what it changed.
    await this.track((async () => {
      const { sequence } = await this.transport[direction]();
      await this.store.reached(sequence);
    })());
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
  bindText(element: HTMLInputElement | HTMLTextAreaElement, handle: object) {
    const locate = (next: object) => {
      const location = this.handlePaths.get(next);
      if (!location || unwrap(location.node).kind !== "text") throw new Error("Expected this document's text handle");
      return location;
    };
    const attach = (location: ReturnType<typeof locate>) => {
      const optional = location.node.kind === "optional";
      const binding = bindField(element, location.path as Segment[], {
        read: (at) => {
          const value = readPath(this.store.state.value, at);
          const parent = readPath(this.store.state.value, at.slice(0, -1));
          return {
            text: value === undefined && optional && parent != null ? "" : value,
            version: this.store.state.version,
            sequence: this.store.state.sequence,
          };
        },
        send: (request) => { this.store.assertWritable(); return this.transport.text(request); },
        reached: (sequence) => this.store.reached(sequence),
        recover: () => this.store.resync(),
        readOnly: () => !!this.transport.readOnly,
        track: (work) => void this.track(work),
        report: (error) => this.report(error),
        undo: (redo) => void (redo ? this.redo() : this.undo()).catch((error) => this.report(error)),
      });
      const drain = async () => {
        await binding.commit();
        if (binding.detached) this.drains.delete(drain);
      };
      this.drains.add(drain);
      const stop = this.subscribePath(location.path, () => binding.refresh());
      return {
        path: location.path,
        destroy: () => {
          stop();
          // No control remains to retry a detached draft: report it once, then stop
          // holding barriers so close can proceed.
          void binding.destroy()
            .catch(error => this.report(error))
            .finally(() => this.drains.delete(drain));
        },
      };
    };
    let active = attach(locate(handle));
    let destroyed = false;
    return {
      update: (next: object) => {
        if (destroyed) return;
        const location = locate(next);
        if (pathKey(location.path) === pathKey(active.path)) return;
        active.destroy();
        active = attach(location);
      },
      destroy: () => {
        if (destroyed) return;
        destroyed = true;
        active.destroy();
      },
    };
  }
}
