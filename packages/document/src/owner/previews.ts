import type { OwnerIntent, OwnerPath as Path } from "@hitslop/schema/owner";
import type { Segment } from "../schema";
import { applyOps, readPath } from "./store";

/** `value` undefined shows an optional field as cleared. */
export type Preview = { readonly path: Path; readonly value: unknown };
/** The write that commits a preview. */
export const commitIntent = (preview: Preview): OwnerIntent =>
  preview.value === undefined
    ? { type: "clear", path: preview.path }
    : { type: "set", path: preview.path, value: preview.value };

/** Local values shown over the snapshot: drags and drawing, live scalar writes awaiting
 * acceptance, and `value` assignments awaiting commit. They write no history; a write
 * covering them, or a barrier, commits or settles them. */
export class Previews {
  private readonly entries = new Map<string, Preview>();

  get size() {
    return this.entries.size;
  }
  get(key: string) {
    return this.entries.get(key);
  }
  all(): [string, Preview][] {
    return [...this.entries];
  }
  set(path: Path, value: unknown): string {
    const key = JSON.stringify(path);
    this.entries.set(key, { path, value: structuredClone(value) });
    return key;
  }
  /** The snapshot with previews applied. A preview whose container is gone (a removed
   * row, an unset optional object or entry) is dropped, never invented. */
  overlay(value: unknown): unknown {
    for (const [key, preview] of this.entries)
      if (readPath(value, preview.path.slice(0, -1) as Segment[]) == null) this.entries.delete(key);
    if (!this.entries.size) return value;
    const set = (preview: Preview) =>
      (preview.value === undefined
        ? { type: "remove", path: preview.path }
        : { type: "set", path: preview.path, value: preview.value }) as any;
    // One pass copies each container once; if one preview no longer fits, apply them
    // singly so only that one is dropped.
    try {
      return applyOps(value, [...this.entries.values()].map(set));
    } catch {
      for (const [key, preview] of this.entries) {
        try {
          value = applyOps(value, [set(preview)]);
        } catch {
          this.entries.delete(key);
        }
      }
      return value;
    }
  }
  /** The previews a batch overwrites: at or beneath a written path. Structural edits of a
   * scalar list shift its indexes, so they cover every element preview in that list. */
  coveredBy(intents: readonly OwnerIntent[]): Map<string, Preview> {
    const covered = new Map<string, Preview>();
    const under = (scope: readonly unknown[], path: readonly unknown[]) =>
      scope.length <= path.length && scope.every((segment, i) => JSON.stringify(segment) === JSON.stringify(path[i]));
    for (const [key, preview] of this.entries)
      for (const intent of intents) {
        if (intent.type === "move" || intent.type === "increment") continue;
        if (intent.type === "insert" && intent.id !== undefined) continue;
        const scope = intent.type === "remove" && intent.id !== undefined ? [...intent.path, { id: intent.id }] : intent.path;
        if (under(scope, preview.path)) { covered.set(key, preview); break; }
      }
    return covered;
  }
  /** Drops covered previews unless a newer preview replaced one meanwhile; returns the
   * keys that changed. */
  settle(covered: ReadonlyMap<string, Preview>): Set<string> {
    const settled = new Set<string>();
    for (const [key, preview] of covered)
      if (this.entries.get(key) === preview) { this.entries.delete(key); settled.add(key); }
    return settled;
  }
}
