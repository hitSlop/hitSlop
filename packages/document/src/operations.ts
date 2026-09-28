import type { InsertResult } from "./handles";
import { OperationRejectedError } from "./errors";
import {
  LoroCounter,
  LoroDoc,
  LoroMap,
  LoroMovableList,
  LoroText,
  LoroTree,
  type LoroTreeNode,
  type ContainerID,
  type TreeID,
} from "loro-crdt";
import { ID_KEY, effectiveIDs, isID, newID } from "./identity";
import {
  checkRecordKey,
  isScalar,
  unwrap,
  validate,
  validateMark,
  type Node,
  type ObjectNode,
  type Path,
  type Field,
  type Scalar,
  type OptionalNode,
  type Text,
  type ListNode,
  type Input,
  type MarkValue,
  type ValueNode,
} from "./schema";
/** Object rows and tree nodes are placed relative to a sibling; tree nodes may also name a parent. */
export type Destination =
  | { before: string; after?: never; parent?: never }
  | { after: string; before?: never; parent?: never }
  | { parent: string | null; before?: never; after?: never };
export type Operation =
  | { type: "set"; path: Path; value: string | boolean | number }
  | { type: "clear"; path: Path }
  | { type: "assign"; path: Path; value: unknown }
  | { type: "text.replace"; path: Path; value: string }
  | { type: "text.splice"; path: Path; index: number; delete: number; insert: string }
  | { type: "text.mark"; path: Path; start: number; end: number; key: string; value: MarkValue }
  | { type: "text.unmark"; path: Path; start: number; end: number; key: string }
  | {
      type: "insert";
      path: Path;
      value: unknown;
      destination?: Destination;
      index?: number;
      /** Import-only: keep a supplied row or tree-node ID. */
      id?: string;
    }
  | { type: "remove"; path: Path; id?: string; index?: number; count?: number }
  | { type: "move"; path: Path; id?: string; destination?: Destination; from?: number; to?: number }
  | { type: "increment"; path: Path; value: number };
/**
 * Executes operations for handles, CLI batches, previews and JSON import. Not an
 * author API: apps write through `fields` and `at()` handles.
 */
export class Commands {
  constructor(private dispatch: (operation: Operation) => InsertResult | void) {}
  execute(operation: Operation) {
    return this.dispatch(operation);
  }
}

type Location = {
  node: Node;
  /** The container holding this value; absent for the document root. */
  parent?: LoroMap | LoroMovableList;
  key: string | number;
  value: any;
  /** Record entries may be created or deleted like optional values. */
  entry: boolean;
};
const reject = (message: string): never => {
  throw new OperationRejectedError(message);
};
const isInt = (value: unknown): value is number => Number.isSafeInteger(value);
function resolve(engine: LoroDoc, root: ObjectNode, path: Path): Location {
  if (!Array.isArray(path)) reject("Invalid operation path");
  let location: Location = { node: root, key: "", value: engine.getMap("data"), entry: false };
  for (const [position, part] of path.entries()) {
    const collection = path.slice(0, position);
    const { node, value } = location;
    if (value === undefined) reject(`Absent value at ${JSON.stringify(path)}; set it first`);
    const inner = unwrap(node);
    usable(inner, value, path);
    if (typeof part === "string" && inner.kind === "object" && Object.hasOwn(inner.properties, part))
      location = {
        node: inner.properties[part]!,
        parent: value,
        key: part,
        value: value.get(part),
        entry: false,
      };
    else if (part && typeof part === "object" && "id" in part && typeof part.id === "string") {
      if (inner.kind === "list" && inner.item.kind === "object") {
        const index = rowIndex(value, part.id, collection);
        location = { node: inner.item, parent: value, key: index, value: value.get(index), entry: false };
      } else if (inner.kind === "tree") {
        const data = treeNode(value, part.id, collection).data;
        location = { node: inner.item, key: part.id, value: data, entry: false };
      } else reject("Unknown schema path");
    } else if (part && typeof part === "object" && "key" in part && inner.kind === "record") {
      checkRecordKey(part.key);
      location = { node: inner.value, parent: value, key: part.key, value: value.get(part.key), entry: true };
    } else if (
      part &&
      typeof part === "object" &&
      "index" in part &&
      inner.kind === "list" &&
      inner.item.kind !== "object"
    ) {
      if (!isInt(part.index) || part.index < 0 || part.index >= value.length)
        reject(`List index out of range at ${JSON.stringify(path)}`);
      location = { node: inner.item, parent: value, key: part.index, value: value.get(part.index), entry: false };
    } else reject("Unknown schema path");
  }
  return location;
}
const storedID = (container: any): unknown =>
  container?.kind?.() === "Map" ? container.get(ID_KEY) : undefined;
