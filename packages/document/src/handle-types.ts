import type {
  CounterNode,
  Input,
  ListNode,
  Node,
  ObjectNode,
  OptionalNode,
  RecordNode,
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
/** Plain values addressed by index. */
export type ScalarListHandle<V> = {
  /** Inserts at `index` (default: the end). */
  insert(value: V, index?: number): void;
  set(index: number, value: V): void;
  /** Shows `value` at `index` locally until `set`, `flush`, close or export. */
  preview(index: number, value: V): void;
  remove(index: number, count?: number): void;
  /** Rewrites the list, keeping unchanged positions. */
  replace(values: V[]): void;
};
type ValueHandle<N extends Node> =
  N extends ObjectNode<infer P>
    ? { readonly [K in keyof P]: Handle<P[K]> }
    : N extends ListNode<infer I>
      ? I extends ObjectNode
        ? {
            item(id: string): Handle<I>;
            /** Mints the row's `$id`; inside `change()` it is available immediately. */
            insert(value: Input<I>, destination?: RowDestination): InsertResult;
            remove(id: string): void;
            move(id: string, destination?: RowDestination): void;
          }
        : ScalarListHandle<Value<I>>
      : N extends RecordNode<infer V>
        ? {
            /** The entry's handle; fields of an entry that is not set are not found. */
            entry(key: string): Handle<V>;
            /** Creates or replaces the entry (an object entry holding text or lists is
             * never replaced; edit its fields instead). */
            put(key: string, value: Input<V>): void;
            /** Removes the entry; removing a missing key does nothing. */
            delete(key: string): void;
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
      : S extends Text
        ? // An unset text reads as "": `set` or typing into a bound field creates it.
          TextHandle & { clear(): void }
        : ScalarHandle<Value<S>> & { clear(): void }
    : ValueHandle<N>;
