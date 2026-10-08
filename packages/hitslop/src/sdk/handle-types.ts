import type { Anchor } from "../schema/core";
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

/** Live handles write asynchronously: a write resolves once `current` shows it. Inside
 * `change()`, transaction handles collect the same writes synchronously. */
type Mode = "live" | "tx";
type Write<M extends Mode, R = void> = M extends "tx" ? R : Promise<R>;

export type InsertResult = { readonly id: string };
/** Resolve a snapshot object (the root, a row, a nested object) to its handle. */
export type At<M extends Mode = "live"> = <N extends Node>(value: Snapshot<N>) => Handle<N, M>;
export type TextHandle<M extends Mode = "live"> = {
  /** Replace the whole field with `value`, as the text is when the owner applies it. */
  set(value: string): Write<M>;
} & LiveOnly<
  M,
  {
    /** Accepted text ("" while an optional text is unset). Read-only: `EditableText`
     * and `bindText` keep pending typing/composition in their native input. */
    readonly value: string;
  }
>;
type LiveOnly<M extends Mode, T> = M extends "live" ? T : unknown;
type ScalarWrites<V, M extends Mode> = {
  /** Live: shows `value` at once and resolves when it is accepted; a refusal reverts it. */
  set(value: V): Write<M>;
} & LiveOnly<
  M,
  {
    /** Show `value` locally without writing history (for drags and drawing). The next
     * `set`, `flush`, close or export commits it. */
    preview(value: V): void;
  }
>;
/** For Svelte `bind:`. Reading gives the shown value; assigning shows it at once and
 * commits it once it settles (a refused value reverts and is reported). */
type Bindable<V, M extends Mode> = M extends "live" ? { value: V } : unknown;
type ScalarHandle<V, M extends Mode = "live"> = ScalarWrites<V, M> & Bindable<V, M>;
/** Plain values addressed by index. */
type ScalarListHandle<V, M extends Mode = "live"> = {
  /** Inserts at `index` (default: the end). */
  insert(value: V, index?: number): Write<M>;
  set(index: number, value: V): Write<M>;
  remove(index: number, count?: number): Write<M>;
  /** Rewrites the list, keeping unchanged positions. */
  replace(values: V[]): Write<M>;
} & LiveOnly<M, { preview(index: number, value: V): void }>;
type ValueHandle<N extends Node, M extends Mode> =
  N extends ObjectNode<infer P>
    ? { readonly [K in keyof P]: Handle<P[K], M> }
    : N extends ListNode<infer I>
      ? I extends ObjectNode
        ? {
            item(id: string): Handle<I, M>;
            /** Mints the row's `$id`; inside `change()` it is available immediately. */
            insert(value: Input<I>, destination?: Anchor): Write<M, InsertResult>;
            remove(id: string): Write<M>;
            move(id: string, destination?: Anchor): Write<M>;
          }
        : ScalarListHandle<Value<I>, M>
      : N extends RecordNode<infer V>
        ? {
            /** The entry's handle; fields of an entry that is not set are not found. */
            entry(key: string): Handle<V, M>;
            /** Creates an absent entry with creation defaults. An existing entry requires
             * all required fields, including defaulted fields, and reconciles children
             * without replacing surviving identities. Omitted optional fields are removed.
             * The type cannot tell whether the entry exists, so it accepts the creation form
             * either way; the owner refuses an incomplete value for an existing entry. */
            put(key: string, value: Input<V>): Write<M>;
            /** Removes the entry; removing a missing key does nothing. */
            delete(key: string): Write<M>;
          }
        : N extends CounterNode
          ? {
              /** Adds `by` (default 1); a negative `by` subtracts. */ increment(
                by?: number,
              ): Write<M>;
            }
          : N extends Text
            ? TextHandle<M>
            : N extends Scalar
              ? ScalarHandle<Value<N>, M>
              : never;
export type Handle<N extends Node, M extends Mode = "live"> =
  N extends OptionalNode<infer S>
    ? S extends ObjectNode
      ? ValueHandle<S, M> & {
          /** Creates an absent object with defaults. An existing object requires all
           * required fields (the type cannot tell which; the owner refuses an incomplete
           * one); complete values reconcile children and remove omitted optionals. */
          set(value: Input<S>): Write<M>;
          clear(): Write<M>;
        }
      : S extends Text
        ? // An unset text reads as "": `set` or typing into a bound field creates it.
          TextHandle<M> & { clear(): Write<M> }
        : // `value` reads undefined while unset; assigning undefined clears it.
          ScalarWrites<Value<S>, M> & { clear(): Write<M> } & Bindable<Value<S> | undefined, M>
    : ValueHandle<N, M>;
