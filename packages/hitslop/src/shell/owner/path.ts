import type { Segment } from "../../schema/core";
import type { Node } from "../../sdk/schema";
import { unwrap } from "../../sdk/internal";

// Snapshot arrays are immutable, so one `$id → index` map per array serves every reader.
export const rowIndexes = new WeakMap<readonly unknown[], Map<string, number>>();
export function indexOf(rows: readonly unknown[], id: string): number {
  let index = rowIndexes.get(rows);
  if (!index) {
    index = new Map();
    rows.forEach((row, i) => {
      if (row && typeof row === "object" && "$id" in row && typeof row.$id === "string" && !index!.has(row.$id))
        index!.set(row.$id, i);
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
/** Where each object of a snapshot sits, for `at(value)`: the node and path of the root,
 * rows, nested objects and record entries. Stops at objects already mapped, so the cost
 * follows the objects a publication created. */
export type Locations = WeakMap<object, { node: Node; path: Segment[] }>;
export function mapPaths(paths: Locations, value: any, node: Node, path: Segment[]) {
  if (value === null || typeof value !== "object" || paths.has(value)) return;
  paths.set(value, { node, path });
  const inner = unwrap(node);
  if (inner.kind === "object" && !Array.isArray(value)) {
    for (const [key, child] of Object.entries(inner.properties)) mapPaths(paths, value[key], child, [...path, key]);
  } else if (inner.kind === "list" && Array.isArray(value)) {
    // Rows a publication kept are already mapped: skip them before building a path.
    for (const row of value)
      if (row && typeof row === "object" && typeof row.$id === "string" && !paths.has(row))
        mapPaths(paths, row, inner.item, [...path, { id: row.$id }]);
  } else if (inner.kind === "record" && !Array.isArray(value)) {
    // Object entries resolve with `doc.at(entry)`, addressed by their key.
    for (const [key, entry] of Object.entries(value))
      if (!paths.has(entry as object)) mapPaths(paths, entry, inner.value, [...path, key]);
  }
}
