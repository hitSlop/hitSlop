import type { Request, Reply } from "./session-types";
import type { createCaptureController } from "./capture";

/** The lifecycle invoked by the native host in visible and headless WebViews. */
export interface SlopRuntimeHandle {
  request(request: Request): Promise<Reply>;
  flush(): Promise<void>;
  prepareClose(): Promise<void>;
  cancelClose(): void;
  close(): Promise<void>;
  retrySave(): Promise<boolean>;
  discardPending(): Promise<void>;
  reloadInterface?(): Promise<void>;
  captureBegin?(token: string): ReturnType<ReturnType<typeof createCaptureController>["begin"]>;
  captureRestore?(token: string): ReturnType<ReturnType<typeof createCaptureController>["restore"]>;
}

declare global {
  var __slop: SlopRuntimeHandle | undefined;
}
