import { isDocumentError, isRejected } from "../../sdk/errors";
import { DocumentError } from "../../sdk/internal";
import type { OwnerIntent, OwnerPath as Path, OwnerState } from "../../schema/core";
import type { Handle } from "../../sdk/internal";
import type { Definition, Node, ObjectNode, Value } from "../../sdk/schema";
import type { Segment } from "../../sdk/schema";
import type { UnresolvedDraft } from "../../wire/page.generated";
import { Store, type Changes } from "./store";
import { readPath, pathKey } from "./path";
import { SnapshotBindings } from "./snapshots";
import { bindText as bindField } from "./text";
import type { OwnerTransport } from "./transport";
import { handleFactory, type Collector } from "./handles";
import { commitIntent, Previews, type Preview } from "./previews";
export type { OwnerTransport } from "./transport";
import type { Scope as OwnerScope } from "../../sdk/abi";

const barrier = () => new DocumentError("closing", "Document barrier is active");
/** How long an assigned `value` waits for the next assignment before it commits, like
 * the host's autosave idle. */
const settleMS = 150;

/** Page-side document state. It contains no CRDT and never writes persistent JSON. */
export class OwnerDocument<N extends ObjectNode> {
  readonly fields: Handle<N>;
  private readonly store: Store;
  private readonly snapshots: SnapshotBindings;
  /** Each binding's drain: sends its unsent edit before a barrier saves. */
  private readonly drains = new Set<() => Promise<void>>();
  private readonly drafts: UnresolvedDraft[] = [];
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

  private readonly makeHandle = handleFactory({
    handle: (node, path, collect) => this.snapshots.handle(node, path, collect),
    submit: (intents, result) => this.submit(intents, result),
    write: (intent) => this.writeShown(intent),
    preview: (path, value) => this.setPreview(path, value),
    read: (path) => {
      this.snapshots.observed();
      return readPath(this.current, path);
    },
    assign: (path, value, optional) => this.assign(path, value, optional),
  });

  private constructor(
    private readonly definition: Definition<N>,
    private readonly transport: OwnerTransport,
    private readonly reportError: (error: unknown, kind?: "application" | "operation") => void,
  ) {
    this.snapshots = new SnapshotBindings(
      definition.descriptor,
      (node, path, collect) => this.makeHandle(node, path, collect),
      (error) => this.report(error, "application"),
    );
    this.store = new Store(
      () => transport.open(),
      (changes) => this.changed(changes),
    );
    this.fields = this.snapshots.handle(definition.descriptor, [], undefined);
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
  /** Detached text that could not be resolved, retained for Copy / Discard after close starts. */
  get unresolvedDrafts(): readonly UnresolvedDraft[] {
    return structuredClone(this.drafts);
  }
  discardUnresolvedDrafts() {
    this.drafts.length = 0;
  }
  subscribe(listener: () => void) {
    return this.snapshots.subscribe(listener);
  }
  observe(read: () => void) {
    return this.snapshots.observe(read);
  }
  private report(error: unknown, kind: "application" | "operation" = "operation") {
    try {
      this.reportError(error, kind);
    } catch {
      /* Error observers cannot reject an accepted operation. */
    }
  }
  private changed(changes: Changes) {
    this.snapshots.invalidate(changes);
    this.present();
    this.snapshots.notify();
    this.snapshots.notifyPaths(changes);
  }
  private present() {
    this.presented = this.previews.overlay(this.store.state.value);
    this.snapshots.map(this.presented);
  }
  private refusal(admitted = false) {
    return this.blocked && !admitted ? barrier() : undefined;
  }
  private assertNotCollecting(message = "Use tx handles inside change()") {
    if (this.collecting) throw new Error(message);
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
    this.snapshots.notify();
    this.snapshots.notifyPaths({ paths: new Set([key]) });
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
    this.settling.set(
      key,
      setTimeout(() => {
        this.settling.delete(key);
        const preview = this.previews.get(key);
        // An active barrier commits the preview itself.
        if (preview && !this.blocked)
          this.writePreview(key, preview, commitIntent(preview)).catch((error) =>
            this.report(error),
          );
      }, settleMS),
    );
  }
  private settlePreviews(covered: ReadonlyMap<string, Preview>) {
    const settled = this.previews.settle(covered);
    if (!settled.size) return;
    this.present();
    this.snapshots.notify();
    this.snapshots.notifyPaths({ paths: settled });
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
      (error) => {
        if (isRejected(error) && preview) this.settlePreviews(new Map([[key, preview]]));
        throw error;
      },
    );
  }
  /** Tracks work the close and capture barriers wait for. */
  private track<T>(work: Promise<T>): Promise<T> {
    this.pending.add(work);
    const done = () => this.pending.delete(work);
    work.then(done, done);
    return work;
  }
  /** `collected` intents were already copied when their collector ran. */
  private submit<R>(
    intents: OwnerIntent[],
    result: R,
    { admitted = false, collected = false }: { admitted?: boolean; collected?: boolean } = {},
  ): Promise<R> {
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
    return this.track(run).then((value) => value);
  }
  at = ((value: object) => {
    const location = this.snapshots.locate(value);
    return this.snapshots.handle(location.node, location.path, undefined);
  }) as <T extends Node>(value: import("../../sdk/schema").Snapshot<T>) => Handle<T>;

