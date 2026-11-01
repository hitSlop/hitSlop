import type { ClosePreparation } from "../wire/page.generated";

/** The lifecycle invoked by the native host through `globalThis.__pop`. */
export interface PopPageHandle {
  publish(pushes: unknown): void;
  flush(): Promise<void>;
  /** Edit ▸ Undo and Redo: the page sends what the person sees first. */
  undo(): Promise<void>;
  redo(): Promise<void>;
  prepareClose(): Promise<ClosePreparation>;
  cancelClose(): void;
  close(): Promise<void>;
  reloadInterface(): Promise<void>;
}

declare global {
  var __pop:
    | (Pick<PopPageHandle, "publish"> &
        Partial<Omit<PopPageHandle, "publish">> & {
          dispatch: ReturnType<typeof import("./host-dispatch").hostDispatcher>;
          /** The height of the page's bottom edge the keyboard and its toolbar cover. */
          keyboard?: (inset: number) => void;
          /** The status bar and home indicator heights the edge-to-edge page runs under. */
          safeArea?: (top: number, bottom: number) => void;
        })
    | undefined;
}
