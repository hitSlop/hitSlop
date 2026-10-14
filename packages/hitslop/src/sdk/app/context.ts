// App-side SDK code is compiled into each slop. It reaches the runtime only through
// the ctx passed to mount (abi.ts); it never imports runtime modules.
import type { SlopContext } from "../abi";

/** One slop mounts per page: the ctx its host services (attachments, speech, the window)
 * reach. Components get the document through `getDocument`, never through this. */
let active: SlopContext | undefined;
export function activate(ctx: SlopContext) {
  active = ctx;
}
export function deactivate(ctx: SlopContext) {
  if (active === ctx) active = undefined;
}
export function current(): SlopContext {
  if (!active)
    throw new Error(
      "Requires a mounted hitSlop app; Preview, Export, Icon and Widget use their snapshot and attachmentURL props",
    );
  return active;
}