  /** Collects intents synchronously, then submits them as one batch. */
  private collect<R>(callback: (tx: OwnerScope<N>) => R): { intents: OwnerIntent[]; result: R } {
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
        fields: this.snapshots.handle(this.definition.descriptor, [], collect),
        at: (value: object) => {
          if (!active) throw new Error("Transaction handle escaped change()");
          const location = this.snapshots.locate(value);
          return this.snapshots.handle(location.node, location.path, collect);
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
  admit<R, T>(store: () => Promise<T>, callback: (tx: OwnerScope<N>, stored: T) => R): Promise<T> {
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
    while (this.pending.size) await Promise.allSettled(this.pending);
    // One invalid draft must not reject an unrelated, valid draft's atomic batch.
    for (const [key, preview] of this.previews.all()) {
      try {
        await this.writePreview(key, preview, commitIntent(preview), true);
      } catch (error) {
        if (!isRejected(error)) throw error;
        this.report(error);
      }
    }
    while (this.pending.size) await Promise.allSettled(this.pending);
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
    await this.track(
      (async () => {
        const { sequence } = await this.transport[direction]();
        await this.store.reached(sequence);
      })(),
    );
  }
  /** Close and capture: refuse new work, drain what is pending, then save. Inputs stay
   * enabled so focus and composition survive a barrier that is cancelled. */
  async prepareClose() {
    this.blocked = true;
    this.snapshots.notify();
    try {
      await this.drainAndSave();
    } catch (error) {
      this.blocked = false;
      this.snapshots.notify();
      throw error;
    }
  }
  cancelClose() {
    this.blocked = false;
    this.snapshots.notify();
  }
  bindText(element: HTMLInputElement | HTMLTextAreaElement, handle: object) {
    const locate = (next: object) => this.snapshots.textLocation(next);
    const attach = (location: ReturnType<typeof locate>) => {
      const optional = location.node.kind === "optional";
      const binding = bindField(element, location.path as Segment[], {
        read: (at) => {
          const value = readPath(this.store.state.value, at);
          const parent = readPath(this.store.state.value, at.slice(0, -1));
          return {
            text: value === undefined && optional && parent != null ? "" : value,
            sequence: this.store.state.sequence,
          };
        },
        send: (batch) => {
          this.store.assertWritable();
          return this.transport.apply(batch);
        },
        reached: (sequence) => this.store.reached(sequence),
        recover: () => this.store.resync(),
        track: (work) => void this.track(work),
        report: (error) => this.report(error),
        undo: (redo) =>
          void (redo ? this.redo() : this.undo()).catch((error) => this.report(error)),
      });
      const drain = async () => {
        await binding.commit();
        if (binding.detached) this.drains.delete(drain);
      };
      this.drains.add(drain);
      const stop = this.snapshots.subscribePath(location.path, () => binding.refresh());
      return {
        path: location.path,
        destroy: () => {
          stop();
          // Keep unresolved text for recovery without holding close barriers or replaying it.
          void binding
            .destroy()
            .catch((error: unknown) => {
              const draft: UnresolvedDraft = {
                path: structuredClone(location.path),
                text: binding.draftText,
              };
              this.drafts.push(draft);
              const code = isDocumentError(error) ? `${error.code}: ` : "";
              const message = error instanceof Error ? error.message : String(error);
              this.report(
                new Error(
                  `Unresolved text draft retained for recovery at ${pathKey(draft.path)}. ${code}${message}`,
                ),
              );
            })
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