/** Effective row IDs by list position; projection and resolution share this rule. */
export function rowIdentities(list: LoroMovableList) {
  const items = list.toArray() as any[];
  const positions: number[] = [];
  items.forEach((row, index) => row?.kind?.() === "Map" && positions.push(index));
  return effectiveIDs(positions.map((index) => ({ stored: items[index].get(ID_KEY), internal: items[index].id }))).map(
    (identity, i) => ({ ...identity, index: positions[i]!, row: items[positions[i]!] as LoroMap }),
  );
}
/** Effective tree node IDs in depth-first pre-order, matching the projected tree. */
export function treeIdentities(tree: LoroTree) {
  const nodes: LoroTreeNode[] = [];
  const visit = (list: LoroTreeNode[] | undefined) => {
    for (const node of list ?? []) {
      nodes.push(node);
      visit(node.children());
    }
  };
  visit(tree.roots());
  return effectiveIDs(nodes.map((node) => ({ stored: storedID(node.data), internal: node.id }))).map(
    (identity, i) => ({ ...identity, node: nodes[i]! }),
  );
}
/**
 * Maps a collection path and effective ID to the internal container (or tree node) ID
 * that projection assigned it. Hints are verified against the engine being edited,
 * so a stale snapshot falls back to the full scan instead of addressing another row.
 */
