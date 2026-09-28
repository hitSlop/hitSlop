import type { Commands, Destination } from "./operations";
import { isScalar, unwrap } from "./schema";
import type {
  CounterNode,
  Input,
  ListNode,
  MarkValue,
  Node,
  ObjectNode,
  OptionalNode,
  Path,
  RecordNode,
  RichText,
  Scalar,
  Snapshot,
  Text,
  TreeInput,
  TreeNode,
  Value,
} from "./schema";

export type InsertResult = { readonly id: string };
/** Resolve a snapshot's original schema, including text/register distinctions. */
export type At = <N extends Node>(value: Snapshot<N>) => Handle<N>;
export type TextHandle = {
  replace(value: string): void;
  /** Delete `deleteCount` UTF-16 units at `index`, then insert `insert` there. */
  splice(index: number, deleteCount: number, insert?: string): void;
};
export type TextRange = { start: number; end: number };
export type RichTextHandle<M extends string = string> = TextHandle & {
  mark(range: TextRange, key: M, value: MarkValue): void;
  unmark(range: TextRange, key: M): void;
};
export type ScalarHandle<V> = {
  set(value: V): void;
  /** Show a local value without writing history; `set` or the next flush commits it. */
  preview(value: V): void;
};
export type RowDestination = { before: string } | { after: string };
export type TreeDestination = { before: string } | { after: string } | { parent: string | null };
type ContainerHandle<N extends Node> =
  N extends ObjectNode<infer P>
    ? { readonly [K in keyof P]: Handle<P[K]> }
    : N extends ListNode<infer I>
      ? I extends ObjectNode
        ? {
            item(id: string): Handle<I>;
            insert(value: Input<I>, destination?: RowDestination): InsertResult;
            remove(id: string): void;
            move(id: string, destination: RowDestination): void;
          }
        : {
            insert(value: Value<I>, index?: number): void;
            set(index: number, value: Value<I>): void;
            preview(index: number, value: Value<I>): void;
            remove(index: number, count?: number): void;
            move(from: number, to: number): void;
            /** Rewrite the list, keeping unchanged positions. */
            replace(values: Value<I>[]): void;
          }
      : N extends RecordNode<infer V>
        ? {
            entry(key: string): Handle<V>;
            put(key: string, value: Input<V>): void;
            delete(key: string): void;
          }
        : N extends TreeNode<infer I>
          ? {
              item(id: string): Handle<I>;
              insert(value: TreeInput<I>, destination?: TreeDestination): InsertResult;
              remove(id: string): void;
              move(id: string, destination: TreeDestination): void;
            }
          : N extends CounterNode
            ? { increment(by?: number): void; decrement(by?: number): void }
            : N extends RichText<infer M>
              ? RichTextHandle<keyof M & string>
              : N extends Text
                ? TextHandle
                : N extends Scalar
                  ? ScalarHandle<Value<N>>
                  : never;
export type Handle<N extends Node> =
  N extends OptionalNode<infer S>
    ? S extends Scalar
      ? ScalarHandle<Value<S>> & { clear(): void }
      : ContainerHandle<S> & { set(value: Input<S>): void; clear(): void }
    : ContainerHandle<N>;

/** Adapters read snapshot values without making handles readable author state. */
export interface Observer {
  beforeFlush?(draft: () => void): () => void;
  read(path: Path): unknown;
  subscribe(listener: () => void): () => void;
  preview?(path: Path, value: unknown): void;
}
const observed = new WeakMap<object, { source: Observer; path: Path; node: Node }>();
export function observe(handle: object) {
  const observer = observed.get(handle);
  if (!observer) throw new Error("Binding requires a live document handle");
  return observer;
}

export function nodeAt(root: Node, path: Path): Node {
  let node = root;
  for (const part of path) {
    const inner = unwrap(node);
    if (typeof part === "string" && inner.kind === "object" && Object.hasOwn(inner.properties, part))
      node = inner.properties[part]!;
    else if (typeof part === "object" && "id" in part && (inner.kind === "list" || inner.kind === "tree"))
      node = inner.item;
    else if (typeof part === "object" && "key" in part && inner.kind === "record") node = inner.value;
    else throw new Error(`Unknown schema path ${JSON.stringify(path)}`);
  }
  return node;
}

