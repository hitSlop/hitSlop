/** The lifecycle invoked by the native host through `globalThis.__slop`. */
export interface SlopPageHandle {
  publish(pushes: unknown): void;
  capture: ReturnType<typeof import("./capture").createCaptureController>;
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
  var __slop: (Pick<SlopPageHandle, "publish"> & Partial<Omit<SlopPageHandle, "publish">> & {
    dispatch: ReturnType<typeof import("./host-dispatch").hostDispatcher>;
  }) | undefined;
}