export type Lookup = (collection: Path, id: string) => string | undefined;
let active: { engine: LoroDoc; lookup?: Lookup } | undefined;
function rowIndex(list: LoroMovableList, id: string, collection: Path) {
  const hint = isID(id) && active ? active.lookup?.(collection, id) : undefined;
  if (hint && active) {
    let found: (string | number)[] | undefined;
    try {
      found = active.engine.getPathToContainer(hint as ContainerID);
    } catch {}
    const parent = active.engine.getPathToContainer(list.id);
    const index = found?.at(-1);
    if (
      found && parent && typeof index === "number" &&
      found.length === parent.length + 1 && parent.every((part, i) => found![i] === part) &&
      (list.get(index) as any)?.id === hint
    )
      return index;
  }
  if (isID(id)) for (const row of rowIdentities(list)) if (row.id === id) return row.index;
  return reject(`Unknown row ID: ${id} at ${JSON.stringify(collection)}`);
}
function treeNode(tree: LoroTree, id: string, collection: Path): LoroTreeNode {
  const hint = isID(id) && active ? active.lookup?.(collection, id) : undefined;
  if (hint)
    try {
      if (tree.has(hint as TreeID) && !tree.isNodeDeleted(hint as TreeID)) return tree.getNodeByID(hint as TreeID)!;
    } catch {}
  if (isID(id)) for (const entry of treeIdentities(tree)) if (entry.id === id) return entry.node;
  return reject(`Unknown tree node ID: ${id} at ${JSON.stringify(collection)}`);
}
/** Structurally unusable stored values stay untouched; edits beneath them are refused. */
function usable(node: ValueNode, value: any, path: Path) {
  if (isScalar(node) || value === undefined) return;
  const kind = kinds[node.kind as keyof typeof kinds];
  if (value?.kind?.() !== kind)
    reject(`Stored value at ${JSON.stringify(path)} is unusable; see document issues`);
}
/** Present value of a composite, rejecting absent optional values and record entries. */
function present(location: Location, path: Path) {
  if (location.value === undefined) reject(`Absent value at ${JSON.stringify(path)}; set it first`);
  const node = unwrap(location.node);
  usable(node, location.value, path);
  return node;
}
function range(start: unknown, end: unknown, length: number) {
  if (!isInt(start) || !isInt(end) || start < 0 || end > length || start >= end)
    reject("Expected a non-empty text range inside the text");
}
export function applyOperation(
  engine: LoroDoc,
  root: ObjectNode,
  op: Operation,
  lookup?: Lookup,
): InsertResult | void {
  active = { engine, lookup };
  try {
    return operate(engine, root, op);
  } finally {
    active = undefined;
  }
}
function operate(engine: LoroDoc, root: ObjectNode, op: Operation): InsertResult | void {
  if (!op || typeof op !== "object") reject("Invalid operation");
  const location = resolve(engine, root, op.path);
  const { node, parent, key, value } = location;
  switch (op.type) {
    case "set": {
      const scalar = unwrap(node);
      if (!isScalar(scalar))
        reject("set requires a scalar; use text.replace for text and assign for composites");
      validate(scalar, op.value);
      if (!parent) return reject("Unknown schema path");
      if (parent instanceof LoroMovableList) parent.set(key as number, op.value);
      else if (parent instanceof LoroMap) parent.set(key as string, op.value);
      else reject("Tree nodes are not scalars");
      return;
    }
    case "clear":
      if (node.kind !== "optional" && !location.entry)
        reject("Only optional fields and record entries can be cleared");
      if (!(parent instanceof LoroMap)) return reject("Only optional fields and record entries can be cleared");
      parent.delete(key as string);
      return;
    case "assign": {
      const target = unwrap(node);
      if (!parent) return reject("assign requires a field path");
      if (op.value === undefined) {
        if (node.kind !== "optional" && !location.entry) reject("Only optional fields can be cleared");
        (parent as LoroMap).delete(key as string);
        return;
      }
      validate(target, op.value);
      // An explicit assignment replaces an unusable stored value; it is not a read repair.
      const stored = isScalar(target) || value?.kind?.() === kinds[target.kind as keyof typeof kinds] ? value : undefined;
      assertAssignable(target, op.value, stored);
      if (parent instanceof LoroMovableList) parent.set(key as number, op.value as any);
      else write(parent as LoroMap, key as string, target, op.value, true);
      return;
    }
    case "text.replace":
    case "text.splice":
    case "text.mark":
    case "text.unmark": {
      const text = present(location, op.path);
      if (text.kind !== "text" && text.kind !== "richtext") reject(`${op.type} requires text`);
      const container = value as LoroText;
      if (op.type === "text.replace") {
        validate(text, op.value);
        container.update(op.value);
      } else if (op.type === "text.splice") {
        const length = container.length;
        if (!isInt(op.index) || !isInt(op.delete) || op.index < 0 || op.delete < 0 || op.index + op.delete > length)
          reject("Text splice is outside the text");
        if (typeof op.insert !== "string") reject("Text splice inserts a string");
        container.splice(op.index, op.delete, op.insert);
      } else {
        if (text.kind !== "richtext") return reject(`${op.type} requires rich text`);
        validateMark(text, op.key, op.type === "text.mark" ? op.value : null);
        range(op.start, op.end, container.length);
        if (op.type === "text.mark") container.mark({ start: op.start, end: op.end }, op.key, op.value);
        else container.unmark({ start: op.start, end: op.end }, op.key);
      }
      return;
    }
    case "increment": {
      const counter = present(location, op.path);
      if (counter.kind !== "counter") reject("increment requires a counter");
      validate(counter, op.value);
      const total = (value as LoroCounter).value;
      if (!Number.isFinite(total) || !Number.isFinite(total + op.value))
        reject("Counter would leave the finite number range");
      (value as LoroCounter).increment(op.value);
      return;
    }
    case "insert":
    case "remove":
    case "move": {
      const target = present(location, op.path);
      if (target.kind === "list" && target.item.kind === "object") return rows(value, target.item, op);
      if (target.kind === "list") return values(value, target.item as Scalar, op);
      if (target.kind === "tree") return tree(value, target.item, op);
      return reject("Operation requires a list or tree field");
    }
    default:
      reject("Unknown operation");
  }
}
/** Preflight the entire assignment before touching any live containers. */
function assertAssignable(node: ValueNode, input: any, existing: any): void {
  if (existing === undefined) return;
  if ((node.kind === "list" && node.item.kind === "object") || node.kind === "tree")
    reject("assign would recreate row identity; use insert, remove and move");
  if (node.kind === "object") {
    for (const [key, child] of Object.entries(node.properties))
      if (input[key] !== undefined) assertAssignable(unwrap(child), input[key], existing.get(key));
  } else if (node.kind === "record") {
    for (const [key, entry] of Object.entries(input))
      assertAssignable(node.value, entry, existing.get(key));
  }
}
type Structural = Extract<Operation, { type: "insert" | "remove" | "move" }>;
function requestedID(op: Structural, taken: (id: string) => boolean) {
  if (op.type !== "insert" || op.id === undefined) return newID();
  if (!isID(op.id) || taken(op.id)) reject(`Row ID ${JSON.stringify(op.id)} is invalid or already used`);
  return op.id!;
}
function insertRow(list: LoroMovableList, index: number, item: ObjectNode, value: any, id = newID()) {
  const row = list.insertContainer(index, new LoroMap());
  row.set(ID_KEY, id);
  fill(row, item, value);
  return id;
}
function rows(list: LoroMovableList, item: ObjectNode, op: Structural): InsertResult | void {
  const dest = (destination: unknown) => {
    const d = destination as Destination | undefined;
    if (
      !d ||
      typeof d !== "object" ||
      "parent" in d ||
      (typeof d.before === "string") === (typeof d.after === "string")
    )
      reject("Move requires exactly one before/after ID");
    return rowIndex(list, (d!.before ?? d!.after)!, op.path) + (d!.after !== undefined ? 1 : 0);
  };
  if (op.type === "insert") {
    validate(item, op.value);
    const id = requestedID(op, (id) => rowIdentities(list).some((row) => row.id === id));
    const index = op.destination === undefined ? list.length : dest(op.destination);
    return Object.freeze({ id: insertRow(list, index, item, op.value, id) });
  }
  if (typeof op.id !== "string") reject("Row operations require an ID");
  const from = rowIndex(list, op.id!, op.path);
  if (op.type === "remove") return void list.delete(from, 1);
  if (op.type !== "move") return;
  const to = dest(op.destination);
  if (to === from || to === from + 1) return;
  list.move(from, to - (from < to ? 1 : 0));
}
function values(list: LoroMovableList, item: Scalar, op: Structural): void {
  const length = list.length;
  if (op.type === "insert") {
    validate(item, op.value);
    const index = op.index ?? length;
    if (!isInt(index) || index < 0 || index > length) reject("List index out of range");
    list.insert(index, op.value as any);
  } else if (op.type === "remove") {
    const count = op.count ?? 1;
    if (!isInt(op.index) || !isInt(count) || op.index < 0 || count < 1 || op.index + count > length)
      reject("List range out of range");
    list.delete(op.index!, count);
  } else if (op.type === "move") {
    if (!isInt(op.from) || !isInt(op.to) || op.from < 0 || op.to < 0 || op.from >= length || op.to >= length)
      reject("List index out of range");
    if (op.from !== op.to) list.move(op.from!, op.to!);
  }
}
function tree(tree: LoroTree, item: ObjectNode, op: Structural): InsertResult | void {
  const place = (destination: unknown): { parent?: TreeID; index: number } => {
    const d = destination as Destination | undefined;
    if (d === undefined) return { index: tree.roots().length };
    if (!d || typeof d !== "object") return reject("Invalid tree destination");
    const given = ["parent", "before", "after"].filter((k) => k in d);
    if (given.length !== 1) reject("Tree destination names exactly one of parent, before or after");
    if ("parent" in d) {
      if (d.parent === null) return { index: tree.roots().length };
      const parent = treeNode(tree, d.parent as string, op.path);
      return { parent: parent.id, index: parent.children()?.length ?? 0 };
    }
    const sibling = treeNode(tree, (d.before ?? d.after)!, op.path);
    return { parent: sibling.parent()?.id, index: sibling.index()! + (d.after !== undefined ? 1 : 0) };
  };
  if (op.type === "insert") {
    validate({ kind: "tree", item }, [op.value]);
    const id = requestedID(op, (id) => treeIdentities(tree).some((entry) => entry.id === id));
    const { parent, index } = place(op.destination);
    return Object.freeze({ id: createTreeNode(tree, item, op.value as any, parent, index, id) });
  }
  if (typeof op.id !== "string") reject("Tree operations require a node ID");
  const node = treeNode(tree, op.id!, op.path);
  if (op.type === "remove") return void tree.delete(node.id);
  if (op.type !== "move") return;
  const d = op.destination as Destination | undefined;
  try {
    if (d && typeof d === "object" && typeof d.before === "string" && !("after" in d) && !("parent" in d))
      node.moveBefore(treeNode(tree, d.before, op.path));
    else if (d && typeof d === "object" && typeof d.after === "string" && !("before" in d) && !("parent" in d))
      node.moveAfter(treeNode(tree, d.after, op.path));
    else {
      const { parent } = place(d);
      const siblings = parent === undefined ? tree.roots() : tree.getNodeByID(parent)!.children() ?? [];
      tree.move(node.id, parent, siblings.filter((s) => s.id !== node.id).length);
    }
  } catch (error) {
    if (error instanceof OperationRejectedError) throw error;
    reject(`Tree move rejected: ${String(error)}`);
  }
}
function createTreeNode(
  tree: LoroTree,
  item: ObjectNode,
  value: Record<string, unknown>,
  parent: TreeID | undefined,
  index: number,
  id = newID(),
): string {
  const { children, ...fields } = value;
  const node = tree.createNode(parent, index);
  node.data.set(ID_KEY, id);
  fill(node.data, item, fields);
  (children as Record<string, unknown>[] | undefined)?.forEach((child, i) =>
    createTreeNode(tree, item, child, node.id, i),
  );
  return id;
}
const kinds = {
  text: "Text",
  richtext: "Text",
  counter: "Counter",
  object: "Map",
  record: "Map",
  list: "MovableList",
  tree: "Tree",
} as const;
const constructors = {
  Text: LoroText,
  Counter: LoroCounter,
  Map: LoroMap,
  MovableList: LoroMovableList,
  Tree: LoroTree,
} as const;
const mergeable = {
  Text: "ensureMergeableText",
  Counter: "ensureMergeableCounter",
  Map: "ensureMergeableMap",
  MovableList: "ensureMergeableMovableList",
  Tree: "ensureMergeableTree",
} as const;
/**
 * Replace the value at a map key. Absent composite children are created with
 * deterministic mergeable IDs when `lazy`, so peers that create the same
 * optional field or record entry concurrently merge instead of losing one side.
 */
