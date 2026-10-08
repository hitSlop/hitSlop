/** Experimental JSON semantics. No production SDK, storage, DOM or host dependencies. */
export type Node = { type: "string" } | { type: "boolean" } |
  { type: "object"; fields: Record<string, Node> } | { type: "list"; item: Node };
export const schema: Node = { type: "object", fields: {
  title: { type: "string" },
  rows: { type: "list", item: { type: "object", fields: {
    text: { type: "string" }, done: { type: "boolean" },
  } } },
} };
export type Row = { $id: string; text: string; done: boolean };
export type Value = { title: string; rows: Row[] };
export type Path = (string | { id: string })[];
export type Anchor = { before: string } | { after: string } | { end: true };
export type Op =
  | { type: "set"; path: Path; value: string | boolean; expected: number }
  | { type: "insert"; path: Path; value: Row; at: Anchor; expected: number }
  | { type: "remove"; path: Path; id: string; expected: number; expectedRow: number }
  | { type: "move"; path: Path; id: string; to: Anchor; expected: number };
export type State = { rev: number; value: Value; revisions: Record<string, number> };
export class Rejected extends Error {
  constructor(public code: string) { super(code); }
}
const reject = (code: string): never => { throw new Rejected(code); };
const safeKey = (key: string) => !["__proto__", "prototype", "constructor", "$id"].includes(key);
const idOK = (id: unknown): id is string => typeof id === "string" && /^[a-zA-Z0-9-]{1,128}$/.test(id);
export const key = (path: Path) => JSON.stringify(path);
export const revision = (state: State, path: Path) => state.revisions[key(path)] ?? 0;

