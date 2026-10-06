import type { Segment } from "@hitslop/schema/core";

// Snapshot arrays are immutable, so one `$id → index` map per array serves every reader.
export const rowIndexes = new WeakMap<readonly any[], Map<string, number>>();
export function indexOf(rows: readonly any[], id: string): number {
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
    if (typeof part === "string")
      value =
        value && typeof value === "object" && Object.hasOwn(value, part) ? value[part] : undefined;
    else if (!Array.isArray(value)) return undefined;
    else if ("index" in part) value = value[part.index];
    else value = value[indexOf(value, part.id)];
  }
  return value;
}
/** Canonical keys for field, row and scalar-list paths. */
export const pathKey = (path: readonly Segment[]) => JSON.stringify(path);
/** Whether a keyed path is the scope itself or one of its descendants. */
export const atOrBeneath = (key: string, scope: string) =>
  scope === "[]" || key === scope || key.startsWith(scope.slice(0, -1) + ",");
export const keyIn = (container: any, segment: Segment) =>
  typeof segment === "string"
    ? segment
    : "index" in segment
      ? segment.index
      : indexOf(container, segment.id);