function write(map: LoroMap, key: string, node: ValueNode, value: any, lazy: boolean): void {
  if (isScalar(node)) return void map.set(key, value);
  const kind = kinds[node.kind as keyof typeof kinds];
  const existing = map.get(key) as any;
  const container =
    existing?.kind?.() === kind
      ? existing
      : lazy && existing === undefined
        ? (map as any)[mergeable[kind]](key)
        : map.setContainer(key, new (constructors[kind] as any)());
  replace(container, node, value, lazy);
}
function replace(container: any, node: ValueNode, value: any, lazy: boolean): void {
  switch (node.kind) {
    case "text":
    case "richtext":
      if (container.toString() !== value) container.update(value);
      return;
    case "counter":
      if (value !== container.value) container.increment(value - container.value);
      return;
    case "object":
      for (const [key, child] of Object.entries(node.properties)) {
        if (value[key] === undefined) {
          if (container.get(key) !== undefined) container.delete(key);
        } else write(container, key, unwrap(child), value[key], lazy);
      }
      return;
    case "record":
      for (const key of container.keys()) if (!Object.hasOwn(value, key)) container.delete(key);
      for (const [key, entry] of Object.entries(value)) write(container, key, node.value, entry, true);
      return;
    case "list": {
      const list = container as LoroMovableList;
      if (node.item.kind === "object") {
        if (list.length) list.delete(0, list.length);
        for (const row of value) insertRow(list, list.length, node.item, row);
        return;
      }
      // Keep unchanged positions so concurrent edits elsewhere in the list survive.
      const shared = Math.min(list.length, value.length);
      for (let i = 0; i < shared; i++) if (list.get(i) !== value[i]) list.set(i, value[i]);
      if (list.length > value.length) list.delete(value.length, list.length - value.length);
      for (let i = shared; i < value.length; i++) list.insert(i, value[i]);
      return;
    }
    case "tree": {
      const tree = container as LoroTree;
      for (const root of tree.roots()) tree.delete(root.id);
      (value as any[]).forEach((node_, i) => createTreeNode(tree, node.item, node_, undefined, i));
    }
  }
}
/** Populate a new map; fixed fields use ordinary child containers. */
export function fill(map: LoroMap, node: ObjectNode, input: any): void {
  for (const [key, child] of Object.entries(node.properties))
    if (child.kind !== "optional" || input[key] !== undefined)
      write(map, key, unwrap(child), input[key], child.kind === "optional");
}
export type Register = (value: object, path: Path) => void;
/**
 * A semantic anomaly in otherwise decodable state, usually from merging peers.
 * `invalid` values are unusable and read as a documented fallback; `constraint`
 * values are preserved as stored. Projection never repairs stored bytes.
 */
