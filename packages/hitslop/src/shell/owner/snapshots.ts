import type { Node, ObjectNode } from "../../sdk/schema";
import { unwrap } from "../../sdk/internal";
import type { OwnerPath as Path } from "../../schema/core";
import type { Collector } from "./handles";
import type { Changes } from "./store";
import { atOrBeneath, mapPaths, pathKey, type Locations } from "./path";

/** Snapshot identity, reusable handles and subscriptions. Write ordering lives in OwnerDocument. */
export class SnapshotBindings {
  private readonly paths: Locations = new WeakMap();
  private readonly handles = new Map<string, any>();
  private readonly handlePaths = new WeakMap<object, { node: Node; path: Path }>();
  private readonly listeners = new Set<() => void>();
  private readonly pathListeners = new Map<string, Set<() => void>>();
  private observer: () => void = () => {};

  constructor(
    private readonly descriptor: ObjectNode,
    private readonly makeHandle: (node: Node, path: Path, collect: Collector | undefined) => any,
    private readonly report: (error: unknown) => void,
  ) {}

  map(value: unknown) { mapPaths(this.paths, value, this.descriptor, []); }
  locate(value: object) {
    const location = this.paths.get(value);
    if (!location) throw new Error("Value is not a snapshot from this document");
    return location;
  }
  handle(node: Node, path: Path, collect: Collector | undefined): any {
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
  textLocation(handle: object) {
    const location = this.handlePaths.get(handle);
    if (!location || unwrap(location.node).kind !== "text") throw new Error("Expected this document's text handle");
    return location;
  }
  invalidate(changes: Changes) {
    // Caller-held handles remain valid paths; deleted rows leave the strong cache.
    if (changes === "all") this.handles.clear();
    else for (const path of changes.removed ?? []) {
      const key = pathKey(path);
      for (const cached of this.handles.keys())
        if (atOrBeneath(cached, key)) this.handles.delete(cached);
    }
  }
  observed() { this.observer(); }
  observe(read: () => void) {
    this.observer = read;
    return () => { if (this.observer === read) this.observer = () => {}; };
  }
  subscribe(listener: () => void) {
    this.listeners.add(listener);
    return () => { this.listeners.delete(listener); };
  }
  notify() {
    for (const listener of this.listeners) {
      try { listener(); } catch (error) { this.report(error); }
    }
  }
  /** Bindings refresh only when a publication touches their path, or on a full refresh. */
  subscribePath(path: Path, listener: () => void) {
    const key = pathKey(path);
    let set = this.pathListeners.get(key);
    if (!set) this.pathListeners.set(key, (set = new Set()));
    set.add(listener);
    return () => {
      set!.delete(listener);
      if (!set!.size) this.pathListeners.delete(key);
    };
  }
  notifyPaths(changes: Changes) {
    const touched = changes === "all" ? undefined : [...changes.paths];
    for (const [key, set] of this.pathListeners) {
      if (touched && !touched.some(path => atOrBeneath(key, path))) continue;
      for (const listener of set) {
        try { listener(); } catch (error) { this.report(error); }
      }
    }
  }
}
