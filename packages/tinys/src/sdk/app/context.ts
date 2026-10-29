// App-side SDK code is compiled into each tiny. It reaches the runtime only through
// the ctx passed to mount (abi.ts); it never imports runtime modules.
import type { TinyContext } from "../abi";

/** One tiny mounts per page: the ctx its host services (attachments, speech, the window)
 * reach. Components get the document through `getDocument`, never through this. */
let active: TinyContext | undefined;
export function activate(ctx: TinyContext) {
  active = ctx;
}
export function deactivate(ctx: TinyContext) {
  if (active === ctx) active = undefined;
}
export function current(): TinyContext {
  if (!active)
    throw new Error(
      "Requires a mounted tinys app; Preview, Export, Icon and Widget use their snapshot and attachmentURL props",
    );
  return active;
}
