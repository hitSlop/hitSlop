import type { Handle } from "./handle-types";
import type { Node, Snapshot } from "./schema";
/** Ordinary writes are accepted asynchronously; transaction handles remain synchronous. */
export type AsyncHandle<H> = {
  readonly [K in keyof H]: H[K] extends (...args: infer A) => infer R
    ? K extends "item" | "entry"
      ? (...args: A) => AsyncHandle<R>
      : K extends "preview"
        ? H[K]
        : (...args: A) => Promise<R>
    : AsyncHandle<H[K]>;
};
export type AsyncAt = <N extends Node>(value: Snapshot<N>) => AsyncHandle<Handle<N>>;
