import { OperationRejectedError } from "./errors";
export { OperationRejectedError } from "./errors";
/**
 * Descriptors are data. Neither the host nor the CLI evaluates authored callbacks.
 * Only kinds the Rust core, the SDK and a fixture implement are offered here; a new
 * kind lands in all three at once.
 */
export type Text = { kind: "text" };
export type BooleanNode = { kind: "boolean" };
/** Last writer wins. `maxLength` counts UTF-16 units. */
export type StringNode = { kind: "string"; maxLength?: number };
/** A finite number; bounds are inclusive. */
export type NumberNode = { kind: "number"; min?: number; max?: number };
/** A safe integer; bounds are inclusive. */
export type IntegerNode = { kind: "integer"; min?: number; max?: number };
export type EnumNode<V extends readonly string[] = readonly string[]> = { kind: "enum"; values: V };
export type Scalar = StringNode | NumberNode | IntegerNode | BooleanNode | EnumNode;
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
/** Absent until set; `clear()` removes it. Holds a scalar or an object. */
export interface OptionalNode<S extends Scalar | ObjectNode = Scalar | ObjectNode> {
  kind: "optional";
  inner: S;
}
export type Node = Text | Scalar | CounterNode | ObjectNode | ListNode | OptionalNode;

/** Compile-time provenance only; snapshots contain no symbols or schema data. */
declare const snapshotNode: unique symbol;
export type Snapshot<N extends Node> = { readonly [snapshotNode]: N };

type OptionalKeys<P extends Record<string, Node>> = {
  [K in keyof P]: P[K] extends OptionalNode ? K : never;
}[keyof P];
type ObjectValue<P extends Record<string, Node>> = {
  readonly [K in Exclude<keyof P, OptionalKeys<P>>]: Value<P[K]>;
} & { readonly [K in OptionalKeys<P>]?: Value<P[K]> };
type ObjectInput<P extends Record<string, Node>> = {
  [K in Exclude<keyof P, OptionalKeys<P>>]: Input<P[K]>;
} & { [K in OptionalKeys<P>]?: Input<P[K]> };
export type Value<N extends Node> = ProjectedValue<N>;
type ProjectedValue<N extends Node, Origin extends Node = N> = N extends Text | StringNode
  ? string
  : N extends CounterNode
    ? number | null
    : N extends NumberNode | IntegerNode
      ? number
      : N extends BooleanNode
        ? boolean
        : N extends EnumNode<infer V>
          ? V[number]
          : N extends OptionalNode<infer S>
            ? ProjectedValue<S, Origin> | undefined
            : N extends ListNode<infer I>
              ? ReadonlyArray<Value<I> & { readonly $id: string }>
              : N extends ObjectNode<infer P>
                ? ObjectValue<P> & Snapshot<Origin>
                : never;
export type Input<N extends Node> =
  N extends ListNode<infer I>
    ? Input<I>[]
    : N extends OptionalNode<infer S>
      ? Input<S> | undefined
      : N extends ObjectNode<infer P>
        ? ObjectInput<P>
        : N extends CounterNode
          ? number
          : Value<N>;
/** Field names, and `{ id }` for a row. */
export type Segment = string | { id: string };
export type Path = Segment[];
export type Field<N extends Node> = { readonly path: Path; readonly node: N };
type Unwrap<N extends Node> = N extends OptionalNode<infer S> ? S : N;
export type Fields<N extends Node> = Field<N> &
  (Unwrap<N> extends ObjectNode<infer P>
    ? { [K in keyof P]: Fields<P[K]> }
    : Unwrap<N> extends ListNode<infer I>
      ? { item(id: string): Fields<I> }
      : {});
export type Descriptor = { format: 1; root: ObjectNode };
export type Definition<N extends ObjectNode> = { descriptor: Descriptor; fields: Fields<N> };

type Bounds = { min?: number; max?: number };
const options = <T extends object>(base: T, extra: object | undefined) =>
  ({
    ...base,
    ...Object.fromEntries(Object.entries(extra ?? {}).filter(([, v]) => v !== undefined)),
  }) as T;
