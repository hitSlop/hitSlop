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
