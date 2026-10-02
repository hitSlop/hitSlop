import type { CaptureTarget, SlopContext } from "../abi";
import type { CaptureMode } from "../abi";
import { current } from "./context";
export type { CaptureMode, CaptureTarget };

/** Export and Quick Look capture hooks, forwarded to the host runtime. */
export const capture = {
  isRenderer: () => current().capture.isRenderer(),
  onPrepare: (handler: Parameters<SlopContext["capture"]["onPrepare"]>[0]) =>
    current().capture.onPrepare(handler),
};
