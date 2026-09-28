import type { CaptureTarget, SlopContext } from "../abi";
import type { CaptureMode } from "../capture";
import { current } from "./context";
export type { CaptureMode, CaptureTarget };

/** Export and Quick Look capture hooks, forwarded to the host runtime. */
export const capture = {
  isRenderer: () => current().capture.isRenderer(),
  registerTarget: (kind: "icon" | "export", target: CaptureTarget) =>
    current().capture.registerTarget(kind, target),
  onPrepare: (handler: Parameters<SlopContext["capture"]["onPrepare"]>[0]) =>
    current().capture.onPrepare(handler),
};