function validate(node: Node, value: any, row = false, depth = 0): void {
  if (depth > 32) reject("too_deep");
  if (node.type === "string" || node.type === "boolean") {
    if (typeof value !== node.type) reject("type_mismatch");
    if (typeof value === "string" && value.length > 1_000_000) reject("too_large");
  } else if (node.type === "list") {
    if (!Array.isArray(value)) reject("type_mismatch");
    if (value.length > 50_000) reject("too_large");
    const ids = new Set<string>();
    for (const item of value) {
      if (!item || !idOK(item.$id)) reject("invalid_id");
      if (ids.has(item.$id)) reject("duplicate_id");
      ids.add(item.$id);
      validate(node.item, item, true, depth + 1);
    }
  } else {
    if (!value || typeof value !== "object" || Array.isArray(value)) reject("type_mismatch");
    for (const field of Object.keys(value)) {
      if (row && field === "$id") continue;
      if (!safeKey(field) || !Object.hasOwn(node.fields, field)) reject("unknown_field");
    }
    for (const [field, child] of Object.entries(node.fields)) validate(child, value[field], false, depth + 1);
  }
}
function bounded(state: State) {
  // UTF-8 bound without TextEncoder (not present in a bare JSContext).
  const json = JSON.stringify(state);
  let bytes = 0;
  for (let i = 0; i < json.length; i++) {
    const c = json.charCodeAt(i);
    if (c < 128) bytes++;
    else if (c < 2048) bytes += 2;
    else if (c >= 0xd800 && c <= 0xdbff && i + 1 < json.length &&
      json.charCodeAt(i + 1) >= 0xdc00 && json.charCodeAt(i + 1) <= 0xdfff) { bytes += 4; i++; }
    else bytes += 3;
    if (bytes > 16 * 1024 * 1024) reject("too_large");
  }
}
export function create(value: Value, descriptor = schema): State {
  validate(descriptor, value);
  const result = { rev: 0, value: JSON.parse(JSON.stringify(value)), revisions: Object.create(null) };
  bounded(result);
  return result;
}
function locate(descriptor: Node, value: any, path: Path): { node: Node; value: any } {
  if (!Array.isArray(path) || path.length > 32) reject("invalid_path");
  let node = descriptor;
  for (const segment of path) {
    if (typeof segment === "string") {
      if (!safeKey(segment) || node.type !== "object" || !Object.hasOwn(node.fields, segment)) reject("invalid_path");
      node = (node as Extract<Node, { type: "object" }>).fields[segment];
      value = value[segment];
    } else {
      if (!segment || !idOK(segment.id) || node.type !== "list") reject("invalid_path");
      value = value.find((r: Row) => r.$id === segment.id);
      if (!value) reject("path_not_found");
      node = (node as Extract<Node, { type: "list" }>).item;
    }
  }
  return { node, value };
}
function position(rows: Row[], anchor: Anchor): number {
  if (!anchor || typeof anchor !== "object" || Object.keys(anchor).length !== 1) reject("invalid_anchor");
  if ("end" in anchor && anchor.end === true) return rows.length;
  const id = "before" in anchor ? anchor.before : "after" in anchor ? anchor.after : undefined;
  const i = rows.findIndex(row => row.$id === id);
  if (!idOK(id) || i < 0) reject("path_not_found");
  return i + ("after" in anchor ? 1 : 0);
}
export function apply(state: State, ops: Op[], descriptor = schema): State {
  if (!Array.isArray(ops) || !ops.length || ops.length > 1024) reject("invalid_batch");
  if (!Number.isSafeInteger(state.rev + 1)) reject("revision_overflow");
  // All preconditions refer to the submitted base, including multi-op batches.
  for (const op of ops) {
    if (!op || !["set", "insert", "remove", "move"].includes(op.type)) reject("unknown_op");
    if (!Number.isSafeInteger(op.expected) || op.expected !== revision(state, op.path)) reject("stale_field");
    if (op.type === "remove" && op.expectedRow !== revision(state, [...op.path, { id: op.id }])) reject("stale_row");
  }
  // Deliberately straightforward full-copy staging; measured and disclosed, not claimed optimal.
  const next: State = JSON.parse(JSON.stringify(state));
  next.rev++;
  for (const op of ops) {
    const found = locate(descriptor, next.value, op.path);
    if (op.type === "set") {
      if (found.node.type !== "string" && found.node.type !== "boolean") reject("type_mismatch");
      validate(found.node, op.value);
      const parent = locate(descriptor, next.value, op.path.slice(0, -1)).value;
      parent[op.path.at(-1) as string] = op.value;
    } else {
      if (found.node.type !== "list") reject("type_mismatch");
      const rows: Row[] = found.value;
      if (op.type === "insert") {
        validate((found.node as Extract<Node, { type: "list" }>).item, op.value, true);
        if (!idOK(op.value.$id)) reject("invalid_id");
        if (rows.some(row => row.$id === op.value.$id)) reject("duplicate_id");
        rows.splice(position(rows, op.at), 0, JSON.parse(JSON.stringify(op.value)));
      } else {
        const index = rows.findIndex(row => row.$id === op.id);
        if (index < 0) reject("path_not_found");
        if (op.type === "remove") rows.splice(index, 1);
        else {
          if (("before" in op.to && op.to.before === op.id) || ("after" in op.to && op.to.after === op.id)) reject("invalid_anchor");
          const [row] = rows.splice(index, 1);
          rows.splice(position(rows, op.to), 0, row);
        }
      }
    }
    next.revisions[key(op.path)] = next.rev;
    // Row revisions protect deletion after another client edited any descendant.
    op.path.forEach((part, i) => { if (typeof part !== "string") next.revisions[key(op.path.slice(0, i + 1))] = next.rev; });
  }
  validate(descriptor, next.value);
  bounded(next);
  return next;
}
export function seed(rows: number): Value {
  return { title: "Engine placement", rows: Array.from({ length: rows }, (_, i) => ({
    $id: `row-${i}`, text: `Row ${i}`, done: false,
  })) };
}

/** Same fixture script runs in Bun and JSC. Literal expectations live in fixtures.json. */
export function fixture(f: any) {
  let state = create(f.initial);
  const results = [];
  for (const step of f.steps) {
    try { state = apply(state, step.ops); results.push({ rev: state.rev, value: state.value }); }
    catch (error) { results.push({ error: error instanceof Rejected ? error.code : String(error), rev: state.rev, value: state.value }); }
  }
  return results;
}
