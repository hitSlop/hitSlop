/** The lifecycle invoked by the native host through `globalThis.__slop`. */
export interface SlopPageHandle {
  publish(pushes: unknown): void;
  capture: ReturnType<typeof import("./capture").createCaptureController>;
  /** Applies effective theme values the native owner already validated and saved. */
  applyTheme(values: Record<string, string>): void;
  flush(): Promise<void>;
  /** Edit ▸ Undo and Redo: the page sends what the person sees first. */
  undo(): Promise<void>;
  redo(): Promise<void>;
  prepareClose(): Promise<void>;
  cancelClose(): void;
  close(): Promise<void>;
  reloadInterface(): Promise<void>;
}

declare global {
  var __slop: Pick<SlopPageHandle, "publish"> & Partial<Omit<SlopPageHandle, "publish">> | undefined;
}