export type Issue = {
  readonly path: Path;
  readonly kind: "invalid" | "constraint" | "unknown-field" | "identity";
  readonly detail: string;
};
const INVALID = Symbol("invalid");
/** Projected row/tree objects remember their container for reuse; never exposed as identity. */
const containers = new WeakMap<object, string>();
const report = (issues: Issue[] | undefined, path: Path, kind: Issue["kind"], detail: string) =>
  void issues?.push(Object.freeze({ path: Object.freeze([...path]) as Path, kind, detail }));
/** Deterministic value read in place of an unusable stored value. */
export function fallback(node: Node): any {
  switch (node.kind) {
    case "optional":
      return undefined;
    case "string":
    case "text":
      return "";
    case "number":
    case "integer":
      return node.min !== undefined && node.min > 0 ? node.min : node.max !== undefined && node.max < 0 ? node.max : 0;
    case "boolean":
      return false;
    case "enum":
      return node.values[0];
    case "counter":
      return 0;
    case "richtext":
      return Object.freeze({ text: "", delta: Object.freeze([]) });
    case "list":
    case "tree":
      return Object.freeze([]);
    case "record":
      return Object.freeze({});
    case "object": {
      const value: Record<string, unknown> = {};
      for (const [key, child] of Object.entries(node.properties))
        if (child.kind !== "optional") value[key] = fallback(child);
      return Object.freeze(value);
    }
  }
}
function scalarIssue(node: Scalar, value: unknown): [Issue["kind"], string] | undefined {
  if (value === undefined) return ["invalid", "Missing value"];
  switch (node.kind) {
    case "string":
      if (typeof value !== "string") return ["invalid", "Expected string"];
      if (node.maxLength !== undefined && value.length > node.maxLength)
        return ["constraint", `Expected at most ${node.maxLength} characters`];
      return;
    case "number":
    case "integer":
      if (typeof value !== "number" || !Number.isFinite(value)) return ["invalid", "Expected finite number"];
      if (node.kind === "integer" && !Number.isInteger(value)) return ["constraint", "Expected integer"];
      if ((node.min !== undefined && value < node.min) || (node.max !== undefined && value > node.max))
        return ["constraint", `Expected a number from ${node.min ?? "-∞"} to ${node.max ?? "∞"}`];
      return;
    case "boolean":
      return typeof value === "boolean" ? undefined : ["invalid", "Expected boolean"];
    case "enum":
      return typeof value === "string" && node.values.includes(value) ? undefined : ["invalid", "Unknown enum value"];
  }
}
/**
 * Read stored state without throwing on semantic anomalies. With `issues`, each
 * anomaly is reported once; unusable values read as `fallback(node)` (a
 * non-finite counter reads as null), and constraint violations keep their value.
 */
