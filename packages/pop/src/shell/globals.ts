import type { PreviewHost } from "./preview";

/** Host-installed globals, captured before authored code mounts. */
declare global {
  var webkit:
    { messageHandlers?: { pop?: { postMessage(message: string): Promise<unknown> } } } | undefined;
  var __popPreview: PreviewHost | undefined;
}
