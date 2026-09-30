import { OperationRejectedError } from "./errors";
export { OperationRejectedError } from "./errors";
/**
 * Descriptors are data. Neither the host nor the CLI evaluates authored callbacks.
 * Only kinds the Rust core, the SDK and a fixture implement are offered here; a new
 * kind lands in all three at once.
 */
export type Text = { kind: "text" };
export type BooleanNode = { kind: "boolean" };
/** An integer that merges concurrent increments; it reads `null` if stored contributions are invalid. */
export type CounterNode = { kind: "counter" };
export interface ObjectNode<P extends Record<string, Node> = Record<string, Node>> {
  kind: "object";
  properties: P;
}
/** Rows with a stable `$id`. */
export interface ListNode<I extends ObjectNode = ObjectNode> {
  kind: "list";
  item: I;
}
export type Node = Text | BooleanNode | CounterNode | ObjectNode | ListNode;

/** Compile-time provenance only; snapshots contain no symbols or schema data. */
declare const snapshotNode: unique symbol;
export type Snapshot<N extends Node> = { readonly [snapshotNode]: N };

export type Value<N extends Node> = ProjectedValue<N>;
type ProjectedValue<N extends Node, Origin extends Node = N> = N extends Text
  ? string
  : N extends CounterNode
    ? number | null
    : N extends BooleanNode
      ? boolean
      : N extends ListNode<infer I>
        ? ReadonlyArray<Value<I> & { readonly $id: string }>
        : N extends ObjectNode<infer P>
          ? { readonly [K in keyof P]: Value<P[K]> } & Snapshot<Origin>
          : never;
export type Input<N extends Node> =
  N extends ListNode<infer I>
    ? Input<I>[]
    : N extends ObjectNode<infer P>
      ? { [K in keyof P]: Input<P[K]> }
      : N extends CounterNode
        ? number
        : Value<N>;
/** Field names, and `{ id }` for a row. */
export type Segment = string | { id: string };
export type Path = Segment[];
export type Field<N extends Node> = { readonly path: Path; readonly node: N };
export type Fields<N extends Node> = Field<N> &
  (N extends ObjectNode<infer P>
    ? { [K in keyof P]: Fields<P[K]> }
    : N extends ListNode<infer I>
      ? { item(id: string): Fields<I> }
      : {});
export type Descriptor = { format: 1; root: ObjectNode };
export type Definition<N extends ObjectNode> = { descriptor: Descriptor; fields: Fields<N> };

export const s = {
  text: (): Text => ({ kind: "text" }),
  boolean: (): BooleanNode => ({ kind: "boolean" }),
  counter: (): CounterNode => ({ kind: "counter" }),
  object: <P extends Record<string, Node>>(properties: P): ObjectNode<P> => ({
    kind: "object",
    properties,
  }),
  list: <I extends ObjectNode>(item: I): ListNode<I> => ({ kind: "list", item }),
};
export function defineDocument<P extends Record<string, Node>>(
  properties: P,
): Definition<ObjectNode<P>> {
  return fromDescriptor({ format: 1, root: s.object(properties) }) as Definition<ObjectNode<P>>;
}
export function fromDescriptor(input: Descriptor): Definition<ObjectNode> {
  const descriptor = JSON.parse(JSON.stringify(input)) as Descriptor;
  if (descriptor.format !== 1) throw new Error("Unsupported schema format");
  checkNode(descriptor.root, 0);
  if (descriptor.root.kind !== "object") throw new Error("Document root must be an object");
  const build = (node: Node, path: Path): any => {
    const field: any = { node, path: Object.freeze(path) };
    if (node.kind === "object")
      for (const [key, child] of Object.entries(node.properties))
        field[key] = build(child, [...path, key]);
    if (node.kind === "list") field.item = (id: string) => build(node.item, [...path, { id }]);
    return Object.freeze(field);
  };
  freeze(descriptor);
  return Object.freeze({ descriptor, fields: build(descriptor.root, []) });
}
function freeze(value: any) {
  if (value && typeof value === "object") {
    Object.values(value).forEach(freeze);
    Object.freeze(value);
  }
}
const allowedKeys: Record<string, string[]> = {
  object: ["properties"],
  list: ["item"],
  text: [],
  boolean: [],
  counter: [],
};
function checkNode(node: Node, depth: number): void {
  if (!node || typeof node !== "object" || depth > 16) throw new Error("Invalid/deep schema");
  const keys = allowedKeys[node.kind];
  if (!keys) throw new Error(`Unsupported schema kind: ${String(node.kind)}`);
  if (Object.keys(node).some((k) => k !== "kind" && !keys.includes(k)))
    throw new Error("Unknown schema option");
  switch (node.kind) {
    case "list":
      if (node.item?.kind !== "object") throw new Error("Lists contain object rows");
      return checkNode(node.item, depth + 1);
    case "object":
      if (!node.properties || typeof node.properties !== "object" || Array.isArray(node.properties))
        throw new Error("Invalid schema node");
      for (const [key, child] of Object.entries(node.properties)) {
        if (
          !/^[a-zA-Z][a-zA-Z0-9_]*$/.test(key) ||
          ["path", "node", "item", "constructor", "prototype"].includes(key)
        )
          throw new Error(`Reserved/invalid field: ${key}`);
        checkNode(child, depth + 1);
      }
  }
}
/** Checks a value (such as `initial.json`) against a descriptor; rows may carry `$id`. */
export function validate(node: Node, value: unknown, row = false): void {
  switch (node.kind) {
    case "text":
      if (typeof value !== "string") throw new OperationRejectedError("Expected string");
      return;
    case "counter":
      if (!Number.isSafeInteger(value)) throw new OperationRejectedError("Expected safe integer");
      return;
    case "boolean":
      if (typeof value !== "boolean") throw new OperationRejectedError("Expected boolean");
      return;
    case "list":
      if (!Array.isArray(value)) throw new OperationRejectedError("Expected list");
      for (const item of value) validate(node.item, item, true);
      return;
    case "object": {
      if (!value || typeof value !== "object" || Array.isArray(value))
        throw new OperationRejectedError("Expected object");
      const record = value as Record<string, unknown>;
      for (const key of Object.keys(record))
        if (!Object.hasOwn(node.properties, key) && !(row && key === "$id"))
          throw new OperationRejectedError(`Unknown field: ${key}`);
      for (const [key, child] of Object.entries(node.properties)) validate(child, record[key]);
    }
  }
}
export function canonicalJSON(value: unknown): string {
  const canonical = (v: any): any =>
    Array.isArray(v)
      ? v.map(canonical)
      : v && typeof v === "object"
        ? Object.fromEntries(
            Object.keys(v)
              .sort()
              .map((k) => [k, canonical(v[k])]),
          )
        : v;
  return JSON.stringify(canonical(value));
}
export const schemaKey = (descriptor: Descriptor) => canonicalJSON(descriptor);
