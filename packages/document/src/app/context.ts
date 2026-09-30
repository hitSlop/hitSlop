// App-side SDK code is compiled into each slop. It reaches the runtime only through
// the ctx passed to mount (abi.ts); it never imports runtime modules.
import type { SlopContext } from "../abi";

/** One slop mounts per page; components, actions and helpers use its ctx and its
 * reactive document. */
let active: { ctx: SlopContext; document: unknown } | undefined;
export function activate(ctx: SlopContext, document: unknown) {
  active = { ctx, document };
}
export function deactivate(ctx: SlopContext) {
  if (active?.ctx === ctx) active = undefined;
}
function mounted() {
  if (!active) throw new Error("Requires a mounted hitSlop app; export default defineSlop(App)");
  return active;
}
export function current(): SlopContext {
  return mounted().ctx;
}
/** The mounted slop's reactive document. */
export function currentDocument(): unknown {
  return mounted().document;
}