export const s = {
  text: (): Text => ({ kind: "text" }),
  boolean: (): BooleanNode => ({ kind: "boolean" }),
  string: (extra?: { maxLength?: number }): StringNode => options({ kind: "string" }, extra),
  number: (extra?: Bounds): NumberNode => options({ kind: "number" }, extra),
  integer: (extra?: Bounds): IntegerNode => options({ kind: "integer" }, extra),
  enum: <const V extends readonly [string, ...string[]]>(values: V): EnumNode<V> => ({
    kind: "enum",
    values,
  }),
  optional: <S extends Scalar | ObjectNode>(inner: S): OptionalNode<S> => ({ kind: "optional", inner }),
  counter: (): CounterNode => ({ kind: "counter" }),
  object: <P extends Record<string, Node>>(properties: P): ObjectNode<P> => ({
    kind: "object",
    properties,
  }),
  list: <I extends ObjectNode>(item: I): ListNode<I> => ({ kind: "list", item }),
};
export const unwrap = (node: Node): Exclude<Node, OptionalNode> =>
  node.kind === "optional" ? node.inner : node;
export const isScalar = (node: Node): node is Scalar =>
  ["string", "number", "integer", "boolean", "enum"].includes(node.kind);
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
    const inner = unwrap(node);
    if (inner.kind === "object")
      for (const [key, child] of Object.entries(inner.properties))
        field[key] = build(child, [...path, key]);
    if (inner.kind === "list") field.item = (id: string) => build(inner.item, [...path, { id }]);
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
  optional: ["inner"],
  enum: ["values"],
  string: ["maxLength"],
  number: ["min", "max"],
  integer: ["min", "max"],
  text: [],
  boolean: [],
  counter: [],
};
const bound = (value: unknown) => value === undefined || (typeof value === "number" && Number.isFinite(value));
/** Mirrors the Rust core's descriptor check, so authoring fails before the app does. */
function checkNode(node: Node, depth: number): void {
  if (!node || typeof node !== "object" || depth > 16) throw new Error("Invalid/deep schema");
  const keys = allowedKeys[node.kind];
  if (!keys) throw new Error(`Unsupported schema kind: ${String(node.kind)}`);
  if (Object.keys(node).some((k) => k !== "kind" && !keys.includes(k)))
    throw new Error("Unknown schema option");
  switch (node.kind) {
    case "string":
      if (node.maxLength !== undefined && !(Number.isInteger(node.maxLength) && node.maxLength >= 0))
        throw new Error("maxLength must be a non-negative integer");
      return;
    case "number":
    case "integer":
      if (!bound(node.min) || !bound(node.max) || (node.min ?? -Infinity) > (node.max ?? Infinity))
        throw new Error("Numeric bounds must be finite with min ≤ max");
      if (node.kind === "integer" && [node.min, node.max].some((b) => b !== undefined && !Number.isSafeInteger(b)))
        throw new Error("Integer bounds must be safe integers");
      return;
    case "enum":
      if (
        !Array.isArray(node.values) ||
        !node.values.length ||
        node.values.length > 1024 ||
        node.values.some((v) => typeof v !== "string") ||
        new Set(node.values).size !== node.values.length
      )
        throw new Error("Enum needs 1–1024 unique string values");
      return;
    case "optional":
      if (!node.inner || (!isScalar(node.inner) && node.inner.kind !== "object"))
        throw new Error("Optional holds a scalar or an object");
      return checkNode(node.inner, depth + 1);
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
const utf16Length = (text: string) => text.length;
/** Checks a value (such as `initial.json`) against a descriptor with the core's write
 * rules; rows may carry `$id`, and optional fields may be omitted. */
export function validate(node: Node, value: unknown, row = false): void {
  const inBounds = (n: number, min?: number, max?: number) => {
    if ((min !== undefined && n < min) || (max !== undefined && n > max))
      throw new OperationRejectedError(`Expected a number from ${min ?? "-∞"} to ${max ?? "∞"}`);
  };
  switch (node.kind) {
    case "optional":
      return validate(node.inner, value);
    case "text":
      if (typeof value !== "string") throw new OperationRejectedError("Expected string");
      return;
    case "string":
      if (typeof value !== "string") throw new OperationRejectedError("Expected string");
      if (node.maxLength !== undefined && utf16Length(value) > node.maxLength)
        throw new OperationRejectedError(`Expected at most ${node.maxLength} characters`);
      return;
    case "enum":
      if (typeof value !== "string" || !node.values.includes(value))
        throw new OperationRejectedError("Unknown enum value");
      return;
    case "number":
      if (typeof value !== "number" || !Number.isFinite(value))
        throw new OperationRejectedError("Expected finite number");
      return inBounds(value, node.min, node.max);
    case "integer":
      if (!Number.isSafeInteger(value)) throw new OperationRejectedError("Expected safe integer");
      return inBounds(value as number, node.min, node.max);
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
      for (const [key, child] of Object.entries(node.properties)) {
        if (child.kind === "optional" && record[key] === undefined) continue;
        validate(child, record[key]);
      }
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
