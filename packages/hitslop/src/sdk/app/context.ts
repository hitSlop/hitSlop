// App-side SDK code is compiled into each slop. It reaches the runtime only through
// the ctx passed to mount (abi.ts); it never imports runtime modules.
import type { SlopContext } from "../abi";

/** One slop mounts per page: its ctx, the definition it mounted with, and the reactive
 * document that definition reads once mounted. */
let active: { ctx: SlopContext; definition: object; document: unknown } | undefined;
export function activate(ctx: SlopContext, definition: object, document: unknown) {
  active = { ctx, definition, document };
}
export function deactivate(ctx: SlopContext) {
  if (active?.ctx === ctx) active = undefined;
}
function mounted() {
  if (!active) throw new Error("Requires a mounted hitSlop app: export default svelteApp(App, { schema })");
  return active;
}
export function current(): SlopContext {
  return mounted().ctx;
}
/** The mounted reactive document, when `definition` is the one the app mounted with. */
export function documentFor(definition: object): unknown {
  const { definition: mountedWith, document } = mounted();
  if (mountedWith !== definition) throw new Error("This document definition is not the one the app mounted");
  return document;
}
