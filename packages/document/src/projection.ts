import type { LoroDoc, LoroEventBatch, LoroMap, LoroMovableList } from "loro-crdt";
import { containerOf, project, projectRows, rowValue as projectRowValue, type Register } from "./operations";
import { ID_KEY } from "./identity";
import { unwrap, type Node, type ObjectNode, type Path } from "./schema";

/** Changed containers from one event batch, keyed by Loro event path segments. */
type Dirty = {
  /** The container's own structure or text changed. */
  whole?: boolean;
  /** Map keys whose values were replaced. */
  keys?: Set<string>;
  children: Map<string | number, Dirty>;
};
const empty = (): Dirty => ({ children: new Map() });

/**
 * Update an immutable snapshot from a Loro event batch, re-reading only the
 * containers the batch touched. Unchanged subtrees keep their identity.
 */
export function patch(root: ObjectNode, engine: LoroDoc, previous: unknown, batch: LoroEventBatch, register?: Register) {
  const dirty = empty();
  for (const event of batch.events) {
    const [head, ...rest] = event.path;
    if (head !== "data") continue;
    let at = dirty;
    for (const part of rest) {
      const segment = part as string | number;
      let next = at.children.get(segment);
      if (!next) at.children.set(segment, (next = empty()));
      at = next;
    }
    if (event.diff.type === "map") for (const key of Object.keys(event.diff.updated)) (at.keys ??= new Set()).add(key);
    else at.whole = true;
  }
  return update(root, engine.getMap("data"), previous, dirty, [], register);
}

function update(node: Node, container: any, previous: any, dirty: Dirty, path: Path, register?: Register): any {
  if (node.kind === "optional")
    return container === undefined ? undefined : update(node.inner, container, previous, dirty, path, register);
  if (previous === undefined || dirty.whole) return full(node, container, previous, dirty, path, register);
  const inner = unwrap(node);
  // Records re-read through project() (reusing unchanged entries), so entry keys and
  // unusable values follow exactly one policy in incremental and full reads.
  if (inner.kind === "record") return project(inner, container, previous, path, register);
  switch (inner.kind) {
    case "object": {
      let next: Record<string, unknown> | undefined;
      const set = (key: string, value: unknown) => {
        if (value === previous[key] && (value !== undefined || !Object.hasOwn(previous, key))) return;
        const copy = (next ??= { ...previous });
        if (value === undefined) delete copy[key];
        else copy[key] = value;
      };
      // Unknown fields and identity changes need a whole re-read to report issues consistently.
      const childNode = (key: string): Node => {
        if (!Object.hasOwn(inner.properties, key)) throw new Error(`Re-read for stored field: ${key}`);
        return inner.properties[key]!;
      };
      const childPath = (key: string) => [...path, key];
      for (const key of dirty.keys ?? []) {
        set(key, project(childNode(key), (container as LoroMap).get(key), previous[key], childPath(key), register));
      }
      for (const [key, child] of dirty.children) {
        if (typeof key !== "string" || dirty.keys?.has(key)) continue;
        set(key, update(childNode(key), (container as LoroMap).get(key), previous[key], child, childPath(key), register));
      }
      if (!next) return previous;
      const value = Object.freeze(next);
      register?.(value, path);
      return value;
    }
    case "list": {
      if (inner.item.kind !== "object") return project(inner, container, previous, path, register);
      // Only row contents changed, so positions are unchanged.
      const list = container as LoroMovableList;
      let next: unknown[] | undefined;
      for (const [index, child] of dirty.children) {
        const row = typeof index === "number" ? (list.get(index) as unknown as LoroMap) : undefined;
        const before = previous[index as number];
        if (!row || containerOf(before) !== row.id || child.keys?.has(ID_KEY))
          return full(node, container, previous, dirty, path, register);
        const value = updateRow(inner.item, row, before, child, path, register);
        if (value !== before) (next ??= [...previous])[index as number] = value;
      }
      return next ? Object.freeze(next) : previous;
    }
    default:
      return project(inner, container, previous, path, register);
  }
}
function updateRow(item: ObjectNode, row: LoroMap, before: any, dirty: Dirty, path: Path, register?: Register) {
  const rowPath = [...path, { id: before.$id }];
  const { $id: id, ...fields } = before;
  const next = update(item, row, fields, dirty, rowPath, register);
  if (next === fields) return before;
  return projectRowValue(next, id, row.id, rowPath, register);
}
/** Re-read a whole container, reusing rows whose contents did not change. */
function full(node: Node, container: any, previous: any, dirty: Dirty, path: Path, register?: Register) {
  const inner = unwrap(node);
  if (inner.kind === "list" && inner.item.kind === "object" && Array.isArray(previous)) {
    const changed = new Set<number>();
    for (const key of dirty.children.keys()) if (typeof key === "number") changed.add(key);
    // Unchanged rows are reused by ID; changed indices refer to post-change positions.
    const rows = projectRows(inner.item, container, previous, path, register, changed);
    return rows;
  }
  return project(node, container, previous, path, register);
}
