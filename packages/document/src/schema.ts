import { OperationRejectedError } from "./errors";
export { OperationRejectedError } from "./errors";
/** v1 descriptors are data. Neither the host nor CLI evaluates authored callbacks. */
export type Text = { kind: "text" };
export type Expand = "after" | "before" | "none" | "both";
export type RichText<M extends Record<string, Expand> = Record<string, Expand>> = {
  kind: "richtext";
  marks: M;
};
export type StringNode = { kind: "string"; maxLength?: number };
export type NumberNode = { kind: "number"; min?: number; max?: number };
export type IntegerNode = { kind: "integer"; min?: number; max?: number };
export type BooleanNode = { kind: "boolean" };
export type EnumNode<V extends readonly string[] = readonly string[]> = { kind: "enum"; values: V };
export type Scalar = StringNode | NumberNode | IntegerNode | BooleanNode | EnumNode;
export type CounterNode = { kind: "counter" };
export interface ObjectNode<P extends Record<string, Node> = Record<string, Node>> {
  kind: "object";
  properties: P;
}
export interface ListNode<I extends ObjectNode | Scalar = ObjectNode | Scalar> {
  kind: "list";
  item: I;
}
export interface RecordNode<V extends ValueNode = ValueNode> {
  kind: "record";
  value: V;
}
export interface TreeNode<I extends ObjectNode = ObjectNode> {
  kind: "tree";
  item: I;
}
/** Any node that always has a value. */
export type ValueNode =
  | Scalar
  | Text
  | RichText
  | CounterNode
  | ObjectNode
  | ListNode
  | RecordNode<any>
  | TreeNode;
export interface OptionalNode<S extends ValueNode = ValueNode> {
  kind: "optional";
  inner: S;
}
export type Node = ValueNode | OptionalNode;
export type Composite = Exclude<ValueNode, Scalar>;

export type Delta = { readonly insert: string; readonly attributes?: Readonly<Record<string, MarkValue>> };
export type MarkValue = string | number | boolean | null;
export type RichTextValue = { readonly text: string; readonly delta: readonly Delta[] };
/** Compile-time provenance only; snapshots contain no symbols or schema data. */
declare const snapshotNode: unique symbol;
export type Snapshot<N extends Node> = { readonly [snapshotNode]: N };
export type TreeNodeValue<I extends ObjectNode> = Value<I> & {
  readonly $id: string;
  readonly children: ReadonlyArray<TreeNodeValue<I>>;
};
export type TreeInput<I extends ObjectNode> = Input<I> & { children?: TreeInput<I>[] };

type OptionalKeys<P extends Record<string, Node>> = {
  [K in keyof P]: P[K] extends OptionalNode ? K : never;
}[keyof P];
type ObjectInput<P extends Record<string, Node>> = {
  [K in Exclude<keyof P, OptionalKeys<P>>]: Input<P[K]>;
} & { [K in OptionalKeys<P>]?: Input<P[K]> };
type ObjectValue<P extends Record<string, Node>> = {
  readonly [K in Exclude<keyof P, OptionalKeys<P>>]: Value<P[K]>;
} & { readonly [K in OptionalKeys<P>]?: Value<P[K]> };
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
          : N extends RichText
            ? RichTextValue
            : N extends OptionalNode<infer S>
              ? ProjectedValue<S, Origin> | undefined
              : N extends ListNode<infer I>
                ? I extends ObjectNode
                  ? ReadonlyArray<Value<I> & { readonly $id: string }>
                  : ReadonlyArray<Value<I>>
                : N extends RecordNode<infer V>
                  ? { readonly [key: string]: Value<V> } & Snapshot<Origin>
                  : N extends TreeNode<infer I>
                    ? ReadonlyArray<TreeNodeValue<I>>
                    : N extends ObjectNode<infer P>
                      ? ObjectValue<P> & Snapshot<Origin>
                      : never;
export type Input<N extends Node> =
  N extends ListNode<infer I>
    ? Input<I>[]
    : N extends RecordNode<infer V>
      ? { [key: string]: Input<V> }
      : N extends TreeNode<infer I>
        ? TreeInput<I>[]
        : N extends RichText
          ? string
          : N extends OptionalNode<infer S>
            ? Input<S> | undefined
            : N extends ObjectNode<infer P>
              ? ObjectInput<P>
              : N extends CounterNode
                ? number
                : Value<N>;
