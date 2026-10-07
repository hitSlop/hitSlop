export type { Command, CommandContext } from "./commands";
import { makeCommand, type CommandSpec, type Command } from "./commands";
import type { Arguments } from "./commands";
export type { DocumentError } from "./errors";
export { isDocumentError, isRejected } from "./errors";
export { defineSlop, type AppDeclaration } from "./slop";
export type { Scope } from "./abi";
export type { InsertResult } from "./handle-types";
import type { Scope } from "./abi";
import type { At, Handle as ModeHandle } from "./handle-types";
import { fromDescriptor } from "./descriptor";
import { documentFor } from "./app/context";
/**
 * Descriptors are data. Neither the host nor the CLI evaluates authored callbacks.
 * Only kinds the Rust core, the SDK and a fixture implement are offered here; a new
 * kind lands in all three at once.
 */
export type Text = Description & { kind: "text" };
export type BooleanNode = Description & { kind: "boolean" };
/** Last writer wins. Length bounds count Unicode code points, not UTF-16 units. */
export type StringNode = Description & { kind: "string"; minLength?: number; maxLength?: number };
/** A finite number; bounds are inclusive. */
export type NumberNode = Description & { kind: "number"; min?: number; max?: number };
/** A safe integer; bounds are inclusive. */
export type IntegerNode = Description & { kind: "integer"; min?: number; max?: number };
export type EnumNode<V extends readonly string[] = readonly string[]> = Description & { kind: "enum"; values: V };
export type Scalar = StringNode | NumberNode | IntegerNode | BooleanNode | EnumNode;
/** An exact checked integer counter in the single-writer owner. */
export type CounterNode = Description & { kind: "counter" };
export interface ObjectNode<P extends Record<string, Node> = Record<string, Node>> extends Description {
  kind: "object";
  properties: P;
}
/** Rows with a stable `$id` (object items), or plain values addressed by index (scalar items). */
export interface ListNode<I extends ObjectNode | Scalar = ObjectNode | Scalar> extends Description {
  kind: "list";
  item: I;
}
/** Entries by string key; each entry is absent until `put` and removed by `delete`. */
export interface RecordNode<V extends Scalar | ObjectNode = Scalar | ObjectNode> extends Description {
  kind: "record";
  value: V;
}
/** Absent until set; `clear()` removes it. Holds a scalar, text or an object. */
export interface OptionalNode<S extends Scalar | Text | ObjectNode = Scalar | Text | ObjectNode> extends Description {
  kind: "optional";
  inner: S;
}
export type Description = { description?: string };
export type Node = Text | Scalar | CounterNode | ObjectNode | ListNode | RecordNode | OptionalNode;

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
  : N extends CounterNode | NumberNode | IntegerNode
    ? number
    : N extends BooleanNode
      ? boolean
      : N extends EnumNode<infer V>
        ? V[number]
        : N extends OptionalNode<infer S>
          ? ProjectedValue<S, Origin> | undefined
          : N extends ListNode<infer I>
            ? I extends ObjectNode
              ? ReadonlyArray<Value<I> & { readonly $id: string }>
              : ReadonlyArray<Value<I>>
            : N extends RecordNode<infer V>
              ? { readonly [key: string]: Value<V> } & Snapshot<Origin>
              : N extends ObjectNode<infer P>
              ? ObjectValue<P> & Snapshot<Origin>
              : never;
export type Input<N extends Node> =
  N extends ListNode<infer I>
    ? Input<I>[]
    : N extends RecordNode<infer V>
      ? { [key: string]: Input<V> }
      : N extends OptionalNode<infer S>
      ? Input<S> | undefined
      : N extends ObjectNode<infer P>
        ? ObjectInput<P>
        : N extends CounterNode
          ? number
          : Value<N>;
/** Field names and record keys, `{ id }` for a row, and `{ index }` for a scalar-list element. */
export type Segment = import("../schema/core").Segment;
export type Path = Segment[];
export type Descriptor = ObjectNode;
export type Definition<N extends ObjectNode> = { descriptor: N };