export function project(
  node: Node,
  value: any,
  previous?: any,
  path: Path = [],
  register?: Register,
  issues?: Issue[],
): any {
  const result = read(node, value, previous, path, register, issues);
  return result === INVALID ? fallback(node) : result;
}
function read(node: Node, value: any, previous: any, path: Path, register?: Register, issues?: Issue[]): any {
  if (node.kind === "optional") {
    if (value === undefined) return undefined;
    const inner = read(node.inner, value, previous, path, register, issues);
    return inner === INVALID ? undefined : inner;
  }
  if (isScalar(node)) {
    const problem = value !== null && typeof value === "object" ? (["invalid", "Expected scalar value"] as const) : scalarIssue(node, value);
    if (!problem) return value;
    report(issues, path, problem[0], problem[1]);
    return problem[0] === "invalid" ? INVALID : value;
  }
  const kind = kinds[node.kind as keyof typeof kinds];
  if (value?.kind?.() !== kind) {
    report(issues, path, "invalid", value === undefined ? "Missing value" : `Expected Loro${kind}`);
    return INVALID;
  }
  switch (node.kind) {
    case "text":
      return value.toString();
    case "counter": {
      const n = value.value;
      if (Number.isFinite(n)) return n;
      report(issues, path, "invalid", "Expected finite number");
      return null;
    }
    case "richtext": {
      const text = value.toString();
      const delta = value.toDelta();
      if (previous?.text === text && JSON.stringify(previous.delta) === JSON.stringify(delta)) return previous;
      return deepFreeze({ text, delta });
    }
    case "list": {
      if (node.item.kind === "object") return projectRows(node.item, value, previous, path, register, undefined, issues);
      const item = node.item;
      return share(
        previous,
        value.toArray().map((v: unknown, index: number) => project(item, v, undefined, [...path, { index }], undefined, issues)),
      );
    }
    case "record": {
      const next: Record<string, unknown> = {};
      for (const key of value.keys()) {
        try {
          checkRecordKey(key);
        } catch {
          report(issues, path, "invalid", `Invalid record key ${JSON.stringify(key)}`);
          continue;
        }
        const entry = read(node.value, value.get(key), previous?.[key], [...path, { key }], register, issues);
        if (entry !== INVALID) next[key] = entry;
      }
      return shareObject(previous, next, path, register);
    }
    case "tree":
      return projectTree(node.item, value, previous, path, register, issues);
    case "object": {
      for (const key of value.keys())
        if (key !== ID_KEY && !Object.hasOwn(node.properties, key))
          report(issues, path, "unknown-field", `Unknown stored field: ${key}`);
      const next: Record<string, unknown> = {};
      for (const [key, child] of Object.entries(node.properties)) {
        const projected = project(child, value.get(key), previous?.[key], [...path, key], register, issues);
        if (projected !== undefined) next[key] = projected;
      }
      return shareObject(previous, next, path, register);
    }
  }
}
/** Internal container ID of a projected row or tree node, for snapshot reuse only. */
export const containerOf = (value: unknown) =>
  value && typeof value === "object" ? containers.get(value) : undefined;
