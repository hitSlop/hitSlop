import type {
  CounterNode,
  Input,
  ListNode,
  Node,
  ObjectNode,
  OptionalNode,
  Scalar,
  Snapshot,
  Text,
  Value,
} from "./schema";

export type InsertResult = { readonly id: string };
/** Resolve a snapshot object (the root, a row or a nested object) to its handle. */
export type At = <N extends Node>(value: Snapshot<N>) => Handle<N>;
export type TextHandle = {
  /** Replace the whole field with `value`, as the text is when the owner applies it. */
  set(value: string): void;
};
export type ScalarHandle<V> = {
  set(value: V): void;
  /** Show `value` locally without writing history (for drags and drawing). The next
   * `set`, `flush`, close or export commits it. */
  preview(value: V): void;
};
export type RowDestination = { before: string } | { after: string };
type ValueHandle<N extends Node> =
  N extends ObjectNode<infer P>
    ? { readonly [K in keyof P]: Handle<P[K]> }
    : N extends ListNode<infer I>
      ? {
          item(id: string): Handle<I>;
          /** Mints the row's `$id`; inside `change()` it is available immediately. */
          insert(value: Input<I>, destination?: RowDestination): InsertResult;
          remove(id: string): void;
          move(id: string, destination?: RowDestination): void;
        }
      : N extends CounterNode
        ? { increment(by?: number): void; decrement(by?: number): void }
        : N extends Text
          ? TextHandle
          : N extends Scalar
            ? ScalarHandle<Value<N>>
            : never;
export type Handle<N extends Node> =
  N extends OptionalNode<infer S>
    ? S extends ObjectNode
      ? // Fields of an unset object are not found; `set` creates or replaces it.
        ValueHandle<S> & { set(value: Input<S>): void; clear(): void }
      : ScalarHandle<Value<S>> & { clear(): void }
    : ValueHandle<N>;
