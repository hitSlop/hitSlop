import type {
  Batch,
  OwnerIntent,
  OwnerPublication,
  OwnerState,
  OwnerPath as Path,
  OwnerTextRequest,
} from "@hitslop/schema/owner";
import type { Handle, At } from "../handle-types";
import { schemaKey, type Definition, type Node, type ObjectNode, type Value } from "../schema";
import { applyOps, projection } from "./projection";
import { bindText as bindDraft, readPath } from "./text-binding";

import { OwnerError } from "../errors";
import { newID } from "../identity";
import type { AsyncHandle } from "../async-types";
export type { AsyncHandle } from "../async-types";
export interface OwnerTransport {
  readonly id?: string;
  readonly readOnly?: boolean;
  state(): Promise<OwnerState>;
  apply(request: { id: string; session: string; batch: Batch }): Promise<OwnerPublication>;
  text(request: OwnerTextRequest): Promise<OwnerPublication>;
  releaseDraft(id: string): Promise<void>;
  flush(): Promise<void>;
}
export type OwnerScope<N extends ObjectNode> = { readonly fields: Handle<N>; readonly at: At };
type Collector = ((intent: OwnerIntent) => void) & { text(path: Path): string };

function freeze(value: any): void {
  if (!value || typeof value !== "object" || Object.isFrozen(value)) return;
  for (const child of Object.values(value)) freeze(child);
  Object.freeze(value);
}

/** Page-side document state. It contains no CRDT and never writes persistent JSON. */
export class OwnerDocument<N extends ObjectNode> {
  readonly key: string;
  readonly fields: AsyncHandle<Handle<N>>;
  private readonly state;
  private readonly paths = new WeakMap<object, { node: Node; path: Path }>();
  private readonly handlePaths = new WeakMap<object, { node: Node; path: Path }>();
  private readonly drafts = new Set<ReturnType<typeof bindDraft>>();
  private readonly detachedDrafts = new Set<ReturnType<typeof bindDraft>>();
  private readonly listeners = new Set<() => void>();
  private readonly pathListeners = new Map<string, Set<() => void>>();
  private tail: Promise<unknown> = Promise.resolve();
  private collecting = false;
  private queued = 0;
  private unsaved = false;
  private durableSequence = 0;
  private saveError: string | null = null;
  private readonly previews = new Map<string, { path: Path; value: boolean }>();
  private presented: any;
  private flushTask?: Promise<void>;
  private blocked = false;
  private admittedCommit = false;
  private readonly participants = new Set<Promise<void>>();
  readonly id: string;