/** Rows are reused by container; `dirty` names post-change indices whose content changed. */
export function projectRows(
  item: ObjectNode,
  list: LoroMovableList,
  previous: any,
  path: Path,
  register?: Register,
  dirty?: Set<number>,
  issues?: Issue[],
): any {
  const byContainer = new Map<string, any>();
  for (const row of Array.isArray(previous) ? previous : []) byContainer.set(containerOf(row)!, row);
  const identities = new Map(rowIdentities(list).map((identity) => [identity.index, identity]));
  const rows: any[] = [];
  (list.toArray() as LoroMap[]).forEach((row, index) => {
    const identity = identities.get(index);
    if (!identity) {
      report(issues, [...path, { index }], "invalid", "Expected LoroMap");
      return;
    }
    const { id, problem } = identity;
    // Rows with an identity problem stay visible and addressable through a derived ID.
    if (problem) report(issues, [...path, { id }], "identity", problem);
    const before = byContainer.get(row.id);
    if (before && before.$id === id && dirty && !dirty.has(index)) rows.push(before);
    else rows.push(projectRow(item, row, before, path, register, id, issues));
  });
  return share(previous, rows);
}
export function projectRow(
  item: ObjectNode,
  row: LoroMap,
  before: any,
  path: Path,
  register: Register | undefined,
  id: string,
  issues?: Issue[],
) {
  const rowPath = [...path, { id }];
  const next = project(item, row, before, rowPath, register, issues);
  if (before && next === before && before.$id === id) return before;
  return rowValue(next, id, row.id, rowPath, register);
}
export function rowValue(fields: object, id: string, container: string, path: Path, register?: Register) {
  const value = Object.freeze({ ...fields, $id: id });
  containers.set(value, container);
  register?.(value, path);
  return value;
}
function projectTree(item: ObjectNode, tree: LoroTree, previous: any, path: Path, register?: Register, issues?: Issue[]) {
  const byContainer = new Map<string, any>();
  const index = (nodes: any) => {
    for (const node of Array.isArray(nodes) ? nodes : []) {
      byContainer.set(containerOf(node)!, node);
      index(node.children);
    }
  };
  index(previous);
  const identities = new Map(treeIdentities(tree).map((entry) => [entry.node.id, entry]));
  const visit = (nodes: LoroTreeNode[] | undefined, siblings: any): any =>
    share(
      siblings,
      (nodes ?? []).map((node) => {
        const before = byContainer.get(node.id);
        const { id, problem } = identities.get(node.id)!;
        const nodePath = [...path, { id }];
        if (problem) report(issues, nodePath, "identity", problem);
        const fields = before ? (({ children: _c, $id: _i, ...rest }) => rest)(before) : undefined;
        const data = project(item, node.data, fields, nodePath, register, issues);
        const children = visit(node.children(), before?.children);
        if (before && data === fields && children === before.children && before.$id === id) return before;
        const value = Object.freeze({ ...data, $id: id, children });
        containers.set(value, node.id);
        register?.(value, nodePath);
        return value;
      }),
    );
  return visit(tree.roots(), previous);
}
function share(previous: any, next: any[]) {
  return Array.isArray(previous) &&
    next.length === previous.length &&
    next.every((row: any, i: number) => row === previous[i])
    ? previous
    : Object.freeze(next);
}
function shareObject(previous: any, next: Record<string, unknown>, path: Path, register?: Register) {
  if (
    previous &&
    Object.keys(previous).filter((key) => key !== "$id").length === Object.keys(next).length &&
    Object.keys(next).every((key) => next[key] === previous[key])
  )
    return previous;
  const value = Object.freeze(next);
  register?.(value, path);
  return value;
}
function deepFreeze<T>(value: T): T {
  if (value && typeof value === "object") {
    Object.values(value).forEach(deepFreeze);
    Object.freeze(value);
  }
  return value;
}
