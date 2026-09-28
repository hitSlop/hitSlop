// App-side SDK code is compiled into each slop. It reaches the runtime only through
// the ctx passed to mount (abi.ts); it never imports runtime modules.
import type { SlopContext } from "../abi";

/** Svelte context key for the mounted slop's ctx. */
export const slopContext = Symbol("hitslop.context");
let active: SlopContext | undefined;
/** One slop mounts per page; actions and module helpers use its ctx. */
export function activate(ctx: SlopContext) {
  active = ctx;
}
export function current(): SlopContext {
  if (!active) throw new Error("Requires a mounted hitSlop app; export default defineSlop(App)");
  return active;
}