  private constructor(
    private readonly definition: Definition<N>,
    private readonly transport: OwnerTransport,
    initial: OwnerState,
    private readonly reportError: (error: unknown, kind?: "application" | "operation") => void,
  ) {
    this.id = transport.id ?? initial.session;
    this.key = schemaKey(definition.descriptor);
    this.state = projection(initial, () => transport.state());
    this.presented = initial.value;
    this.register(initial.value, definition.descriptor.root, []);
    freeze(initial.issues);
    this.fields = this.handle(definition.descriptor.root, [], undefined);
  }
  static async open<N extends ObjectNode>(
    definition: Definition<N>,
    transport: OwnerTransport,
    reportError: (error: unknown, kind?: "application" | "operation") => void = console.error,
  ) {
    return new OwnerDocument(definition, transport, await transport.state(), reportError);
  }
  get current(): Value<N> {
    return this.presented;
  }
  get issues(): OwnerState["issues"] {
    return this.state.get().issues;
  }
  get status(): "pending" | "saved" | "save-failed" {
    return this.saveError
      ? "save-failed"
      : this.queued ||
          this.unsaved ||
          this.previews.size ||
          [...this.drafts].some((d) => d.pending())
        ? "pending"
        : "saved";
  }
  get error() {
    return this.saveError;
  }
  subscribe(listener: () => void) {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }
  private report(error: unknown, kind: "application" | "operation" = "operation") {
    // Native save/recovery status owns these incidents.
    if (error instanceof OwnerError && ["save_failed", "owner_invalidated"].includes(error.code))
      return;
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
  private notifyPaths(keys: Set<string> | "all") {
    for (const [key, set] of this.pathListeners) {
      if (keys !== "all") {
        // A touched path refreshes its own bindings and every binding beneath it.
        let hit = false;
        for (const touched of keys) {
          if (key === touched || key.startsWith(touched.slice(0, -1) + ",")) { hit = true; break; }
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

  private register(value: any, node: Node, path: Path) {
    if (value === null || typeof value !== "object" || this.paths.has(value)) return;
    this.paths.set(value, { node, path });
    if (node.kind === "object" && !Array.isArray(value)) {
      for (const [key, child] of Object.entries(node.properties))
        this.register(value[key], child, [...path, key]);
    } else if (node.kind === "list" && Array.isArray(value)) {
      for (const row of value)
        if (row && typeof row === "object" && typeof row.$id === "string") {
          this.register(row, node.item, [...path, { id: row.$id }]);
        }
    }
    // Freeze arbitrary preserved anomaly values as well as declared fields.
    freeze(value);
  }
  async receive(publication: OwnerPublication) {
    const before = this.state.get();
    await this.state.accept(publication);
    const after = this.state.get();
    this.unsaved = this.state.get().sequence > this.durableSequence;
    if (after === before) return;
    this.present();
    freeze(this.issues);
    this.notify();
    const incremental = after.sequence === publication.patch.sequence && before.sequence === publication.patch.previous;
    if (!incremental) return this.notifyPaths("all");
    const touched = new Set<string>();
    for (const op of publication.patch.ops) {
      const path = op.type === "deleteRow" ? [...op.path, { id: op.id }] : op.path;
      if (op.type !== "insertRow" && op.type !== "moveRow") touched.add(JSON.stringify(path));
    }
    if (this.previews.size) return this.notifyPaths("all");
    this.notifyPaths(touched);
  }
  private present() {
    this.presented = this.previews.size
      ? applyOps(
          this.state.get().value,
          [...this.previews.values()].map((p) => ({ type: "set", ...p })),
        )
      : this.state.get().value;
    this.register(this.current, this.definition.descriptor.root, []);
  }
  private enqueue<R>(action: () => Promise<R>): Promise<R> {
    this.queued++;
    this.notify();
    const result = this.tail.then(action);
    this.tail = result.catch(() => {});
    return result.then(
      (value) => {
        this.queued--;
        this.notify();
        return value;
      },
      (error) => {
        this.queued--;
        this.report(error);
        this.notify();
        throw error;
      },
    );
  }
  private submit<R>(intents: OwnerIntent[], result: R, drain = false): Promise<R> {
    if (this.transport.readOnly) return Promise.reject(new Error("Read-only document"));
    if (this.blocked && !this.admittedCommit && !drain)
      return Promise.reject(new Error("Document barrier is active"));
    if (this.collecting) throw new Error("Use tx handles inside change()");
    const batch = structuredClone({ intents }) as Batch;
    const session = this.state.get().session;
    const id = crypto.randomUUID();
    return this.enqueue(async () => {
      if (session !== this.state.get().session)
        throw new Error("session_changed: queued edit was not sent");
      const publication = await this.transport.apply({ id, session, batch });
      this.unsaved = true;
      await this.receive(publication);
      if (session !== this.state.get().session)
        throw new Error("unknown_outcome: owner changed; inspect current state");
      return result;
    });
  }
  at = (<T extends Node>(value: object) => {
    const location = this.paths.get(value);
    if (!location) throw new Error("Value is not a snapshot from this document");
    return this.handle(location.node, location.path, undefined);
  }) as <T extends Node>(value: import("../schema").Snapshot<T>) => AsyncHandle<Handle<T>>;

  change<R>(callback: (tx: OwnerScope<N>) => R, _options?: { message?: string }): Promise<R> {
    if (this.transport.readOnly) return Promise.reject(new Error("Read-only document"));
    if (this.blocked && !this.admittedCommit)
      return Promise.reject(new Error("Document barrier is active"));
    if (this.collecting) throw new Error("Nested change() is not supported");
    const intents: OwnerIntent[] = [];
    let active = true;
    const collect: Collector = Object.assign(
      (intent: OwnerIntent) => {
        if (!active) throw new Error("Transaction handle escaped change()");
        intents.push(structuredClone(intent));
      },
      {
        text: (path: Path) => {
          if (!active) throw new Error("Transaction handle escaped change()");
          let value = readPath(this.state.get().value, path);
          // Only lower text replacement to sequential splices. This local collector
          // does not validate or execute CRDT commands; the core owns atomic validation.
          for (const intent of intents) {
            if (
              intent.type === "insert" &&
              JSON.stringify(path.slice(0, intent.path.length)) === JSON.stringify(intent.path)
            ) {
              const row = path[intent.path.length];
              if (row && typeof row === "object" && row.id === intent.id)
                value = readPath(intent.value, path.slice(intent.path.length + 1));
            }
            if (
              intent.type === "splice" &&
              typeof value === "string" &&
              JSON.stringify(path) === JSON.stringify(intent.path)
            ) {
              value =
                value.slice(0, intent.index) +
                intent.insert +
                value.slice(intent.index + intent.delete);
            }
          }
          if (typeof value !== "string") throw new Error("Cannot replace anomalous text");
          return value;
        },
      },
    );
    this.collecting = true;
    let result: R;
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
      result = callback(tx);
      if (result && typeof (result as any).then === "function") {
        // Observe an async callback's eventual failure without submitting any work.
        Promise.resolve(result).catch((error) => this.report(error));
        throw new Error("change() callback must be synchronous");
      }
    } catch (error) {
      this.report(error);
      return Promise.reject(error);
    } finally {
      active = false;
      this.collecting = false;
    }
    return this.submit(intents, result);
  }

  private handle(node: Node, path: Path, collect: Collector | undefined): any {
    const result = this.makeHandle(node, path, collect);
    if (!collect) this.handlePaths.set(result, { node, path });
    return result;
  }
  private makeHandle(node: Node, path: Path, collect: Collector | undefined): any {
    const send = <R>(intent: OwnerIntent, result: R) => {
      if (collect) {
        collect(intent);
        return result;
      }
      return this.submit([intent], result);
    };
    switch (node.kind) {
      case "object":
        return Object.freeze(
          Object.fromEntries(
            Object.entries(node.properties).map(([key, child]) => [
              key,
              this.handle(child, [...path, key], collect),
            ]),
          ),
        );
      case "boolean": {
        const key = JSON.stringify(path);
        return Object.freeze({
          set: (value: boolean) => {
            if (collect) return send({ type: "set", path, value }, undefined);
            const preview = this.previews.get(key);
            return this.submit([{ type: "set", path, value }], undefined).then(() => {
              if (preview && this.previews.get(key) === preview) {
                this.previews.delete(key);
                this.present();
                this.notify();
                this.notifyPaths("all");
              }
            });
          },
          preview: (value: boolean) => {
            if (this.transport.readOnly) throw new Error("Read-only document");
            if (this.blocked) throw new Error("Document barrier is active");
            if (collect || this.collecting) throw new Error("Previews are not transaction writes");
            if (typeof value !== "boolean") throw new Error("Expected boolean preview");
            this.previews.set(key, { path, value });
            this.present();
            this.notify();
            this.notifyPaths("all");
          },
        });
      }
      case "counter": {
        const increment = (by = 1) => send({ type: "increment", path, by }, undefined);
        return Object.freeze({ increment, decrement: (by = 1) => increment(-by) });
      }
      case "list": {
        if (node.item.kind !== "object") throw new Error("Unsupported descriptor: scalar list");
        return Object.freeze({
          item: (id: string) => this.handle(node.item, [...path, { id }], collect),
          insert: (value: unknown, at?: { before: string } | { after: string }) => {
            const id = newID();
            return send(
              { type: "insert", path, value, id, ...(at ? { at } : {}) },
              Object.freeze({ id }),
            );
          },
          remove: (id: string) => send({ type: "remove", path, id }, undefined),
          move: (id: string, at: { before: string } | { after: string }) =>
            send({ type: "move", path, id, at }, undefined),
        });
      }
      case "text":
        return Object.freeze({
          splice: (index: number, deleteCount: number, insert = "") =>
            send(
              {
                type: "splice",
                path,
                base: this.state.get().version,
                index,
                delete: deleteCount,
                insert,
              },
              undefined,
            ),
          replace: (value: string) => {
            const current = collect ? collect.text(path) : readPath(this.state.get().value, path);
            if (typeof current !== "string") throw new Error("Cannot replace anomalous text");
            return send(
              {
                type: "splice",
                path,
                base: this.state.get().version,
                index: 0,
                delete: current.length,
                insert: value,
              },
              undefined,
            );
          },
        });
      default:
        throw new Error(`Unsupported descriptor: ${node.kind}`);
    }
  }
  /** Native save status is separate from command acceptance. */
  saved(
    status: "pending" | "saved" | "save-failed",
    error?: string | null,
    sequence = this.state.get().sequence,
  ) {
    if (status === "saved") {
      this.durableSequence = Math.max(this.durableSequence, sequence);
      this.unsaved = this.state.get().sequence > this.durableSequence;
      this.saveError = null;
    } else if (status === "save-failed") this.saveError = error ?? "Save failed";
    else this.unsaved = true;
    this.notify();
  }
  async prepareClose() {
    this.blocked = true;
    this.notify();
    this.notifyPaths("all");
    try {
      await this.flush();
    } catch (error) {
      this.blocked = false;
      this.notify();
      this.notifyPaths("all");
      throw error;
    }
  }
  cancelClose() {
    this.blocked = false;
    this.notify();
    this.notifyPaths("all");
  }
  async close() {
    await this.prepareClose();
  }
  async stageSave(
    work: (commit: (callback: () => void | Promise<void>) => Promise<void>) => Promise<void>,
  ) {
    if (this.transport.readOnly) throw new Error("Read-only document");
    if (this.blocked) throw new Error("Document barrier is active");
    const task = work(async (callback) => {
      // A blob admitted before the barrier may submit its reference edit. Scope
      // this permission to that callback's synchronous submission, not its wait.
      this.admittedCommit = true;
      let result: void | Promise<void>;
      try {
        result = callback();
      } finally {
        this.admittedCommit = false;
      }
      await result;
    });
    this.participants.add(task);
    try {
      await task;
    } finally {
      this.participants.delete(task);
    }
    await this.flush();
  }
  bindValue(element: HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement, initial: object) {
    let handle = initial;
    let location: { node: Node; path: Path };
    let request = 0;
    let pending = false;
    const resolve = (next: object) => {
      const found = this.handlePaths.get(next);
      if (!found || found.node.kind !== "boolean")
        throw new Error("Expected this document's boolean handle");
      location = found;
      handle = next;
    };
    const sync = () => {
      const value = readPath(this.current, location.path);
      const disabled = this.transport.readOnly || this.blocked || typeof value !== "boolean";
      if (element.disabled !== disabled) element.disabled = disabled;
      if (!pending) {
        if ((element as HTMLInputElement).type === "checkbox")
          (element as HTMLInputElement).checked = value === true;
        else element.value = String(value);
      }
    };
    const commit = async () => {
      const serial = ++request;
      const value =
        (element as HTMLInputElement).type === "checkbox"
          ? (element as HTMLInputElement).checked
          : element.value === "true";
      pending = true;
      try {
        await (handle as AsyncHandle<import("../handle-types").ScalarHandle<boolean>>).set(value);
      } catch {
        /* submit already reports the rejection centrally. */
      } finally {
        if (serial === request) {
          pending = false;
          sync();
        }
      }
    };
    resolve(initial);
    let stop = this.subscribePath(location!.path, sync);
    element.addEventListener("change", commit);
    sync();
    return {
      update: (next: object) => {
        resolve(next);
        stop();
        stop = this.subscribePath(location.path, sync);
        sync();
      },
      destroy: () => {
        request++;
        stop();
        element.removeEventListener("change", commit);
      },
    };
  }
  bindText(element: HTMLInputElement | HTMLTextAreaElement, handle: object) {
    let binding: ReturnType<typeof bindDraft>;
    let stop: () => void;
    let path: Path;
    const start = (next: object) => {
      const location = this.handlePaths.get(next);
      if (!location || location.node.kind !== "text")
        throw new Error("Expected this document's text handle");
      path = location.path;
      binding = bindDraft(
        element,
        path,
        {
          frame: () => this.state.get(),
          editable: () => !this.transport.readOnly && !this.blocked,
          text: (request) =>
            this.enqueue(async () => {
              const reply = await this.transport.text(request);
              this.unsaved = true;
              await this.receive(reply);
              return reply;
            }),
          release: (id) => this.transport.releaseDraft(id),
        },
        (error) => {
          this.report(error);
          this.notify();
        },
      );
      this.drafts.add(binding);
      stop = this.subscribePath(path, () => binding.refresh());
    };
    const changed = () => this.notify();
    start(handle);
    element.addEventListener("input", changed);
    element.addEventListener("compositionstart", changed);
    element.addEventListener("compositionend", changed);
    return {
      update: (next: object) => {
        const location = this.handlePaths.get(next);
        if (!location || location.node.kind !== "text")
          throw new Error("Expected this document's text handle");
        if (JSON.stringify(location.path) === JSON.stringify(path)) return;
        if (binding.pending()) throw new Error("Flush the text draft before changing its binding");
        stop();
        binding.destroy();
        this.drafts.delete(binding);
        start(next);
      },
      destroy: () => {
        stop();
        element.removeEventListener("input", changed);
        element.removeEventListener("compositionstart", changed);
        element.removeEventListener("compositionend", changed);
        // Keep a pending detached field reachable until the next drain. An authored
        // unmount must not silently erase characters before a close/export barrier.
        if (!binding.pending()) {
          binding.destroy();
          this.drafts.delete(binding);
        } else this.detachedDrafts.add(binding);
      },
    };
  }
  flush(): Promise<void> {
    if (this.collecting) throw new Error("Cannot flush inside change()");
    return (this.flushTask ??= this.drainAndSave().finally(() => {
      this.flushTask = undefined;
    }));
  }
  private async drainAndSave(): Promise<void> {
    await Promise.all(this.participants);
    for (const draft of this.drafts) {
      await draft.drain();
      if (this.detachedDrafts.delete(draft)) {
        draft.destroy();
        this.drafts.delete(draft);
      }
    }
    const previews = [...this.previews.entries()];
    if (previews.length) {
      await this.submit(
        previews.map(([, p]) => ({ type: "set" as const, ...p })),
        undefined,
        true,
      );
      for (const [key, preview] of previews)
        if (this.previews.get(key) === preview) this.previews.delete(key);
      this.present();
      this.notify();
      this.notifyPaths("all");
    }
    return this.enqueue(async () => {
      try {
        await this.transport.flush();
        this.durableSequence = this.state.get().sequence;
        this.unsaved = false;
        this.saveError = null;
      } catch (error) {
        this.saveError = String(error);
        throw error;
      }
    });
  }
}