type Bounds = Description & { min?: number; max?: number };
const options = <T extends object>(base: T, extra: object | undefined) =>
  ({
    ...base,
    ...Object.fromEntries(Object.entries(extra ?? {}).filter(([, v]) => v !== undefined)),
  }) as T;
export const s = {
  text: (extra?: Description): Text => options({ kind: "text" }, extra),
  boolean: (extra?: Description): BooleanNode => options({ kind: "boolean" }, extra),
  string: (extra?: Description & { minLength?: number; maxLength?: number }): StringNode => options({ kind: "string" }, extra),
  number: (extra?: Bounds): NumberNode => options({ kind: "number" }, extra),
  integer: (extra?: Bounds): IntegerNode => options({ kind: "integer" }, extra),
  enum: <const V extends readonly [string, ...string[]]>(values: V, extra?: Description): EnumNode<V> => options({
    kind: "enum",
    values,
  }, extra),
  optional: <S extends Scalar | Text | ObjectNode>(inner: S, extra?: Description): OptionalNode<S> => options({ kind: "optional", inner }, extra),
  record: <V extends Scalar | ObjectNode>(value: V, extra?: Description): RecordNode<V> => options({ kind: "record", value }, extra),
  counter: (extra?: Description): CounterNode => options({ kind: "counter" }, extra),
  object: <P extends Record<string, Node>>(properties: P, extra?: Description): ObjectNode<P> => options({
    kind: "object",
    properties,
  }, extra),
  list: <I extends ObjectNode | Scalar>(item: I, extra?: Description): ListNode<I> => options({ kind: "list", item }, extra),
};
/** A field's write handle. Writes resolve once `current` shows them; inside `change()`
 * the same handles collect synchronously. */
export type Handle<N extends Node> = ModeHandle<N, "live">;
/** What `defineDocument` returns: the descriptor, and, once the app is mounted, the live
 * document. Components `import doc from "./schema"` and read `doc.current`. */
export type DocumentDefinition<N extends ObjectNode> = Definition<N> & LiveDocument<N> & {
  command<A extends Arguments, R>(spec: CommandSpec<N, A, R>): Command<A, R>;
};
export interface LiveDocument<N extends ObjectNode> {
  /** Immutable snapshot. Unchanged rows keep their identity. */
  readonly current: Value<N>;
  readonly fields: Handle<N>;
  /** The handle for an object taken from `current`: the root, a row, a nested object or
   * a record entry. */
  readonly at: At;
  /** Collects synchronously; resolves after acceptance and local publication. */
  change<R>(callback: (tx: Scope<N>) => R): Promise<R>;
  /** Durability barrier: sends unsent text, waits for pending writes, then for storage. */
  flush(): Promise<void>;
  /** Edit ▸ Undo: the last step, the person's or an agent's. Resolves once `current`
   * shows the result. */
  undo(): Promise<void>;
  redo(): Promise<void>;
}
export function defineDocument<P extends Record<string, Node>>(
  properties: P,
): DocumentDefinition<ObjectNode<P>> {
  type Live = LiveDocument<ObjectNode<P>>;
  const definition = { ...fromDescriptor(s.object(properties)) };
  // Non-enumerable, and read only once mounted: build tools see just the descriptor.
  const live = () => documentFor(definition) as Live;
  return Object.freeze(
    Object.defineProperties(definition, {
      command: { value: (spec: CommandSpec<ObjectNode<P>, Arguments, unknown>) => makeCommand(definition, spec) },
      current: { get: () => live().current },
      fields: { get: () => live().fields },
      at: { get: () => live().at },
      change: { value: ((callback) => live().change(callback)) satisfies Live["change"] },
      flush: { value: () => live().flush() },
      undo: { value: () => live().undo() },
      redo: { value: () => live().redo() },
    }),
  ) as DocumentDefinition<ObjectNode<P>>;
}
