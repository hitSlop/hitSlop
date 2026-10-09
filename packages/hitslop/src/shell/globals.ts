import type { PreviewHost } from "./preview";

/** Host-installed globals, captured before authored code mounts. */
declare global {
  var webkit: { messageHandlers?: { hitslop?: { postMessage(message: string): Promise<unknown> } } } | undefined;
  var __slopPreview: PreviewHost | undefined;
}