export function createHandles<N extends Node>(
  node: N,
  commands: Commands,
  observer?: Observer,
  path: Path = [],
): Handle<N> {
  const frozenPath = Object.freeze(
    path.map((part) => (typeof part === "string" ? part : Object.freeze({ ...part }))),
  ) as unknown as Path;
  const run = commands.execute.bind(commands);
  const child = (next: Node, segment: Path[number]) =>
    createHandles(next, commands, observer, [...path, segment]);
  const inner = unwrap(node);
  let handle: Record<string, unknown>;
  switch (inner.kind) {
    case "object":
      handle = Object.fromEntries(
        Object.entries(inner.properties).map(([key, value]) => [key, child(value, key)]),
      );
      break;
    case "list":
      if (inner.item.kind === "object") {
        const item = inner.item;
        handle = {
          item: (id: string) => child(item, { id }),
          insert: (value: unknown, destination?: Destination) =>
            run({ type: "insert", path: frozenPath, value, ...(destination ? { destination } : {}) }),
          remove: (id: string) => void run({ type: "remove", path: frozenPath, id }),
          move: (id: string, destination: Destination) =>
            void run({ type: "move", path: frozenPath, id, destination }),
        };
      } else
        handle = {
          insert: (value: unknown, index?: number) =>
            void run({ type: "insert", path: frozenPath, value, ...(index === undefined ? {} : { index }) }),
          set: (index: number, value: any) => void run({ type: "set", path: [...frozenPath, { index }], value }),
          preview: (index: number, value: unknown) => observer?.preview?.([...frozenPath, { index }], value),
          remove: (index: number, count?: number) =>
            void run({ type: "remove", path: frozenPath, index, ...(count === undefined ? {} : { count }) }),
          move: (from: number, to: number) => void run({ type: "move", path: frozenPath, from, to }),
          replace: (values: unknown[]) => void run({ type: "assign", path: frozenPath, value: values }),
        };
      break;
    case "record": {
      const value = inner.value;
      handle = {
        entry: (key: string) => child(value, { key }),
        put: (key: string, entry: unknown) =>
          void run({ type: "assign", path: [...frozenPath, { key }], value: entry }),
        delete: (key: string) => void run({ type: "clear", path: [...frozenPath, { key }] }),
      };
      break;
    }
    case "tree": {
      const item = inner.item;
      handle = {
        item: (id: string) => child(item, { id }),
        insert: (value: unknown, destination?: Destination) =>
          run({ type: "insert", path: frozenPath, value, ...(destination ? { destination } : {}) }),
        remove: (id: string) => void run({ type: "remove", path: frozenPath, id }),
        move: (id: string, destination: Destination) =>
          void run({ type: "move", path: frozenPath, id, destination }),
      };
      break;
    }
    case "counter":
      handle = {
        increment: (by = 1) => void run({ type: "increment", path: frozenPath, value: by }),
        decrement: (by = 1) => void run({ type: "increment", path: frozenPath, value: -by }),
      };
      break;
    case "text":
    case "richtext":
      handle = {
        replace: (value: string) => void run({ type: "text.replace", path: frozenPath, value }),
        splice: (index: number, deleteCount: number, insert = "") =>
          void run({ type: "text.splice", path: frozenPath, index, delete: deleteCount, insert }),
        ...(inner.kind === "richtext"
          ? {
              mark: ({ start, end }: { start: number; end: number }, key: string, value: unknown) =>
                void run({ type: "text.mark", path: frozenPath, start, end, key, value: value as any }),
              unmark: ({ start, end }: { start: number; end: number }, key: string) =>
                void run({ type: "text.unmark", path: frozenPath, start, end, key }),
            }
          : {}),
      };
      break;
    default:
      handle = {
        set: (value: any) => void run({ type: "set", path: frozenPath, value }),
        preview: (value: unknown) => observer?.preview?.(frozenPath, value),
      };
  }
  if (node.kind === "optional") {
    if (isScalar(inner))
      Object.assign(handle, { clear: () => void run({ type: "clear", path: frozenPath }) });
    else
      Object.assign(handle, {
        set: (value: unknown) => void run({ type: "assign", path: frozenPath, value }),
        clear: () => void run({ type: "clear", path: frozenPath }),
      });
  }
  if (observer) observed.set(handle, { source: observer, path: frozenPath, node });
  return Object.freeze(handle) as Handle<N>;
}
