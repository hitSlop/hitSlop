// App-side SDK code is compiled into each pop. It reaches the runtime only through
// the ctx passed to mount (abi.ts); it never imports runtime modules.
import type { PopContext } from "../abi";

/** One pop mounts per page: the ctx its host services (attachments, speech, the window)
 * reach. Components get the document through `getDocument`, never through this. */
let active: PopContext | undefined;
export function activate(ctx: PopContext) {
  active = ctx;
}
export function deactivate(ctx: PopContext) {
  if (active === ctx) active = undefined;
}
export function current(): PopContext {
  if (!active)
    throw new Error(
      "Requires a mounted pop app; Preview, Export, Icon and Widget use their snapshot and attachmentURL props",
    );
  return active;
}