/** Row and tree node IDs, record keys, and scalar list indices address values inside containers. */
export type Segment = string | { id: string } | { key: string } | { index: number };
export type Path = Segment[];
export type Field<N extends Node> = { readonly path: Path; readonly node: N };
type Unwrap<N extends Node> = N extends OptionalNode<infer S> ? S : N;
export type Fields<N extends Node> = Field<N> &
  (Unwrap<N> extends ObjectNode<infer P>
    ? { [K in keyof P]: Fields<P[K]> }
    : Unwrap<N> extends ListNode<infer I>
      ? I extends ObjectNode
        ? { item(id: string): Fields<I> }
        : {}
      : Unwrap<N> extends TreeNode<infer I>
        ? { item(id: string): Fields<I> }
        : Unwrap<N> extends RecordNode<infer V>
          ? { entry(key: string): Fields<V> }
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
  richtext: <const M extends Record<string, Expand>>(marks: M): RichText<M> => ({
    kind: "richtext",
    marks,
  }),
  string: (extra?: { maxLength?: number }): StringNode => options({ kind: "string" }, extra),
  number: (extra?: Bounds): NumberNode => options({ kind: "number" }, extra),
  integer: (extra?: Bounds): IntegerNode => options({ kind: "integer" }, extra),
  boolean: (): BooleanNode => ({ kind: "boolean" }),
  counter: (): CounterNode => ({ kind: "counter" }),
  enum: <const V extends readonly [string, ...string[]]>(values: V): EnumNode<V> => ({
    kind: "enum",
    values,
  }),
  optional: <S extends ValueNode>(inner: S): OptionalNode<S> => ({ kind: "optional", inner }),
  object: <P extends Record<string, Node>>(properties: P): ObjectNode<P> => ({
    kind: "object",
    properties,
  }),
  list: <I extends ObjectNode | Scalar>(item: I): ListNode<I> => ({ kind: "list", item }),
  record: <V extends ValueNode>(value: V): RecordNode<V> => ({ kind: "record", value }),
  tree: <I extends ObjectNode>(item: I): TreeNode<I> => ({ kind: "tree", item }),
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
  textStyles(descriptor.root);
  const build = (node: Node, path: Path): any => {
    const field: any = { node, path: Object.freeze(path) };
    const inner = unwrap(node);
    if (inner.kind === "object")
      for (const [key, child] of Object.entries(inner.properties))
        field[key] = build(child, [...path, key]);
    if ((inner.kind === "list" && inner.item.kind === "object") || inner.kind === "tree")
      field.item = (id: string) => build(inner.item, [...path, { id }]);
    if (inner.kind === "record") field.entry = (key: string) => build(inner.value, [...path, { key }]);
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
export const unwrap = (node: Node): ValueNode => (node.kind === "optional" ? node.inner : node);
export const isScalar = (node: Node): node is Scalar =>
  ["string", "number", "integer", "boolean", "enum"].includes(node.kind);
const allowedKeys: Record<string, string[]> = {
  object: ["properties"],
  list: ["item"],
  record: ["value"],
  tree: ["item"],
  optional: ["inner"],
  enum: ["values"],
  richtext: ["marks"],
  string: ["maxLength"],
  number: ["min", "max"],
  integer: ["min", "max"],
  text: [],
  boolean: [],
  counter: [],
};
const bound = (value: unknown) => value === undefined || (typeof value === "number" && Number.isFinite(value));
function checkNode(node: Node, depth: number): void {
  if (!node || typeof node !== "object" || depth > 16) throw new Error("Invalid/deep schema");
  const keys = allowedKeys[node.kind];
  if (!keys) throw new Error(`Unknown schema kind: ${String(node.kind)}`);
  if (Object.keys(node).some((k) => k !== "kind" && !keys.includes(k)))
    throw new Error("Unknown schema option");
  switch (node.kind) {
    case "text":
    case "boolean":
    case "counter":
      return;
    case "string":
      if (node.maxLength !== undefined && !(Number.isInteger(node.maxLength) && node.maxLength >= 0))
        throw new Error("maxLength must be a non-negative integer");
      return;
    case "number":
    case "integer":
      if (!bound(node.min) || !bound(node.max) || (node.min ?? -Infinity) > (node.max ?? Infinity))
        throw new Error("Invalid numeric bounds");
      if (node.kind === "integer" && [node.min, node.max].some((b) => b !== undefined && !Number.isInteger(b)))
        throw new Error("Integer bounds must be integers");
      return;
    case "enum":
      if (
        !Array.isArray(node.values) ||
        !node.values.length ||
        node.values.some((v) => typeof v !== "string") ||
        new Set(node.values).size !== node.values.length
      )
        throw new Error("Enum requires unique string values");
      return;
    case "richtext":
      if (
        !node.marks ||
        typeof node.marks !== "object" ||
        Object.entries(node.marks).some(
          ([k, v]) => !/^[a-zA-Z][a-zA-Z0-9_:-]*$/.test(k) || !["after", "before", "none", "both"].includes(v),
        )
      )
        throw new Error("Rich text marks map names to after/before/none/both");
      return;
    case "optional":
      if (!node.inner || node.inner.kind === ("optional" as string))
        throw new Error("Optional requires a non-optional value");
      if (node.inner.kind === "list" && node.inner.item.kind !== "object")
        throw new Error("Scalar lists cannot be optional; use an empty list");
      if (node.inner.kind === "object")
        for (const key of ["set", "clear"])
          if (Object.hasOwn(node.inner.properties ?? {}, key))
            throw new Error(`Reserved/invalid field in optional object: ${key}`);
      return checkNode(node.inner, depth + 1);
    case "list":
      if (!node.item || (node.item.kind !== "object" && !isScalar(node.item)))
        throw new Error("Lists contain object rows or scalars");
      return checkNode(node.item, depth + 1);
    case "record":
      if (!node.value || node.value.kind === ("optional" as string))
        throw new Error("Record values cannot be optional; delete the entry instead");
      return checkNode(node.value, depth + 1);
    case "tree":
      if (node.item?.kind !== "object") throw new Error("Trees require object nodes");
      if (Object.hasOwn(node.item.properties ?? {}, "children"))
        throw new Error("Reserved/invalid field in tree node: children");
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
/** Loro text styles are document-wide; every rich text field must agree on each mark's expansion. */
export function textStyles(root: Node): Record<string, { expand: Expand }> {
  const styles: Record<string, { expand: Expand }> = {};
  const visit = (node: Node): void => {
    if (node.kind === "richtext")
      for (const [key, expand] of Object.entries(node.marks)) {
        if (styles[key] && styles[key].expand !== expand)
          throw new Error(`Rich text mark ${key} has conflicting expansion`);
        styles[key] = { expand };
      }
    else if (node.kind === "optional") visit(node.inner);
    else if (node.kind === "list" || node.kind === "tree") visit(node.item);
    else if (node.kind === "record") visit(node.value);
    else if (node.kind === "object") Object.values(node.properties).forEach(visit);
  };
  visit(root);
  return styles;
}
export const MAX_RECORD_KEY = 256;
export function checkRecordKey(key: unknown): asserts key is string {
  if (
    typeof key !== "string" ||
    !key.length ||
    key.length > MAX_RECORD_KEY ||
    ["__proto__", "constructor", "prototype"].includes(key)
  )
    throw new OperationRejectedError("Record keys are 1-256 characters and not reserved names");
}
const isMark = (v: unknown) =>
  v === null || typeof v === "string" || typeof v === "boolean" || (typeof v === "number" && Number.isFinite(v));
export function validate(node: Node, value: unknown): void {
  switch (node.kind) {
    case "optional":
      if (value !== undefined) validate(node.inner, value);
      return;
    case "text":
    case "richtext":
      if (typeof value !== "string") throw new OperationRejectedError("Expected string");
      return;
    case "string":
      if (typeof value !== "string") throw new OperationRejectedError("Expected string");
      if (node.maxLength !== undefined && value.length > node.maxLength)
        throw new OperationRejectedError(`Expected at most ${node.maxLength} characters`);
      return;
    case "number":
    case "integer":
    case "counter":
      if (typeof value !== "number" || !Number.isFinite(value))
        throw new OperationRejectedError("Expected finite number");
      if (node.kind === "counter") return;
      if (node.kind === "integer" && !Number.isInteger(value))
        throw new OperationRejectedError("Expected integer");
      if ((node.min !== undefined && value < node.min) || (node.max !== undefined && value > node.max))
        throw new OperationRejectedError(`Expected a number from ${node.min ?? "-∞"} to ${node.max ?? "∞"}`);
      return;
    case "boolean":
      if (typeof value !== "boolean") throw new OperationRejectedError("Expected boolean");
      return;
    case "enum":
      if (typeof value !== "string" || !node.values.includes(value))
        throw new OperationRejectedError("Unknown enum value");
      return;
    case "list":
      if (!Array.isArray(value)) throw new OperationRejectedError("Expected list");
      for (const item of value) validate(node.item, item);
      return;
    case "record":
      if (!value || typeof value !== "object" || Array.isArray(value))
        throw new OperationRejectedError("Expected record");
      for (const [key, entry] of Object.entries(value)) {
        checkRecordKey(key);
        validate(node.value, entry);
      }
      return;
    case "tree":
      if (!Array.isArray(value)) throw new OperationRejectedError("Expected tree node list");
      for (const item of value) {
        if (!item || typeof item !== "object" || Array.isArray(item))
          throw new OperationRejectedError("Expected tree node");
        const { children, ...fields } = item as Record<string, unknown>;
        validate(node.item, fields);
        if (children !== undefined) validate(node, children);
      }
      return;
    case "object": {
      if (!value || typeof value !== "object" || Array.isArray(value))
        throw new OperationRejectedError("Expected object");
      const record = value as Record<string, unknown>;
      for (const key of Object.keys(record))
        if (!Object.hasOwn(node.properties, key))
          throw new OperationRejectedError(`Unknown field: ${key}`);
      for (const [key, child] of Object.entries(node.properties)) validate(child, record[key]);
    }
  }
}
export function validateMark(node: RichText, key: unknown, value: unknown) {
  if (typeof key !== "string" || !Object.hasOwn(node.marks, key))
    throw new OperationRejectedError(`Unknown rich text mark: ${String(key)}`);
  if (!isMark(value)) throw new OperationRejectedError("Mark values are JSON scalars");
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
